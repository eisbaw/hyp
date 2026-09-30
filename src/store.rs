use crate::{
    error::{Classified, ErrorKind},
    model::*,
};
use anyhow::{Context, Result, bail, ensure};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
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
pub use crate::error::Conflict;
/// An object a write named, by full ID, with its revision after the write
/// (None once deleted): what write commands print with `--json`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Written {
    pub id: String,
    pub kind: Kind,
    pub revision: Option<String>,
    /// The batch-local reference (`"@name"`) a create of `hyp apply` gave
    /// as its ID; left out of the JSON for any other change.
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    /// False for an update or archive that changed nothing and was not
    /// written. Not part of the JSON: there, an unchanged revision says so.
    #[serde(skip)]
    pub changed: bool,
}
impl Written {
    pub fn of(e: &Entry) -> Self {
        Self {
            id: e.record.id.clone(),
            kind: e.record.data.kind_value(),
            revision: Some(e.revision.clone()),
            reference: None,
            changed: true,
        }
    }
}
/// The result of `Store::commit_written`.
#[derive(Debug, Clone)]
pub struct Committed {
    /// The object each change named, in change order.
    pub written: Vec<Written>,
    /// The project after the write.
    pub snapshot: Snapshot,
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
    /// Sets only the fields in `set` (record fields by name, as `hyp --json
    /// show` prints them) and keeps the rest as stored; otherwise an update.
    Patch {
        id: String,
        expected_revision: String,
        set: serde_json::Map<String, serde_json::Value>,
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
    /// `state` of `hyp --json show`). An assessment states its hypothesis;
    /// it may cite only evidence linked to it, which the state covers.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub hypotheses: BTreeMap<String, SeenHypothesis>,
    /// Record ID -> its revision as read: every experiment target, and a
    /// run's experiment and cited evidence.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub revisions: BTreeMap<String, String>,
}
/// The field of `HypothesisState` that an assessment depends on.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeenHypothesis {
    #[serde(default)]
    pub review_token: String,
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
                        review_token: st.review_token.clone(),
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
/// stated for an update, patch, archive or delete. A record the batch
/// created (`created`) cannot be changed in it: the caller could not have
/// read its revision, so that is input to fix, not a conflict to retry.
fn stated<'a>(
    after: &'a Snapshot,
    id: &str,
    expected_revision: &str,
    created: &[String],
    references: &References,
) -> Result<&'a Entry> {
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
        bail!(Conflict::on(
            vec![id.to_string()],
            format!("{id} does not exist (deleted since you read it, or never existed)")
        ));
    }
    let current = after.find(id)?;
    let full = &current.record.id;
    if created.contains(full) {
        let named = references
            .name_of(full)
            .map_or(full.clone(), |name| format!("{name} ({full})"));
        bail!("{named} is created by this batch; put its values in the create");
    }
    if current.revision != expected_revision {
        bail!(Conflict::on(
            vec![current.record.id.clone()],
            "object changed; reload and retry"
        ));
    }
    Ok(current)
}
/// Where the create's statements disagree with `before`, the state the
/// caller read: stated hypotheses whose review token changed,
/// stated revisions that changed, and stated records that disappeared, as
/// (ID, what changed). Incomplete statements are ordinary errors.
/// Statements about records the batch created (`created`) are not checked:
/// the caller could not have read them, and they need none.
fn stale_statements(
    before: &Snapshot,
    expected: &Expected,
    created: &[String],
) -> Result<Vec<(String, String)>> {
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
        if created.contains(id) {
            continue;
        }
        ensure!(
            !seen.review_token.is_empty(),
            "expected.hypotheses[\"{id}\"].review_token is required: \
             .state.review_token of `hyp --json show {id}` as you read it"
        );
        ensure!(
            seen.review_token.len() == 64
                && seen.review_token.bytes().all(|b| b.is_ascii_hexdigit()),
            "expected.hypotheses[\"{id}\"].review_token must be the 64-hex-digit \
             .state.review_token of `hyp --json show {id}`"
        );
        full_id(id)?;
        match before.hypotheses.get(id) {
            None => stale.push((
                id.clone(),
                format!(
                    "hypothesis {id} does not exist (deleted since you read it, or never existed)"
                ),
            )),
            Some(now) if !now.review_token.eq_ignore_ascii_case(&seen.review_token) => stale
                .push((
                    id.clone(),
                    format!(
                        "hypothesis {id} changed: its basis or its current assessments (review_token)"
                    ),
                )),
            Some(_) => {}
        }
    }
    for (id, revision) in &expected.revisions {
        if created.contains(id) {
            continue;
        }
        ensure!(
            !revision.is_empty(),
            "expected.revisions[\"{id}\"] is empty: give the revision you read"
        );
        full_id(id)?;
        match before.get(id) {
            None => stale.push((
                id.clone(),
                format!("{id} does not exist (deleted since you read it, or never existed)"),
            )),
            Some(e) if e.revision != *revision => {
                stale.push((id.clone(), format!("{id} changed (revision)")))
            }
            Some(_) => {}
        }
    }
    Ok(stale)
}
/// IDs in `basis` (of `hypothesis`, at a create's position in a batch) that
/// were not in its basis in `before`, the state the caller's review token
/// covers, and that the batch did not create. Within a batch the basis may
/// grow only by the author's own new records.
fn basis_growth(
    before: &Snapshot,
    basis: &BTreeMap<String, serde_json::Value>,
    hypothesis: &str,
    created: &[String],
) -> Vec<String> {
    let reviewed = before.basis(hypothesis);
    basis
        .keys()
        .filter(|id| !reviewed.contains_key(*id) && !created.contains(id))
        .cloned()
        .collect()
}
/// Checks a create against `before`, the project as the caller read it,
/// before the server derives or freezes content for it from the state
/// including earlier changes of the batch. Without this, an assessment
/// would be recorded as based on records its author never saw. Records
/// created earlier in the batch (`created`) are the caller's own and need no
/// statement, so a create that depends only on them may omit `expected`.
/// Missing statements are ordinary errors; stale ones are one `Conflict`
/// listing them.
fn check_create(
    before: &Snapshot,
    record: &Record,
    expected: Option<&Expected>,
    created: &[String],
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
    let stated = expected.is_some();
    let expected = expected.unwrap_or(&default);
    // A statement is required, and missing: without `expected` at all, say that first.
    let required = |ok: bool, detail: String| -> Result<()> {
        match (ok, stated) {
            (true, _) => Ok(()),
            (false, true) => bail!(detail),
            (false, false) => bail!(
                "creating {} {kind} requires `expected`: what you read of the records it \
                 depends on. {detail}",
                article(kind)
            ),
        }
    };
    let revision_stated = |id: &str, what: &str| -> Result<()> {
        required(
            before.get(id).is_none() || expected.revisions.contains_key(id),
            format!("expected.revisions must give the revision you read of {what} {id}"),
        )
    };
    let problems = stale_statements(before, expected, created)?;
    match &record.data {
        Data::Assessment { hypothesis, .. } if before.get(hypothesis).is_some() => {
            required(
                expected.hypotheses.contains_key(hypothesis),
                format!(
                    "an assessment of {hypothesis} requires expected.hypotheses[\"{hypothesis}\"]: \
                     {{review_token}} as you read it (.state of `hyp --json show {hypothesis}`)"
                ),
            )?;
        }
        Data::Experiment { targets, .. } => {
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
    if !problems.is_empty() {
        let (ids, what): (Vec<String>, Vec<String>) = problems.into_iter().unzip();
        bail!(Conflict::on(
            ids,
            format!(
                "since you read the project: {}; re-read, review what changed and retry",
                what.join("; ")
            )
        ));
    }
    Ok(())
}
/// Batch-local references (HYPO-0077): a create in a `hyp apply` batch may
/// give `"id": "@name"`, and later changes of the batch write `"@name"`
/// wherever an ID is expected. Each is replaced by the full ID generated for
/// that create, before the change is checked, so the rest of the commit sees
/// only full IDs.
#[derive(Default)]
struct References(BTreeMap<String, String>);
impl References {
    fn is_reference(id: &str) -> bool {
        id.starts_with('@')
    }
    /// The reference that named the created record `id`, if one did.
    fn name_of(&self, id: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(_, full)| *full == id)
            .map(|(name, _)| name.as_str())
    }
    /// Replaces `id` if it is a reference: to one defined earlier, else an error.
    fn resolve(&self, id: &mut String) -> Result<()> {
        if !Self::is_reference(id) {
            return Ok(());
        }
        let full = self.0.get(id.as_str()).with_context(|| {
            format!("unknown reference {id}: no earlier create in this batch has \"id\": \"{id}\"")
        })?;
        id.clone_from(full);
        Ok(())
    }
    fn resolve_record(&self, r: &mut Record) -> Result<()> {
        r.data
            .references_mut()
            .into_iter()
            .try_for_each(|id| self.resolve(id))
    }
    /// The map with its keys resolved; two keys naming one record are an error.
    fn resolve_keys<V>(&self, map: &mut BTreeMap<String, V>, field: &str) -> Result<()> {
        for (mut id, value) in std::mem::take(map) {
            let given = id.clone();
            self.resolve(&mut id)?;
            ensure!(!map.contains_key(&id), "{field} names {id} twice ({given})");
            map.insert(id, value);
        }
        Ok(())
    }
    /// Resolves the references in `change`, except in a patch's `set`, which
    /// becomes a record only against the stored one. A create whose ID is a
    /// new reference gets its full ID here; the reference is returned.
    fn apply(&mut self, change: &mut Change) -> Result<Option<String>> {
        match change {
            Change::Create { record, expected } => {
                self.resolve_record(record)?;
                if let Some(e) = expected {
                    self.resolve_keys(&mut e.hypotheses, "expected.hypotheses")?;
                    self.resolve_keys(&mut e.revisions, "expected.revisions")?;
                }
                if !Self::is_reference(&record.id) {
                    return Ok(None);
                }
                let name = record.id.clone();
                ensure!(
                    name.len() > 1
                        && name[1..]
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
                    "invalid reference {name:?}: write \"@\" and a name of letters, digits, \
                     '-' or '_'"
                );
                ensure!(
                    !self.0.contains_key(&name),
                    "reference {name} is defined twice in this batch"
                );
                record.id = format!("{}-{}", record.data.prefix(), uuid::Uuid::new_v4());
                self.0.insert(name.clone(), record.id.clone());
                Ok(Some(name))
            }
            Change::Update { record, .. } => {
                self.resolve(&mut record.id)?;
                self.resolve_record(record)?;
                Ok(None)
            }
            Change::Patch { id, .. } | Change::Archive { id, .. } | Change::Delete { id, .. } => {
                self.resolve(id)?;
                Ok(None)
            }
        }
    }
}
/// Checks that an update from `old` to `new` changes only what may change.
fn check_update(old: &Record, new: &Record) -> Result<()> {
    ensure!(
        old.data.kind() == new.data.kind(),
        "cannot change object kind"
    );
    ensure!(
        !matches!(old.data, Data::Assessment { .. } | Data::Run { .. }),
        "assessments and runs are immutable; create a new record"
    );
    ensure!(
        old.created_at == new.created_at,
        "cannot change creation time"
    );
    if let (Data::Experiment { targets: a, .. }, Data::Experiment { targets: b, .. }) =
        (&old.data, &new.data)
    {
        ensure!(
            serde_json::to_value(a)? == serde_json::to_value(b)?,
            "frozen targets are immutable; create a new experiment"
        );
    }
    Ok(())
}
/// Record `old` with the fields in `set` replaced, as `Change::Patch`
/// applies it. The ID, kind and creation time cannot change, and
/// `updated_at` is the server's; assessments and runs are immutable.
fn patched(old: &Record, set: serde_json::Map<String, serde_json::Value>) -> Result<Record> {
    let id = &old.id;
    ensure!(
        !matches!(old.data, Data::Assessment { .. } | Data::Run { .. }),
        "{id} is {} {}: assessments and runs are immutable; create a new record",
        article(old.data.kind()),
        old.data.kind()
    );
    let mut fields = serde_json::to_value(old)?;
    for (name, value) in set {
        ensure!(
            !["id", "kind", "created_at"].contains(&name.as_str()),
            "patch of {id}: cannot change {name}"
        );
        ensure!(
            name != "updated_at",
            "patch of {id}: updated_at is set by the server"
        );
        fields[name] = value;
    }
    serde_json::from_value(fields).with_context(|| {
        format!("patch of {id} (every record also takes title, body, tags and archived; see hyp apply --help)")
    })
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
/// An error in the YAML front matter of a record file, its line numbers
/// counted in the file (line 1 is the opening `---`).
#[derive(Debug)]
pub struct FrontMatter(pub String);
impl std::fmt::Display for FrontMatter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for FrontMatter {}
/// `message` with every "line N column M" (how serde_yaml locates an error)
/// moved down by `by` lines.
pub fn shift_lines(message: &str, by: usize) -> String {
    let mut out = String::new();
    let mut rest = message;
    while let Some(at) = rest.find("line ") {
        let (before, after) = rest.split_at(at + "line ".len());
        out.push_str(before);
        let digits = after.bytes().take_while(u8::is_ascii_digit).count();
        match after[..digits].parse::<usize>() {
            Ok(n) if after[digits..].starts_with(" column ") => out.push_str(&(n + by).to_string()),
            _ => out.push_str(&after[..digits]),
        }
        rest = &after[digits..];
    }
    out.push_str(rest);
    out
}
pub fn decode(s: &str) -> Result<Record> {
    let s = s
        .strip_prefix("---\n")
        .context("file must start with YAML front matter (---)")?;
    let (header, body) = s
        .split_once("\n---\n")
        .context("missing front matter closing delimiter")?;
    let mut r: Record =
        serde_yaml::from_str(header).map_err(|e| FrontMatter(shift_lines(&e.to_string(), 1)))?;
    r.body = body.to_string();
    Ok(r)
}
fn atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    write_atomic(path, bytes, tempfile::Builder::new())
}
/// Like `atomic`, but the file gets mode 0644 less the umask instead of the
/// temporary file's private 0600: for files other tools read, such as the
/// agent skills.
pub(crate) fn atomic_readable(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut builder = tempfile::Builder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(fs::Permissions::from_mode(0o644));
    }
    write_atomic(path, bytes, builder)
}
fn write_atomic(path: &Path, bytes: &[u8], builder: tempfile::Builder) -> Result<()> {
    let parent = path.parent().context("missing parent")?;
    let mut tmp = builder.tempfile_in(parent)?;
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
        let mut path = path.canonicalize().map_err(|e| {
            let message = format!("cannot open {}: {e}", path.display());
            match e.kind() {
                std::io::ErrorKind::NotFound => {
                    anyhow::Error::new(Classified::new(ErrorKind::NotFound, message))
                }
                _ => anyhow::Error::new(e).context(format!("cannot open {}", path.display())),
            }
        })?;
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
            if !path.pop() {
                bail!(Classified::new(
                    ErrorKind::NotFound,
                    "no hyp project found; run hyp init"
                ));
            }
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
    /// The path diagnostics of record `r` name, relative to the project root.
    fn path_of(r: &Record) -> String {
        format!("hyp/{}", Self::relative(r))
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
                    Err(e) => snap.diagnostics.push(Diagnostic::new(
                        path.strip_prefix(&self.root)?.display().to_string(),
                        Code::Malformed,
                        e.to_string(),
                    )),
                }
            }
        }
        snap.objects.sort_by(|a, b| a.record.id.cmp(&b.record.id));
        for entry in &snap.objects {
            for v in snap.validate(&entry.record) {
                let mut d = Diagnostic::new(Self::path_of(&entry.record), v.code, v.message);
                d.repair = v.repair;
                snap.diagnostics.push(d);
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
                        snap.diagnostics.push(Diagnostic::new(
                            &a.path,
                            Code::Attachment,
                            "attachment missing, unsafe, or hash mismatch",
                        ));
                    }
                }
            }
            if let Data::Hypothesis {
                untestable_reason, ..
            } = &entry.record.data
            {
                // An untestable reason is the stated alternative to a criterion.
                if untestable_reason.trim().is_empty()
                    && !snap.has_active_criterion(&entry.record.id)
                {
                    snap.diagnostics.push(Diagnostic::new(
                        &entry.record.id,
                        Code::NoCriterion,
                        "no active falsification criterion",
                    ));
                }
            }
        }
        snap.derive();
        Ok(snap)
    }
    pub fn commit(&self, changes: Vec<Change>, expected_project: Option<&str>) -> Result<Snapshot> {
        self.commit_written(changes, expected_project)
            .map(|c| c.snapshot)
    }
    /// `commit`, also returning the object each change named, by full ID (a
    /// create's ID may be assigned here). A change that turned out to be a
    /// no-op is still listed.
    ///
    /// Diagnostics that block writes (malformed files, broken attachments,
    /// invalid records) reject every write. Other errors, between loaded
    /// records, arrive from merges, syncs and hand edits; a write is accepted
    /// if it adds no error, identified by `Diagnostic::identity`, that the
    /// project did not already have. So a write may repair such errors, and
    /// an unrelated write that leaves them as they are is not blocked by them.
    pub fn commit_written(
        &self,
        changes: Vec<Change>,
        expected_project: Option<&str>,
    ) -> Result<Committed> {
        let _lock = self.lock()?;
        let before = self.read_unlocked()?;
        before.assert_writable()?;
        if let Some(expected) = expected_project {
            if expected != before.revision {
                bail!(Conflict::new("project changed; reload and retry"));
            }
        }
        let mut after = before.clone();
        let mut writes = BTreeMap::new();
        let mut created = Vec::new();
        let mut references = References::default();
        // The object each change names: ID, kind, file, batch-local reference.
        let mut named: Vec<(String, Kind, String, Option<String>)> = Vec::new();
        let name = |r: &Record, reference: Option<String>| {
            (
                r.id.clone(),
                r.data.kind_value(),
                Self::relative(r),
                reference,
            )
        };
        for mut change in changes {
            let reference = references.apply(&mut change)?;
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
                    // The hypothesis is always a target, as `hyp experiment add` makes it.
                    if let Data::Experiment {
                        hypothesis,
                        targets,
                        ..
                    } = &mut record.data
                    {
                        if !targets.iter().any(|t| t.id == *hypothesis) {
                            targets.insert(
                                0,
                                FrozenRef {
                                    id: hypothesis.clone(),
                                    ..FrozenRef::default()
                                },
                            );
                        }
                    }
                    check_create(&before, &record, expected.as_ref(), &created)?;
                    after.validate_new(&record)?;
                    created.push(record.id.clone());
                    named.push(name(&record, reference));
                    record.created_at = chrono::Utc::now().to_rfc3339();
                    // Derived and frozen content comes from the current state, which
                    // check_create tied to what the caller read.
                    match &mut record.data {
                        Data::Assessment {
                            hypothesis,
                            based_on,
                            supersedes,
                            ..
                        } => {
                            let basis = after.basis(hypothesis);
                            let unreviewed = basis_growth(&before, &basis, hypothesis, &created);
                            ensure!(
                                unreviewed.is_empty(),
                                "this batch brings {} into the basis of {hypothesis} (a new or \
                                 restored link or run to records that existed before the batch), \
                                 so this assessment would rest on records not stated as read; \
                                 link pre-existing evidence in an earlier write, re-read, then assess",
                                unreviewed.join(", ")
                            );
                            *based_on = fingerprint_of(&basis);
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
                                    .ok_or_else(|| {
                                        Classified::new(
                                            ErrorKind::NotFound,
                                            format!("experiment target {} does not exist", t.id),
                                        )
                                    })?
                                    .frozen();
                            }
                        }
                        Data::Run {
                            experiment, plan, ..
                        } => {
                            let e = after.get(experiment).ok_or_else(|| {
                                Classified::new(
                                    ErrorKind::NotFound,
                                    format!("experiment {experiment} does not exist"),
                                )
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
                    let old = stated(
                        &after,
                        &record.id,
                        &expected_revision,
                        &created,
                        &references,
                    )?;
                    named.push(name(&old.record, None));
                    check_update(&old.record, &record)?;
                    record
                }
                Change::Patch {
                    id,
                    expected_revision,
                    set,
                } => {
                    let old = stated(&after, &id, &expected_revision, &created, &references)?;
                    named.push(name(&old.record, None));
                    let mut record = patched(&old.record, set)?;
                    references.resolve_record(&mut record)?;
                    check_update(&old.record, &record)?;
                    record
                }
                Change::Archive {
                    id,
                    archived,
                    expected_revision,
                } => {
                    let old = stated(&after, &id, &expected_revision, &created, &references)?;
                    named.push(name(&old.record, None));
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
                    let old =
                        stated(&after, &id, &expected_revision, &created, &references)?.clone();
                    named.push(name(&old.record, None));
                    let id = &old.record.id;
                    ensure!(
                        !matches!(old.record.data, Data::Assessment { .. } | Data::Run { .. }),
                        "historical records cannot be deleted"
                    );
                    let referrers: Vec<&Entry> = after
                        .objects
                        .iter()
                        .filter(|e| e.record.data.references().contains(&id.as_str()))
                        .collect();
                    if !referrers.is_empty() {
                        let keep = if old.record.archived {
                            "or keep it: archived, it is already out of lists and the graph"
                                .to_string()
                        } else {
                            format!("or archive it instead of deleting it: hyp archive {id}")
                        };
                        bail!(
                            "cannot delete {id}: these records refer to it:\n{}\n\
                             Delete them first (archive, then delete; assessments and runs cannot be deleted), {keep}",
                            listing(&referrers)
                        );
                    }
                    ensure!(
                        old.record.archived,
                        "archive {id} before deleting it: hyp archive {id}"
                    );
                    writes.insert(Self::relative(&old.record), None);
                    after.objects.retain(|e| e.record.id != old.record.id);
                    continue;
                }
            };
            let mut record = record;
            // An update or archive that changes nothing but updated_at is not
            // written, so the file and its revision stay as they are.
            if let Some(old) = after.get(&record.id) {
                record.updated_at.clone_from(&old.record.updated_at);
                if encode(&record)? == encode(&old.record)? {
                    continue;
                }
            }
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
        let known: Vec<(&str, Code)> = before
            .diagnostics
            .iter()
            .filter(|d| d.severity == "error")
            .map(Diagnostic::identity)
            .collect();
        for e in &after.objects {
            let path = Self::path_of(&e.record);
            if let Some(v) = after
                .validate(&e.record)
                .into_iter()
                .find(|v| !known.contains(&(path.as_str(), v.code)))
            {
                return Err(v.into());
            }
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
            bail!(Conflict::new("files changed during transaction"));
        }
        atomic(
            &self.root.join(".hyp/transaction.json"),
            &serde_json::to_vec(&writes)?,
        )?;
        self.recover()?;
        let snapshot = self.read_unlocked()?;
        let written = named
            .into_iter()
            .map(|(id, kind, file, reference)| Written {
                revision: snapshot.get(&id).map(|e| e.revision.clone()),
                changed: writes.contains_key(&file),
                id,
                kind,
                reference,
            })
            .collect();
        Ok(Committed { written, snapshot })
    }
    /// Imports the file as an asset and attaches it to evidence `id`.
    pub fn attach(&self, id: &str, path: &Path) -> Result<Committed> {
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
        self.commit_written(
            vec![Change::Update {
                record,
                expected_revision: e.revision.clone(),
            }],
            None,
        )
    }
}
