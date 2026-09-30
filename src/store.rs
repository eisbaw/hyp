use crate::model::*;
use anyhow::{Context, Result, bail, ensure};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

pub const DIRECTORIES: &[&str] = &[
    "hypotheses",
    "predictions",
    "criteria",
    "evidence",
    "links",
    "experiments",
    "runs",
    "assessments",
    "gaps",
];
/// A write precondition failed: the project or an object changed since the
/// caller read it. The CLI exits with code 3 and the API answers 409 when this
/// type is anywhere in the error chain, so wrapping it in `.context()` does not
/// change its classification. The message starts with "conflict:", which the
/// WebUI and agents read.
#[derive(Debug)]
pub struct Conflict(pub String);
impl std::fmt::Display for Conflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "conflict: {}", self.0)
    }
}
impl std::error::Error for Conflict {}
impl Conflict {
    pub fn in_chain(err: &anyhow::Error) -> bool {
        err.chain().any(|cause| cause.is::<Conflict>())
    }
}
#[derive(Debug, Clone)]
pub struct Store {
    pub root: PathBuf,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Change {
    /// Assessments, experiments and runs also need `expected`: what the
    /// caller read of the records the server derives or freezes their
    /// content from.
    Create {
        record: Record,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expected: Option<Expected>,
    },
    Update {
        record: Record,
        expected_revision: String,
    },
    Archive {
        id: String,
        archived: bool,
        expected_revision: String,
    },
    Delete {
        id: String,
        expected_revision: String,
    },
}
/// What the caller read, as the preconditions of a create. Any stated entry
/// that changed or disappeared since is a `Conflict`. Keys are full IDs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expected {
    /// Hypothesis ID -> its state as read (`Snapshot::hypotheses`, the
    /// `state` of `hyp --json show`). An assessment states its hypothesis.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub hypotheses: BTreeMap<String, SeenHypothesis>,
    /// Record ID -> its revision as read: every experiment target, a run's
    /// experiment and cited evidence, and every record an assessment adds to
    /// its hypothesis' relevant set (see `assessment_additions`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub revisions: BTreeMap<String, String>,
}
/// The fields of `HypothesisState` that an assessment depends on.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeenHypothesis {
    #[serde(default)]
    pub fingerprint: String,
    /// Absent is a missing statement; an empty list states "no assessments".
    #[serde(default)]
    pub assessment_ids: Option<Vec<String>>,
}
/// Records an assessment brings into its hypothesis' fingerprint beyond what
/// the fingerprint already covered in `read`: its cited evidence, links
/// touching that evidence and their other ends. `current` is the state the
/// assessment is added to. Only IDs that exist in `read` are returned. The
/// server and `Change::create_seen` both use this; web/app.js mirrors it.
pub fn assessment_additions(
    read: &Snapshot,
    current: &Snapshot,
    assessment: &Record,
) -> Vec<String> {
    let Data::Assessment { hypothesis, .. } = &assessment.data else {
        return vec![];
    };
    let covered = read.relevant(hypothesis);
    current
        .relevant_with(assessment)
        .difference(&covered)
        .filter(|id| read.get(id).is_some())
        .cloned()
        .collect()
}
impl Change {
    /// A create whose `expected` states what `seen`, the snapshot the caller
    /// read, holds of the records the server derives or freezes content
    /// from. Server-set fields (`based_on`, `supersedes`, frozen content) are
    /// cleared. Records missing from `seen` are not stated: either the batch
    /// creates them, or commit reports them.
    pub fn create_seen(mut record: Record, seen: &Snapshot) -> Self {
        let mut expected = Expected::default();
        let mut state = |id: &str| {
            if let Some(e) = seen.get(id) {
                expected
                    .revisions
                    .insert(id.to_string(), e.revision.clone());
            }
        };
        match &mut record.data {
            Data::Assessment {
                based_on,
                supersedes,
                ..
            } => {
                based_on.clear();
                supersedes.clear();
                for id in assessment_additions(seen, seen, &record) {
                    state(&id);
                }
            }
            Data::Experiment {
                hypothesis,
                targets,
                ..
            } => {
                if !targets.iter().any(|t| t.id == *hypothesis) {
                    targets.insert(0, FrozenRef::default());
                    targets[0].id.clone_from(hypothesis);
                }
                for t in targets.iter_mut() {
                    *t = FrozenRef {
                        id: seen
                            .find(&t.id)
                            .map_or(t.id.clone(), |e| e.record.id.clone()),
                        ..FrozenRef::default()
                    };
                    state(&t.id);
                }
            }
            Data::Run {
                experiment,
                plan,
                evidence,
                ..
            } => {
                *plan = FrozenRef::default();
                state(experiment);
                evidence.iter().for_each(|id| state(id));
            }
            _ => {}
        }
        if let Data::Assessment { hypothesis, .. } = &record.data {
            if let Some(st) = seen.hypotheses.get(hypothesis) {
                expected.hypotheses.insert(
                    hypothesis.clone(),
                    SeenHypothesis {
                        fingerprint: st.fingerprint.clone(),
                        assessment_ids: Some(st.assessment_ids.clone()),
                    },
                );
            }
        }
        let needs_expected = matches!(
            record.data,
            Data::Assessment { .. } | Data::Experiment { .. } | Data::Run { .. }
        );
        Self::Create {
            record,
            expected: needs_expected.then_some(expected),
        }
    }
}
/// The record `id` (a full ID or unique prefix) whose revision the caller
/// stated for an update, archive or delete.
fn stated<'a>(after: &'a Snapshot, id: &str, expected_revision: &str) -> Result<&'a Entry> {
    ensure!(
        !expected_revision.is_empty(),
        "expected_revision is required: the revision of {id} as you read it"
    );
    let prefix = id.to_lowercase();
    if !after
        .objects
        .iter()
        .any(|e| e.record.id.to_lowercase().starts_with(&prefix))
    {
        bail!(Conflict(format!(
            "{id} does not exist (deleted since you read it, or never existed)"
        )));
    }
    let current = after.find(id)?;
    if current.revision != expected_revision {
        bail!(Conflict("object changed; reload and retry".into()));
    }
    Ok(current)
}
/// Where the create's statements disagree with `before`, the state the
/// caller read: stated hypotheses whose fingerprint or assessments changed,
/// stated revisions that changed, and stated records that disappeared.
/// Incomplete statements are ordinary errors.
fn stale_statements(before: &Snapshot, expected: &Expected) -> Result<Vec<String>> {
    let full_id = |id: &str| -> Result<()> {
        if let Ok(e) = before.find(id) {
            ensure!(
                e.record.id == id,
                "expected names {id}; use the full ID {}",
                e.record.id
            );
        }
        Ok(())
    };
    let mut stale = Vec::new();
    for (id, seen) in &expected.hypotheses {
        ensure!(
            !seen.fingerprint.is_empty(),
            "expected.hypotheses[\"{id}\"].fingerprint is required: \
             .state.fingerprint of `hyp --json show {id}` as you read it"
        );
        let Some(ids) = &seen.assessment_ids else {
            bail!(
                "expected.hypotheses[\"{id}\"].assessment_ids is required: \
                 .state.assessment_ids of `hyp --json show {id}` as you read it ([] for none)"
            );
        };
        full_id(id)?;
        match before.hypotheses.get(id) {
            None => stale.push(format!(
                "hypothesis {id} does not exist (deleted since you read it, or never existed)"
            )),
            Some(now) => {
                if now.fingerprint != seen.fingerprint {
                    stale.push(format!("hypothesis {id} changed (fingerprint)"));
                }
                let current: BTreeSet<&String> = now.assessment_ids.iter().collect();
                if current != ids.iter().collect() {
                    stale.push(format!("the assessments of {id} changed (assessment_ids)"));
                }
            }
        }
    }
    for (id, revision) in &expected.revisions {
        ensure!(
            !revision.is_empty(),
            "expected.revisions[\"{id}\"] is empty: give the revision you read"
        );
        full_id(id)?;
        match before.get(id) {
            None => stale.push(format!(
                "{id} does not exist (deleted since you read it, or never existed)"
            )),
            Some(e) if e.revision != *revision => stale.push(format!("{id} changed (revision)")),
            Some(_) => {}
        }
    }
    Ok(stale)
}
/// Checks a create against `before`, the project as the caller read it,
/// before the server derives or freezes content for it from `after` (the
/// state including earlier changes of the batch). Without this, an
/// assessment would be recorded as based on records its author never saw.
/// Records created earlier in the batch are the caller's own and need no
/// statement. Missing statements are ordinary errors; stale ones and
/// records the author could not have stated are one `Conflict` listing them.
fn check_create(
    before: &Snapshot,
    after: &Snapshot,
    record: &Record,
    expected: Option<&Expected>,
) -> Result<()> {
    let kind = record.data.kind();
    match &record.data {
        Data::Assessment {
            based_on,
            supersedes,
            ..
        } => ensure!(
            based_on.is_empty() && supersedes.is_empty(),
            "based_on and supersedes are set by the server; state the hypothesis \
             as you read it in expected.hypotheses"
        ),
        Data::Experiment { targets, .. } => ensure!(
            targets
                .iter()
                .all(|t| t.revision.is_empty() && t.title.is_empty() && t.body.is_empty()),
            "experiment targets take only an id; the server freezes their content, \
             and the revisions you read go in expected.revisions"
        ),
        Data::Run { plan, .. } => ensure!(
            plan.id.is_empty()
                && plan.revision.is_empty()
                && plan.title.is_empty()
                && plan.body.is_empty(),
            "a run's plan is set by the server; omit it and state the experiment's \
             revision in expected.revisions"
        ),
        _ => {}
    }
    let default = Expected::default();
    let expected = match expected {
        Some(e) => e,
        None => {
            ensure!(
                !matches!(
                    record.data,
                    Data::Assessment { .. } | Data::Experiment { .. } | Data::Run { .. }
                ),
                "creating {} {kind} requires `expected`: what you read of the records it depends on",
                if kind.starts_with(['a', 'e']) {
                    "an"
                } else {
                    "a"
                }
            );
            &default
        }
    };
    let revision_stated = |id: &str, what: &str| -> Result<()> {
        if before.get(id).is_some() {
            ensure!(
                expected.revisions.contains_key(id),
                "expected.revisions must give the revision you read of {what} {id}"
            );
        }
        Ok(())
    };
    let mut problems = stale_statements(before, expected)?;
    let mut unstated = Vec::new();
    match &record.data {
        Data::Assessment { hypothesis, .. } => {
            if before.get(hypothesis).is_some() {
                ensure!(
                    expected.hypotheses.contains_key(hypothesis),
                    "an assessment of {hypothesis} requires expected.hypotheses[\"{hypothesis}\"]: \
                     {{fingerprint, assessment_ids}} as you read them (.state of `hyp --json show {hypothesis}`)"
                );
            }
            unstated = assessment_additions(before, after, record)
                .into_iter()
                .filter(|id| !expected.revisions.contains_key(id))
                .collect();
        }
        Data::Experiment {
            hypothesis,
            targets,
            ..
        } => {
            ensure!(
                targets.iter().any(|t| t.id == *hypothesis),
                "experiment targets must include its hypothesis {hypothesis}"
            );
            for t in targets {
                revision_stated(&t.id, "experiment target")?;
            }
        }
        Data::Run {
            experiment,
            evidence,
            ..
        } => {
            revision_stated(experiment, "experiment")?;
            for e in evidence {
                revision_stated(e, "cited evidence")?;
            }
        }
        _ => {}
    }
    if !unstated.is_empty() {
        problems.push(format!(
            "the assessment would also be based on {}, not stated in expected.revisions \
             (new or newly linked?)",
            unstated.join(", ")
        ));
    }
    if !problems.is_empty() {
        bail!(Conflict(format!(
            "since you read the project: {}; re-read, review what changed and retry",
            problems.join("; ")
        )));
    }
    Ok(())
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    schema_version: u32,
    name: String,
}

pub fn encode(r: &Record) -> Result<String> {
    let mut header = r.clone();
    header.body.clear();
    Ok(format!(
        "---\n{}---\n{}",
        serde_yaml::to_string(&header)?,
        r.body
    ))
}
pub fn decode(s: &str) -> Result<Record> {
    let s = s
        .strip_prefix("---\n")
        .context("file must start with YAML front matter (---)")?;
    let (header, body) = s
        .split_once("\n---\n")
        .context("missing front matter closing delimiter")?;
    let mut r: Record = serde_yaml::from_str(header)?;
    r.body = body.to_string();
    Ok(r)
}
fn atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("missing parent")?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|e| e.error)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}
fn safe_dir(path: &Path) -> Result<()> {
    let meta =
        fs::symlink_metadata(path).with_context(|| format!("cannot inspect {}", path.display()))?;
    ensure!(
        meta.is_dir() && !meta.file_type().is_symlink(),
        "expected a real directory: {}",
        path.display()
    );
    Ok(())
}
/// Create `path` if it is missing, then require a real directory. Copies,
/// syncs and Git clones drop empty directories, so their absence is normal.
/// The parent must already exist: a missing `hyp/` is not silently recreated.
fn ensure_dir(path: &Path) -> Result<()> {
    match fs::create_dir(path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => {
            return Err(e).with_context(|| format!("cannot create {}", path.display()));
        }
    }
    safe_dir(path)
}
impl Store {
    pub fn init(root: &Path) -> Result<Self> {
        fs::create_dir_all(root).with_context(|| format!("cannot create {}", root.display()))?;
        let root = root
            .canonicalize()
            .with_context(|| format!("cannot open {}", root.display()))?;
        ensure!(
            !root.join("hyp").exists(),
            "hyp/ already exists; refusing to overwrite it"
        );
        fs::create_dir(root.join("hyp"))
            .with_context(|| format!("cannot create {}", root.join("hyp").display()))?;
        let config = Config {
            schema_version: 1,
            name: root
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
        };
        atomic(
            &root.join("hyp/config.toml"),
            toml::to_string_pretty(&config)?.as_bytes(),
        )?;
        let store = Self { root };
        store.ensure_layout()?;
        Ok(store)
    }
    pub fn open(path: &Path) -> Result<Self> {
        let mut path = path
            .canonicalize()
            .with_context(|| format!("cannot open {}", path.display()))?;
        if path.is_file() {
            path.pop();
        }
        loop {
            let config = path.join("hyp/config.toml");
            if config.is_file() {
                safe_dir(&path.join("hyp"))?;
                let text = fs::read_to_string(&config)
                    .with_context(|| format!("cannot read {}", config.display()))?;
                let cfg: Config = toml::from_str(&text)
                    .with_context(|| format!("invalid {}", config.display()))?;
                ensure!(
                    cfg.schema_version == 1,
                    "unsupported schema version {}",
                    cfg.schema_version
                );
                let store = Self { root: path };
                store.ensure_layout()?;
                return Ok(store);
            }
            ensure!(path.pop(), "no hyp project found; run hyp init");
        }
    }
    /// Idempotent: creates missing data directories and `.hyp/` (with a
    /// `.gitignore`, harmless without Git) and rejects symlinks or files in
    /// their place.
    fn ensure_layout(&self) -> Result<()> {
        safe_dir(&self.root.join("hyp"))?;
        for dir in DIRECTORIES.iter().copied().chain(["assets"]) {
            ensure_dir(&self.root.join("hyp").join(dir))?;
        }
        ensure_dir(&self.root.join(".hyp"))?;
        let gitignore = self.root.join(".hyp/.gitignore");
        if fs::symlink_metadata(&gitignore).is_err() {
            atomic(&gitignore, b"*\n")
                .with_context(|| format!("cannot write {}", gitignore.display()))?;
        }
        Ok(())
    }
    fn lock(&self) -> Result<File> {
        self.ensure_layout()?;
        let path = self.root.join(".hyp/write.lock");
        if path.symlink_metadata().is_ok() {
            ensure!(
                !path.symlink_metadata()?.file_type().is_symlink(),
                "lock is a symlink"
            );
        }
        let f = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .with_context(|| format!("cannot open {}", path.display()))?;
        f.lock_exclusive()
            .with_context(|| format!("cannot lock {}", path.display()))?;
        self.recover()?;
        Ok(f)
    }
    fn relative(r: &Record) -> String {
        format!("{}/{}.md", r.data.directory(), r.id)
    }
    fn target(&self, relative: &str) -> Result<PathBuf> {
        let components: Vec<_> = relative.split('/').collect();
        ensure!(
            components.len() == 2 && DIRECTORIES.contains(&components[0]),
            "invalid journal path"
        );
        ensure!(
            components[1].ends_with(".md")
                && !components[1].contains("..")
                && !components[1].contains('\\'),
            "invalid journal filename"
        );
        let path = self.root.join("hyp").join(relative);
        if let Ok(meta) = path.symlink_metadata() {
            ensure!(
                !meta.file_type().is_symlink(),
                "refusing symlink: {}",
                path.display()
            );
        }
        Ok(path)
    }
    fn recover(&self) -> Result<()> {
        let journal = self.root.join(".hyp/transaction.json");
        if !journal.exists() {
            return Ok(());
        }
        ensure!(
            !journal.symlink_metadata()?.file_type().is_symlink(),
            "transaction journal is a symlink"
        );
        let writes: BTreeMap<String, Option<String>> = serde_json::from_slice(
            &fs::read(&journal).with_context(|| format!("cannot read {}", journal.display()))?,
        )
        .with_context(|| format!("invalid {}", journal.display()))?;
        for (relative, body) in &writes {
            let path = self.target(relative)?;
            match body {
                Some(s) => atomic(&path, s.as_bytes())?,
                None => {
                    if path.exists() {
                        fs::remove_file(&path)?;
                        File::open(path.parent().unwrap())?.sync_all()?;
                    }
                }
            }
        }
        fs::remove_file(&journal)?;
        File::open(journal.parent().unwrap())?.sync_all()?;
        Ok(())
    }
    pub fn snapshot(&self) -> Result<Snapshot> {
        let _lock = self.lock()?;
        self.read_unlocked()
    }
    fn read_unlocked(&self) -> Result<Snapshot> {
        let mut snap = Snapshot::default();
        for dir in DIRECTORIES {
            let path = self.root.join("hyp").join(dir);
            let mut files = fs::read_dir(&path)
                .and_then(|entries| entries.collect::<std::io::Result<Vec<_>>>())
                .with_context(|| format!("cannot read {}", path.display()))?;
            files.sort_by_key(|f| f.file_name());
            for file in files {
                let path = file.path();
                if path.extension().is_none_or(|x| x != "md") {
                    continue;
                }
                let read = || -> Result<Entry> {
                    ensure!(file.file_type()?.is_file(), "record is not a regular file");
                    let raw = fs::read_to_string(&path)?;
                    let r = decode(&raw)?;
                    ensure!(
                        r.data.directory() == *dir,
                        "object kind does not match directory"
                    );
                    ensure!(
                        file.file_name().to_string_lossy() == format!("{}.md", r.id),
                        "filename must match object ID"
                    );
                    ensure!(
                        !snap.objects.iter().any(|e| e.record.id == r.id),
                        "duplicate object ID"
                    );
                    Ok(Entry {
                        record: r,
                        revision: hash(raw),
                    })
                };
                match read() {
                    Ok(e) => snap.objects.push(e),
                    Err(e) => snap.diagnostics.push(Diagnostic {
                        path: path.strip_prefix(&self.root)?.display().to_string(),
                        message: e.to_string(),
                        severity: "error".into(),
                    }),
                }
            }
        }
        snap.objects.sort_by(|a, b| a.record.id.cmp(&b.record.id));
        for entry in &snap.objects {
            if let Err(e) = snap.validate(&entry.record) {
                snap.diagnostics.push(Diagnostic {
                    path: format!("hyp/{}", Self::relative(&entry.record)),
                    message: e.to_string(),
                    severity: "error".into(),
                });
            }
            if let Data::Evidence { attachments, .. } = &entry.record.data {
                for a in attachments {
                    let path = self.root.join("hyp").join(&a.path);
                    let valid = path
                        .canonicalize()
                        .ok()
                        .filter(|p| p.starts_with(self.root.join("hyp/assets")))
                        .and_then(|p| fs::read(p).ok())
                        .is_some_and(|bytes| hash(bytes) == a.sha256);
                    if !valid {
                        snap.diagnostics.push(Diagnostic {
                            path: a.path.clone(),
                            message: "attachment missing, unsafe, or hash mismatch".into(),
                            severity: "error".into(),
                        });
                    }
                }
            }
            if let Data::Hypothesis { .. } = entry.record.data {
                if !snap.objects.iter().any(|e|!e.record.archived && matches!(&e.record.data,Data::Criterion{hypothesis} if hypothesis==&entry.record.id)) {
                    snap.diagnostics.push(Diagnostic{path:entry.record.id.clone(),message:"no active falsification criterion".into(),severity:"warning".into()});
                }
            }
        }
        snap.derive();
        Ok(snap)
    }
    pub fn commit(&self, changes: Vec<Change>, expected_project: Option<&str>) -> Result<Snapshot> {
        let _lock = self.lock()?;
        let before = self.read_unlocked()?;
        before.assert_healthy()?;
        if let Some(expected) = expected_project {
            if expected != before.revision {
                bail!(Conflict("project changed; reload and retry".into()));
            }
        }
        let mut after = before.clone();
        let mut writes = BTreeMap::new();
        for change in changes {
            let record = match change {
                Change::Create {
                    mut record,
                    expected,
                } => {
                    if record.id.is_empty() {
                        record.id = format!("{}-{}", record.data.prefix(), uuid::Uuid::new_v4());
                    }
                    ensure!(
                        !after.objects.iter().any(|e| e.record.id == record.id),
                        "ID already exists"
                    );
                    check_create(&before, &after, &record, expected.as_ref())?;
                    record.created_at = chrono::Utc::now().to_rfc3339();
                    // Derived and frozen content comes from the current state, which
                    // check_create tied to what the caller read.
                    let relevant = after.relevant_with(&record);
                    match &mut record.data {
                        Data::Assessment {
                            hypothesis,
                            based_on,
                            supersedes,
                            ..
                        } => {
                            *based_on = after.fingerprint_of(&relevant);
                            *supersedes = after
                                .assessment_heads(hypothesis)
                                .iter()
                                .map(|e| e.record.id.clone())
                                .collect();
                        }
                        Data::Experiment { targets, .. } => {
                            for t in targets.iter_mut() {
                                *t = after
                                    .get(&t.id)
                                    .with_context(|| {
                                        format!("experiment target {} does not exist", t.id)
                                    })?
                                    .frozen();
                            }
                        }
                        Data::Run {
                            experiment, plan, ..
                        } => {
                            let e = after.get(experiment).with_context(|| {
                                format!("experiment {experiment} does not exist")
                            })?;
                            *plan = e.frozen();
                            // Include the complete plan metadata as well as prose in the historical snapshot.
                            plan.body = encode(&e.record)?;
                        }
                        _ => {}
                    }
                    record
                }
                Change::Update {
                    record,
                    expected_revision,
                } => {
                    let old = stated(&after, &record.id, &expected_revision)?;
                    ensure!(
                        old.record.data.kind() == record.data.kind(),
                        "cannot change object kind"
                    );
                    ensure!(
                        !matches!(old.record.data, Data::Assessment { .. } | Data::Run { .. }),
                        "assessments and runs are immutable; create a new record"
                    );
                    ensure!(
                        old.record.created_at == record.created_at,
                        "cannot change creation time"
                    );
                    if let (
                        Data::Experiment { targets: a, .. },
                        Data::Experiment { targets: b, .. },
                    ) = (&old.record.data, &record.data)
                    {
                        ensure!(
                            serde_json::to_value(a)? == serde_json::to_value(b)?,
                            "frozen targets are immutable; create a new experiment"
                        );
                    }
                    record
                }
                Change::Archive {
                    id,
                    archived,
                    expected_revision,
                } => {
                    let old = stated(&after, &id, &expected_revision)?;
                    ensure!(
                        !matches!(old.record.data, Data::Assessment { .. } | Data::Run { .. }),
                        "historical assessments and runs cannot be archived"
                    );
                    let mut r = old.record.clone();
                    r.archived = archived;
                    r
                }
                Change::Delete {
                    id,
                    expected_revision,
                } => {
                    let old = stated(&after, &id, &expected_revision)?.clone();
                    ensure!(old.record.archived, "archive before deleting");
                    ensure!(
                        !matches!(old.record.data, Data::Assessment { .. } | Data::Run { .. }),
                        "historical records cannot be deleted"
                    );
                    ensure!(
                        !after.objects.iter().any(|e| e
                            .record
                            .data
                            .references()
                            .contains(&old.record.id.as_str())),
                        "object is referenced; archive it instead"
                    );
                    writes.insert(Self::relative(&old.record), None);
                    after.objects.retain(|e| e.record.id != old.record.id);
                    continue;
                }
            };
            let mut record = record;
            record.updated_at = chrono::Utc::now().to_rfc3339();
            let raw = encode(&record)?;
            ensure!(
                !writes.contains_key(&Self::relative(&record)),
                "batch changes the same object twice"
            );
            writes.insert(Self::relative(&record), Some(raw.clone()));
            after.objects.retain(|e| e.record.id != record.id);
            after.objects.push(Entry {
                record,
                revision: hash(raw),
            });
            after.objects.sort_by(|a, b| a.record.id.cmp(&b.record.id));
        }
        for e in &after.objects {
            after.validate(&e.record)?;
            if let Data::Evidence { attachments, .. } = &e.record.data {
                for attachment in attachments {
                    let path = self
                        .root
                        .join("hyp")
                        .join(&attachment.path)
                        .canonicalize()
                        .context("attachment does not exist; import it with hyp evidence attach")?;
                    ensure!(
                        path.starts_with(self.root.join("hyp/assets")),
                        "attachment escapes assets directory"
                    );
                    ensure!(
                        hash(fs::read(path)?) == attachment.sha256,
                        "attachment hash mismatch"
                    );
                }
            }
        }
        // Verify a second time immediately before writing to detect ordinary editor saves.
        if self.read_unlocked()?.revision != before.revision {
            bail!(Conflict("files changed during transaction".into()));
        }
        atomic(
            &self.root.join(".hyp/transaction.json"),
            &serde_json::to_vec(&writes)?,
        )?;
        self.recover()?;
        self.read_unlocked()
    }
    pub fn attach(&self, id: &str, path: &Path) -> Result<Snapshot> {
        let bytes = fs::read(path)?;
        ensure!(bytes.len() <= 32 * 1024 * 1024, "attachment exceeds 32 MiB");
        let sha = hash(&bytes);
        let rel = format!("assets/{sha}");
        let snap = self.snapshot()?;
        let e = snap.find(id)?;
        let mut record = e.record.clone();
        if let Data::Evidence { attachments, .. } = &mut record.data {
            if !attachments.iter().any(|a| a.sha256 == sha) {
                attachments.push(Attachment {
                    path: rel.clone(),
                    sha256: sha,
                });
            }
        } else {
            bail!("attachments require evidence");
        }
        let dest = self.root.join("hyp").join(rel);
        if dest.exists() {
            ensure!(
                !dest.symlink_metadata()?.file_type().is_symlink(),
                "attachment is a symlink"
            );
        }
        atomic(&dest, &bytes)?;
        self.commit(
            vec![Change::Update {
                record,
                expected_revision: e.revision.clone(),
            }],
            None,
        )
    }
}
