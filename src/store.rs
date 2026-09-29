use crate::model::*;
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
#[derive(Debug, Clone)]
pub struct Store {
    pub root: PathBuf,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Change {
    Create {
        record: Record,
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
    let meta = fs::symlink_metadata(path)?;
    ensure!(
        meta.is_dir() && !meta.file_type().is_symlink(),
        "expected a real directory: {}",
        path.display()
    );
    Ok(())
}
impl Store {
    pub fn init(root: &Path) -> Result<Self> {
        fs::create_dir_all(root)?;
        let root = root.canonicalize()?;
        ensure!(
            !root.join("hyp").exists(),
            "hyp/ already exists; refusing to overwrite it"
        );
        fs::create_dir(root.join("hyp"))?;
        for dir in DIRECTORIES.iter().copied().chain(["assets"]) {
            fs::create_dir(root.join("hyp").join(dir))?;
        }
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
        fs::create_dir_all(root.join(".hyp"))?;
        atomic(&root.join(".hyp/.gitignore"), b"*\n")?;
        Ok(Self { root })
    }
    pub fn open(path: &Path) -> Result<Self> {
        let mut path = path
            .canonicalize()
            .with_context(|| format!("cannot open {}", path.display()))?;
        if path.is_file() {
            path.pop();
        }
        loop {
            if path.join("hyp/config.toml").is_file() {
                safe_dir(&path.join("hyp"))?;
                let cfg: Config =
                    toml::from_str(&fs::read_to_string(path.join("hyp/config.toml"))?)?;
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
    fn ensure_layout(&self) -> Result<()> {
        safe_dir(&self.root.join("hyp"))?;
        for dir in DIRECTORIES.iter().copied().chain(["assets"]) {
            safe_dir(&self.root.join("hyp").join(dir))?;
        }
        fs::create_dir_all(self.root.join(".hyp"))?;
        safe_dir(&self.root.join(".hyp"))?;
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
            .open(path)?;
        f.lock_exclusive()?;
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
        let writes: BTreeMap<String, Option<String>> =
            serde_json::from_slice(&fs::read(&journal)?)?;
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
            let mut files = fs::read_dir(self.root.join("hyp").join(dir))?
                .collect::<std::io::Result<Vec<_>>>()?;
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
            ensure!(
                expected == before.revision,
                "conflict: project changed; reload and retry"
            );
        }
        let mut after = before.clone();
        let mut writes = BTreeMap::new();
        for change in changes {
            let record = match change {
                Change::Create { mut record } => {
                    if record.id.is_empty() {
                        record.id = format!("{}-{}", record.data.prefix(), uuid::Uuid::new_v4());
                    }
                    ensure!(
                        !after.objects.iter().any(|e| e.record.id == record.id),
                        "ID already exists"
                    );
                    record.created_at = chrono::Utc::now().to_rfc3339();
                    let mut basis = after.clone();
                    basis.objects.push(Entry {
                        record: record.clone(),
                        revision: String::new(),
                    });
                    if let Data::Assessment {
                        hypothesis,
                        based_on,
                        supersedes,
                        ..
                    } = &mut record.data
                    {
                        *based_on = basis.fingerprint(hypothesis);
                        *supersedes = after
                            .assessment_heads(hypothesis)
                            .iter()
                            .map(|e| e.record.id.clone())
                            .collect();
                    }
                    record
                }
                Change::Update {
                    record,
                    expected_revision,
                } => {
                    let old = after.find(&record.id)?;
                    ensure!(
                        old.revision == expected_revision,
                        "conflict: object changed; reload and retry"
                    );
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
                    let old = after.find(&id)?;
                    ensure!(
                        old.revision == expected_revision,
                        "conflict: object changed; reload and retry"
                    );
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
                    let old = after.find(&id)?.clone();
                    ensure!(
                        old.revision == expected_revision,
                        "conflict: object changed; reload and retry"
                    );
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
            // Freeze referenced text inside the same lock used to commit the experiment/run.
            if !before.objects.iter().any(|e| e.record.id == record.id) {
                match &mut record.data {
                    Data::Experiment {
                        hypothesis,
                        targets,
                        ..
                    } => {
                        let mut ids: Vec<_> = targets.iter().map(|t| t.id.clone()).collect();
                        if !ids.contains(hypothesis) {
                            ids.insert(0, hypothesis.clone());
                        }
                        *targets = ids
                            .iter()
                            .map(|id| after.find(id).map(Entry::frozen))
                            .collect::<Result<_>>()?;
                    }
                    Data::Run {
                        experiment, plan, ..
                    } => {
                        let e = after.find(experiment)?;
                        *plan = e.frozen();
                        // Include the complete plan metadata as well as prose in the historical snapshot.
                        plan.body = encode(&e.record)?;
                    }
                    _ => {}
                }
            }
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
        ensure!(
            self.read_unlocked()?.revision == before.revision,
            "conflict: files changed during transaction"
        );
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
            Some(&snap.revision),
        )
    }
}
