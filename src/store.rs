use crate::{
    error::{Classified, ErrorKind, kind_of},
    model::*,
};
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
    "data",
];
pub use crate::error::Conflict;
/// How much of the stored bytes (`hyp/assets/`) a read checks (HYPO-0004).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verify {
    /// Metadata only: each stored file a record names exists as a regular
    /// file inside `hyp/assets/`, and a data record's size matches its
    /// length (a mismatch is then hashed, to tell changed bytes from a
    /// changed record). Of the content, only the first
    /// `data::PREVIEW_BYTES` of `text/*` data are read, for previews. Bytes
    /// changed in place to other bytes of the same length go unnoticed here;
    /// `hyp check` (`Content`) reports them (`Code::ChangedBytes`), a write
    /// that newly cites them or assesses a basis citing them hashes them
    /// (`Store::verify_cited`, `Store::verify_basis`), and `hyp data get`
    /// hashes what it returns. Every read but `hyp check`.
    Metadata,
    /// Also hashes every stored file once: `hyp check`.
    Content,
}
// The locks (advisory, `flock`), all under `.hyp/`. Every holder of more
// than one takes them in the order WRITE_LOCK, GATE_LOCK, APPLY_LOCK and
// never waits for one while holding a later one, so they cannot deadlock.
//
// - A write holds WRITE_LOCK for its whole commit, then GATE_LOCK and
//   APPLY_LOCK, both exclusively, while it writes its journal, applies it
//   and removes it.
// - A read takes GATE_LOCK exclusively, then APPLY_LOCK shared, releases
//   GATE_LOCK and reads the record files and the metadata of stored bytes.
//   It never waits for a whole write; it waits while a journal is applied,
//   and while a writer waits to apply one, which waits for the reads in
//   progress. So a read is held up by the longest read in progress when a
//   writer queues; `hyp check` therefore hashes stored bytes after letting
//   go (`Store::read`). A read sees each transaction whole or not at all.
// - A read that finds a journal (under APPLY_LOCK shared, so a crashed
//   write's, or one an older hyp that does not take APPLY_LOCK is
//   applying) lets go and rolls it forward as a writer does: WRITE_LOCK,
//   then GATE_LOCK and APPLY_LOCK.
//
// GATE_LOCK gives writers precedence: `flock` favours no one, so with reads
// that always overlap, a writer waiting only for APPLY_LOCK might never
// get it. A writer waiting for APPLY_LOCK holds GATE_LOCK, so new reads
// queue behind it and it waits only for the reads in progress.
//
// On NFS, `flock` is emulated with POSIX locks, and an exclusive one needs a
// file open for writing: a read that could open GATE_LOCK only read-only
// goes without it there (`read_lock`), losing only writer precedence.
/// Held exclusively by a writer for its whole commit: writers take turns.
/// Reads take it only to roll a journal forward.
const WRITE_LOCK: &str = ".hyp/write.lock";
/// Held exclusively by a writer from before it waits for `APPLY_LOCK` until
/// it has applied its journal, and by a read only while it takes
/// `APPLY_LOCK` shared: reads queue behind a waiting writer.
const GATE_LOCK: &str = ".hyp/gate.lock";
/// Held exclusively while a journal is written and applied (or a crashed
/// one rolled forward), and shared by every read while it reads.
const APPLY_LOCK: &str = ".hyp/apply.lock";
const LOCKS: [&str; 3] = [WRITE_LOCK, GATE_LOCK, APPLY_LOCK];
/// A write's journal: present only while it is applied, or after a crash
/// until the next writer or reader rolls it forward.
const JOURNAL: &str = ".hyp/transaction.json";
/// What the files of a notebook held when read: `config.toml` and each
/// record file, by path relative to `hyp/`, mapped to the hash of its
/// content (or why it could not be read). A commit compares it just before
/// writing, to notice an editor or sync that saved a file meanwhile.
type Files = BTreeMap<String, String>;
#[cfg(test)]
thread_local! {
    /// How many full reads (`Store::read_unlocked`) this thread made.
    static FULL_READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
#[cfg(unix)]
fn is_bad_fd(e: &std::io::Error) -> bool {
    e.raw_os_error() == Some(libc::EBADF)
}
#[cfg(not(unix))]
fn is_bad_fd(_: &std::io::Error) -> bool {
    false
}
/// Opens the lock file at `path` with `options`, refusing anything but a
/// regular file: a symlink could point anywhere, and opening a FIFO
/// read-only would wait forever. It is opened non-blocking and without
/// following a symlink, then checked, so nothing can swap it in between.
fn lock_file(path: &Path, options: &OpenOptions) -> Result<File> {
    let refuse = || {
        anyhow::anyhow!(
            "{} is not a regular file (a symlink, FIFO or directory); hyp keeps its locks as \
             regular files: remove it, and hyp creates it again",
            path.display()
        )
    };
    let mut options = options.clone();
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
    }
    let file = match options.open(path) {
        #[cfg(unix)]
        Err(e) if e.raw_os_error() == Some(libc::ELOOP) => return Err(refuse()),
        other => other.with_context(|| format!("cannot open {}", path.display()))?,
    };
    if !file.metadata()?.is_file() {
        return Err(refuse());
    }
    Ok(file)
}
/// The path the diagnostics of legacy attachment `a` name: its file, as
/// every diagnostic path, relative to the project root.
fn attachment_shown(a: &Attachment) -> String {
    format!("hyp/{}", a.path)
}
/// The diagnostic for legacy attachment `a` whose bytes are not intact
/// (`code`: `Attachment` or `ChangedBytes`), `why` saying how.
fn attachment_diagnostic(a: &Attachment, code: Code, why: &str) -> Diagnostic {
    let mut d = Diagnostic::new(
        attachment_shown(a),
        code,
        format!("attachment missing, unsafe, or hash mismatch: {why}"),
    );
    d.repair = Some(Repair {
        note: Some(format!(
            "Restore hyp/{} as a regular file with the recorded bytes (sha256 {}), from a \
             backup, version control or the source of the merge or sync; a symlink there must \
             be replaced by a copy of what it points to. hyp check confirms the fix.{}",
            a.path,
            a.sha256,
            until_restored(code)
        )),
        commands: vec![],
    });
    d
}
/// The diagnostic for data record `r` whose stored bytes are not intact
/// (`code`: `Attachment` or `ChangedBytes`), `why` saying how.
fn data_diagnostic(r: &Record, code: Code, why: &str) -> Diagnostic {
    let Data::Captured {
        sha256,
        size,
        media_type,
        ..
    } = &r.data
    else {
        unreachable!("only data records have stored bytes")
    };
    let what = match code {
        Code::ChangedBytes => {
            "changed in place (to bytes of the same length, which only hyp check notices)"
        }
        _ => "not intact",
    };
    let mut d = Diagnostic::new(
        Store::path_of(r),
        code,
        format!("the bytes of data record {} are {what}: {why}", r.id),
    );
    d.repair = Some(Repair {
        note: Some(format!(
            "Restore hyp/assets/{sha256} ({size} bytes, {media_type}) as a regular file from a \
             backup, version control or the source of the merge or sync; or capture the \
             original again (hyp capture FILE --origin \"...\"), which stores the same bytes at \
             the same path. hyp check confirms the fix.{}",
            until_restored(code)
        )),
        commands: vec![],
    });
    d
}
/// The SHA-256 of the file at `path`, read in pieces: `hyp check` hashes
/// every stored file without holding one whole in memory.
fn hash_file(path: &Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 1 << 20];
    loop {
        match file.read(&mut buffer)? {
            0 => return Ok(format!("{:x}", hasher.finalize())),
            n => hasher.update(&buffer[..n]),
        }
    }
}
/// What the repair note of a stored-bytes diagnostic adds for `code`: for
/// bytes changed in place (`Code::ChangedBytes`), what hyp refuses until
/// they are restored.
fn until_restored(code: Code) -> &'static str {
    match code {
        Code::ChangedBytes => {
            " Until then hyp refuses writes that newly cite these bytes and assessments \
             whose basis holds them; other writes go on (ordinary reads do not hash \
             stored bytes)."
        }
        _ => "",
    }
}
/// Marks each of `diagnostics` that is about a record one of a write's
/// changes names with that change's index (`Diagnostic::change`) and its
/// batch-local reference. `changed` holds, in change order, the full ID each
/// change names and its reference; when several name one record, the last
/// wins.
fn locate_changes(diagnostics: &mut [Diagnostic], changed: &[(&str, Option<&str>)]) {
    for d in diagnostics {
        let file = d.path.rsplit('/').next().unwrap_or_default();
        let id = file.strip_suffix(".md").unwrap_or(file);
        if let Some((i, (_, reference))) = changed.iter().enumerate().rfind(|(_, (c, _))| *c == id)
        {
            d.change = Some(i);
            d.reference = reference.map(str::to_string);
        }
    }
}
/// Errors for an error message: how many, then each as `path (code):
/// message`, its identity first.
fn listed(errors: &[Diagnostic]) -> String {
    match errors.len() {
        1 => format!("1 error: {}", listed_items(errors)),
        n => format!("{n} errors: {}", listed_items(errors)),
    }
}
/// Diagnostics as `path (code): message`, joined by "; ".
fn listed_items(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|d| format!("{} ({}): {}", d.path, d.code, d.message))
        .collect::<Vec<_>>()
        .join("; ")
}
/// Whether `err` comes from lacking write access: a read-only file system
/// or missing permissions. Reads go on without writing then.
fn cannot_write(err: &anyhow::Error) -> bool {
    err.chain()
        .filter_map(|e| e.downcast_ref::<std::io::Error>())
        .any(|e| {
            matches!(
                e.kind(),
                std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::ReadOnlyFilesystem
            )
        })
}
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
    /// The records the write converted, beyond those its changes named:
    /// the data records it created from legacy evidence attachments and the
    /// evidence it made reference them (decision-0005); usually empty.
    pub migrated: Vec<Written>,
    /// The schema the write raised the notebook to, if it did.
    pub raised_to: Option<u32>,
}
/// The locks a writer holds while it applies a journal (`Store::applying`).
/// Fields drop in order: `APPLY_LOCK`, then `GATE_LOCK`.
struct Applying {
    _apply: File,
    _gate: File,
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
/// that changed since is a `Conflict`; one that does not exist is
/// `ErrorKind::NotFound` (HYPO-0087). Keys are full IDs.
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
/// The error for a stated `id` that matches no record before the batch
/// (HYPO-0087): `NotFound`, not a conflict. Without tombstones hyp cannot
/// tell a record deleted since the caller read it from a mistyped ID, and a
/// conflict would have the caller re-read and retry a typo forever. Either
/// way the record is not there to change; re-reading shows that.
fn absent(id: &str) -> anyhow::Error {
    Classified::not_found(
        id,
        format!(
            "{id} does not exist: no record has this ID or prefix (if you read it before, \
             it has been deleted since)"
        ),
    )
    .into()
}
/// Whether `id`, as a full ID or prefix, matches a record of `s`.
fn matches_any(s: &Snapshot, id: &str) -> bool {
    let prefix = id.to_lowercase();
    s.objects
        .iter()
        .any(|e| e.record.id.to_lowercase().starts_with(&prefix))
}
/// A create naming, in any reference field, an ID that matches no record
/// before or within the batch is `absent`, as the same ID given to the CLI
/// is `NotFound` (HYPO-0087). Anything else wrong with a reference (the
/// wrong kind, a prefix, a record the batch deleted) `validate` reports.
fn references_exist(before: &Snapshot, after: &Snapshot, r: &Record) -> Result<()> {
    for id in r.references() {
        if !matches_any(before, id) && !matches_any(after, id) {
            return Err(absent(id).context(format!("the new {} references {id}", r.data.kind())));
        }
    }
    Ok(())
}
/// The record `id` (a full ID or unique prefix) whose revision the caller
/// stated for an update, patch, archive or delete, in `after`, the state
/// the batch has reached. A record the batch created (`created`) cannot be
/// changed in it: the caller could not have read its revision, so that is
/// input to fix, not a conflict to retry. Neither is naming a record an
/// earlier change of the batch deleted. A record that did not exist
/// before the batch either is `absent`.
fn stated<'a>(
    before: &Snapshot,
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
    if !matches_any(after, id) {
        if matches_any(before, id) {
            bail!("{id} is deleted by an earlier change of this batch");
        }
        return Err(absent(id));
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
/// caller read: stated hypotheses whose review token changed and stated
/// revisions that changed, as (ID, what changed). Incomplete statements are
/// ordinary errors, and a stated record that does not exist is `absent`.
/// Statements about records the batch created (`created`) are not checked:
/// the caller could not have read them, and they need none.
fn stale_statements(
    before: &Snapshot,
    expected: &Expected,
    created: &[String],
) -> Result<Vec<(String, String)>> {
    // A stated record existed before the batch, named by its full ID; one
    // that did not is `absent`, whatever the ID looks like (HYPO-0087).
    let full_id = |id: &str, field: &str| -> Result<()> {
        let e = before.find(id).map_err(|err| match kind_of(&err) {
            ErrorKind::NotFound => absent(id).context(format!("{field}[\"{id}\"]")),
            _ => err,
        })?;
        ensure!(
            e.record.id == id,
            "expected names {id}; use the full ID {}",
            e.record.id
        );
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
        full_id(id, "expected.hypotheses")?;
        match before.hypotheses.get(id) {
            None => bail!("expected.hypotheses names {id}, which is not a hypothesis"),
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
        full_id(id, "expected.revisions")?;
        if before.get(id).is_some_and(|e| e.revision != *revision) {
            stale.push((id.clone(), format!("{id} changed (revision)")));
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
        Data::Captured {
            captured_at, size, ..
        } => ensure!(
            captured_at.is_empty() && *size == 0,
            "a data record's captured_at and size are set by the server from the stored \
             bytes; omit them"
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
        r.references_mut()
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
        !old.is_immutable(),
        "{} is immutable ({}): assessments, runs and data records cannot be changed; \
         create a new record",
        old.id,
        old.data.kind()
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
/// `updated_at` is the server's; assessments, runs and data records are
/// immutable (`Record::is_immutable`).
fn patched(old: &Record, set: serde_json::Map<String, serde_json::Value>) -> Result<Record> {
    let id = &old.id;
    ensure!(
        !old.is_immutable(),
        "{id} is immutable ({}): assessments, runs and data records cannot be changed; \
         create a new record",
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
/// The notebook schemas (decision-0004), oldest first, each with the first
/// hyp version that reads it. This hyp reads and writes all of them. A new
/// notebook starts at the first; a write raises it when it first stores
/// something only a later schema holds (`Data::schema`), and reading never
/// does. The schema table in docs/storage.md lists the same rows. From
/// schema 3 on a raise also writes the row's version to config.toml
/// (`raised`).
pub const SCHEMAS: &[(u32, &str)] = &[
    (1, "0.1.0"), // the record fields of hyp 0.1.0
    (2, "0.2.0"), // + a gap's `resolved_by` (HYPO-0076)
    (3, "0.3.0"), // + data records and `data` references; attachments migrate (decision-0005)
];
/// The schema that introduces data records: raising a notebook to it turns
/// every evidence attachment into a data record (`migrate_attachments`).
const DATA_SCHEMA: u32 = 3;
/// The schema file, relative to `hyp/`; also its key in the journal.
const CONFIG: &str = "config.toml";
/// The first schema whose config.toml names the hyp version that reads it
/// (`Config::min_hyp_version`). Earlier ones leave it out, so hyp 0.1.0,
/// which rejects unknown keys, still says "unsupported schema version".
const FIRST_SCHEMA_NAMING_ITS_HYP: u32 = 3;
/// `hyp/config.toml`. Parsed strictly, like records: an unknown key is an
/// error. `read_config` first checks the schema, so a newer notebook, which
/// may have keys this hyp does not know, gets the upgrade message instead.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    schema_version: u32,
    name: String,
    /// The first hyp that reads `schema_version`, from
    /// `FIRST_SCHEMA_NAMING_ITS_HYP` on: what an older hyp tells its user
    /// to upgrade to. Set only by `raised`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    min_hyp_version: Option<String>,
}
/// `config` raised to schema `needed`, with the `min_hyp_version` that
/// `schemas` (`SCHEMAS`, a parameter for tests) gives it from
/// `FIRST_SCHEMA_NAMING_ITS_HYP` on, and none before.
fn raised(config: Config, needed: u32, schemas: &[(u32, &str)]) -> Result<Config> {
    let min_hyp_version = if needed >= FIRST_SCHEMA_NAMING_ITS_HYP {
        let (_, version) = schemas
            .iter()
            .find(|(schema, _)| *schema == needed)
            .with_context(|| format!("schema {needed} has no row in SCHEMAS"))?;
        Some((*version).to_string())
    } else {
        None
    };
    Ok(Config {
        schema_version: needed,
        min_hyp_version,
        ..config
    })
}
/// The config of the notebook whose `hyp/` directory is `dir`. A schema
/// newer than `SCHEMAS` knows is `ErrorKind::UnsupportedSchema`, naming the
/// hyp version to upgrade to: the config's `min_hyp_version` when it has
/// one (notebooks of schema 3 and later carry it, as only the hyp that wrote
/// them knows it), otherwise any version newer than this one.
fn read_config(dir: &Path) -> Result<Config> {
    read_config_of(dir, SCHEMAS)
}
/// `read_config` for a hyp that reads `schemas` (`SCHEMAS`; a parameter so
/// that a test can play an older hyp).
fn read_config_of(dir: &Path, schemas: &[(u32, &str)]) -> Result<Config> {
    let path = dir.join(CONFIG);
    let text =
        fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))?;
    let invalid = || format!("invalid {}", path.display());
    let table: toml::Table = toml::from_str(&text).with_context(invalid)?;
    let (first, last) = (schemas[0], schemas[schemas.len() - 1]);
    let version = env!("CARGO_PKG_VERSION");
    if let Some(schema) = table
        .get("schema_version")
        .and_then(toml::Value::as_integer)
        .filter(|&n| n > i64::from(last.0))
    {
        let upgrade = match table.get("min_hyp_version").and_then(toml::Value::as_str) {
            Some(min) => format!("upgrade hyp to >= {min}"),
            None => format!("upgrade hyp to a version newer than {version}"),
        };
        bail!(Classified::new(
            ErrorKind::UnsupportedSchema,
            format!(
                "this notebook uses schema {schema} ({}), but this hyp {version} reads \
                 schemas {} to {}: {upgrade} (`hyp --version` shows yours). Nothing was \
                 read or written",
                path.display(),
                first.0,
                last.0
            )
        ));
    }
    let config: Config = toml::from_str(&text).with_context(invalid)?;
    ensure!(
        config.schema_version >= first.0,
        "{}: schema_version {} does not exist; the first is {}",
        invalid(),
        config.schema_version,
        first.0
    );
    Ok(config)
}

pub fn encode(r: &Record) -> Result<String> {
    let mut header = r.clone();
    header.body.clear();
    let yaml = serde_yaml::to_string(&header)?;
    // serde_yaml ends a block scalar that ends in U+2028 or U+2029 (YAML
    // line breaks) without a line feed; the closing `---` would then not
    // start a line, and `decode` could not read the file back. Refuse it
    // rather than write a file that blocks every later write.
    if !yaml.ends_with('\n') {
        let field = serde_json::to_value(&header)
            .ok()
            .and_then(|v| {
                v.as_object()?
                    .iter()
                    .find(|(_, v)| {
                        v.as_str().is_some_and(|s| {
                            s.contains('\n') && s.ends_with(['\u{2028}', '\u{2029}'])
                        })
                    })
                    .map(|(name, _)| format!("field {name}"))
            })
            .unwrap_or_else(|| "a field".into());
        bail!(Classified::new(
            ErrorKind::InvalidInput,
            format!(
                "{}: {field} is multi-line and ends with a Unicode line or paragraph \
                 separator (U+2028 or U+2029), which hyp cannot store",
                r.id
            )
        ));
    }
    Ok(format!("---\n{yaml}---\n{}", r.body))
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
    // The header keeps its last line break: without it, a block scalar
    // ending the front matter would lose its final newline.
    let at = s
        .find("\n---\n")
        .context("missing front matter closing delimiter")?;
    let (header, body) = (&s[..=at], &s[at + "\n---\n".len()..]);
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
            schema_version: SCHEMAS[0].0,
            name: root
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            min_hyp_version: None,
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
            if path.join("hyp").join(CONFIG).is_file() {
                safe_dir(&path.join("hyp"))?;
                read_config(&path.join("hyp"))?;
                let store = Self { root: path };
                store.ensure_layout_if_writable()?;
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
        // Created here, so that a notebook later made read-only still has
        // them for reads to lock (`read_lock`).
        for name in LOCKS {
            self.open_lock(name)?;
        }
        Ok(())
    }
    /// `ensure_layout` where hyp may write; false (and nothing created) on
    /// read-only media or without write permission, where reads go on with
    /// what is there and a write fails with the cause. Other problems (a
    /// symlink or file in place of a directory) are errors either way.
    fn ensure_layout_if_writable(&self) -> Result<bool> {
        match self.ensure_layout() {
            Ok(()) => Ok(true),
            Err(e) if cannot_write(&e) => Ok(false),
            Err(e) => Err(e),
        }
    }
    /// Opens lock file `name` (`WRITE_LOCK`, `GATE_LOCK` or `APPLY_LOCK`),
    /// creating it if needed, for reading and writing (`lock_file`).
    fn open_lock(&self, name: &str) -> Result<File> {
        let path = self.root.join(name);
        let mut options = OpenOptions::new();
        options.create(true).truncate(false).read(true).write(true);
        lock_file(&path, &options).map_err(|e| {
            if cannot_write(&e) {
                e.context(format!(
                    "cannot open {}: hyp needs write access to {}; nothing was written",
                    path.display(),
                    self.root.join(".hyp").display()
                ))
            } else {
                e
            }
        })
    }
    /// The write lock, held for a whole commit, after rolling forward a
    /// journal a crashed write left.
    fn lock(&self) -> Result<File> {
        self.ensure_layout()?;
        let f = self.open_lock(WRITE_LOCK)?;
        f.lock_exclusive()
            .with_context(|| format!("cannot lock {WRITE_LOCK}"))?;
        if self.root.join(JOURNAL).exists() {
            let _applying = self.applying()?;
            self.recover()?;
        }
        Ok(f)
    }
    /// `GATE_LOCK`, then `APPLY_LOCK`, both exclusively: what a writer holds
    /// (already holding `WRITE_LOCK`) to write, apply and remove a journal.
    /// No read is in progress, and new reads wait, until it is dropped.
    fn applying(&self) -> Result<Applying> {
        let gate = self.open_lock(GATE_LOCK)?;
        gate.lock_exclusive()
            .with_context(|| format!("cannot lock {GATE_LOCK}"))?;
        let apply = self.open_lock(APPLY_LOCK)?;
        apply
            .lock_exclusive()
            .with_context(|| format!("cannot lock {APPLY_LOCK}"))?;
        Ok(Applying {
            _apply: apply,
            _gate: gate,
        })
    }
    /// Lock file `name` for a read: opened (and created) for writing where
    /// hyp may write (`writable`), else read-only (locks need no write
    /// access); None if it does not exist and cannot be created.
    fn lock_for_read(&self, name: &str, writable: bool) -> Result<Option<File>> {
        let path = self.root.join(name);
        match writable.then(|| self.open_lock(name)) {
            Some(Ok(f)) => return Ok(Some(f)),
            Some(Err(e)) if !cannot_write(&e) => return Err(e),
            _ => {}
        }
        match lock_file(&path, OpenOptions::new().read(true)) {
            Ok(f) => Ok(Some(f)),
            Err(e)
                if e.downcast_ref::<std::io::Error>()
                    .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
            {
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }
    /// The lock a read holds while it reads (dropping the file releases it):
    /// `APPLY_LOCK` shared, taken through `GATE_LOCK` (see the lock order
    /// above), so no transaction is applied meanwhile. A journal seen under
    /// it is rolled forward under the write lock (`lock`) before reading.
    ///
    /// Where hyp cannot write (read-only media, another user's notebook),
    /// the lock files are opened read-only. `ensure_layout` creates them
    /// wherever hyp has written, so they are missing only where hyp never
    /// could write, as in a copy without `.hyp/` on read-only media: then
    /// the read takes no lock, as nothing writes there through hyp. A
    /// journal there cannot be rolled forward, and is an error.
    fn read_lock(&self) -> Result<Option<File>> {
        let journal = self.root.join(JOURNAL);
        loop {
            let writable = self.ensure_layout_if_writable()?;
            let gate = self.lock_for_read(GATE_LOCK, writable)?;
            let Some(apply) = self.lock_for_read(APPLY_LOCK, writable)? else {
                ensure!(
                    !journal.exists(),
                    "{} holds a write that did not finish, and hyp cannot roll it forward \
                     without write access to {}: run any hyp command there with write access",
                    journal.display(),
                    self.root.display()
                );
                return Ok(None);
            };
            let gate = match gate.map(|g| g.lock_exclusive().map(|()| g)) {
                Some(Ok(g)) => Some(g),
                // NFS emulates flock with POSIX locks, and an exclusive one
                // needs a file open for writing: on a read-only NFS mount the
                // read goes without the gate, losing only writer precedence.
                Some(Err(e)) if is_bad_fd(&e) => None,
                Some(Err(e)) => {
                    return Err(e).with_context(|| format!("cannot lock {GATE_LOCK}"));
                }
                None => None,
            };
            // fs2's, not std's `File::lock_shared` (Rust 1.89, newer than the MSRV).
            FileExt::lock_shared(&apply).with_context(|| format!("cannot lock {APPLY_LOCK}"))?;
            drop(gate);
            if !journal.exists() {
                return Ok(Some(apply));
            }
            drop(apply);
            // Rolled forward as a writer would, then read under the shared
            // lock like any other read.
            if let Err(e) = self.lock() {
                return Err(if cannot_write(&e) {
                    e.context(format!(
                        "{} holds a write that did not finish; rolling it forward needs write \
                         access",
                        journal.display()
                    ))
                } else {
                    e
                });
            }
        }
    }
    fn relative(r: &Record) -> String {
        format!("{}/{}.md", r.data.directory(), r.id)
    }
    /// The path diagnostics of record `r` name, relative to the project root.
    fn path_of(r: &Record) -> String {
        format!("hyp/{}", Self::relative(r))
    }
    /// The records `changes` repair, by full ID; empty when nothing blocks
    /// writes. Only invalid records may block here (`Code::Invalid`,
    /// `Snapshot::blocking` of `before`): `commit_planned` refused the
    /// others. Fails as blocked (`Snapshot::assert_writable`, its diagnostics
    /// marked with the changes that name them) unless every change updates,
    /// patches, archives or deletes one of them. Decided from the changes
    /// alone, before any of them is checked, so a blocked write is told so
    /// first; `assert_repaired` then requires each repaired record to be
    /// valid (or deleted). Records a migration of legacy attachments converts
    /// on the way are not changes, and not limited.
    fn repair_scope(before: &Snapshot, changes: &[Change]) -> Result<Vec<String>> {
        let blocking = before.blocking();
        if blocking.is_empty() {
            return Ok(vec![]);
        }
        // `commit_planned` refused other blocking errors before planning.
        debug_assert!(blocking.iter().all(|d| d.code == Code::Invalid));
        let invalid: BTreeSet<&str> = blocking.iter().map(|d| d.path.as_str()).collect();
        let repaired = |id: &str| {
            before
                .find(id)
                .ok()
                .filter(|e| invalid.contains(Self::path_of(&e.record).as_str()))
                .map(|e| e.record.id.clone())
        };
        // The full ID each change names, if it names an existing record.
        let named: Vec<Option<String>> = changes
            .iter()
            .map(|change| {
                let id = match change {
                    Change::Create { .. } => return None,
                    Change::Update { record, .. } => &record.id,
                    Change::Patch { id, .. }
                    | Change::Archive { id, .. }
                    | Change::Delete { id, .. } => id,
                };
                before.find(id).ok().map(|e| e.record.id.clone())
            })
            .collect();
        let ids: Option<Vec<String>> = named
            .iter()
            .map(|id| id.as_deref().and_then(repaired))
            .collect();
        if let Some(ids) = ids.filter(|ids| !ids.is_empty()) {
            return Ok(ids);
        }
        let mut err = before
            .assert_writable()
            .expect_err("invalid records block writes");
        if let Some(blocked) = err.downcast_mut::<Classified>() {
            let changed: Vec<(&str, Option<&str>)> = named
                .iter()
                .map(|id| (id.as_deref().unwrap_or_default(), None))
                .collect();
            locate_changes(&mut blocked.diagnostics, &changed);
        }
        Err(err)
    }
    /// Fails, as blocked, while `still` lists rules that records a repair
    /// changed still break.
    fn assert_repaired(still: Vec<Diagnostic>) -> Result<()> {
        if still.is_empty() {
            return Ok(());
        }
        bail!(Classified::about(
            ErrorKind::Blocked,
            format!(
                "nothing was written: while invalid records block writes, only a write that \
                 leaves each record it changes valid (or deletes it) is accepted, and this one \
                 leaves {}",
                listed(&still)
            ),
            still,
        ));
    }
    /// The file a journal entry writes: a record file, or `config.toml` when
    /// the write raises the schema.
    fn target(&self, relative: &str) -> Result<PathBuf> {
        let components: Vec<_> = relative.split('/').collect();
        // Accepting config.toml here is new in hyp 0.2.0: an older hyp that
        // finds a journal left by a crashed schema raise rejects this entry
        // as an invalid journal path and cannot recover it. That needs a
        // crash mid-raise and then a hyp downgrade on the same machine (the
        // journal is local, under .hyp/), so it is accepted, not handled;
        // this hyp rolls such a journal forward.
        if relative != CONFIG {
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
        }
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
    /// Rolls the journal forward, if there is one: writes each file it
    /// holds (atomically, one by one) and then removes it. Idempotent, so a
    /// crash here is recovered by the next call. Only with `WRITE_LOCK`
    /// held and `applying`.
    fn recover(&self) -> Result<()> {
        let journal = self.root.join(JOURNAL);
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
        // What to do when a step fails: the journal stays, and the next hyp
        // with write access rolls it forward again.
        let retry = || {
            format!(
                "the journal {} stays; run any hyp command with write access to {} to \
                 finish this write",
                journal.display(),
                self.root.display()
            )
        };
        for (relative, body) in &writes {
            let path = self.target(relative)?;
            let applied = match body {
                Some(s) => atomic(&path, s.as_bytes()),
                None if path.exists() => fs::remove_file(&path)
                    .and_then(|()| File::open(path.parent().unwrap())?.sync_all())
                    .map_err(anyhow::Error::from),
                None => Ok(()),
            };
            applied.with_context(|| format!("cannot apply {}; {}", path.display(), retry()))?;
        }
        fs::remove_file(&journal)
            .and_then(|()| File::open(journal.parent().unwrap())?.sync_all())
            .with_context(|| {
                format!(
                    "cannot remove the journal {} after applying it (its files are written); \
                     hyp needs write access to {} to remove it",
                    journal.display(),
                    self.root.join(".hyp").display()
                )
            })?;
        Ok(())
    }
    /// The project as it is, its stored bytes checked by metadata only
    /// (`Verify::Metadata`): what every command but `hyp check` reads.
    pub fn snapshot(&self) -> Result<Snapshot> {
        self.read(Verify::Metadata)
    }
    /// The project as it is, its stored bytes checked as `verify` says.
    /// Holds `APPLY_LOCK` shared while it reads (`read_lock`), not the write
    /// lock: a read waits only while a journal is applied or a writer waits
    /// to apply one, and a writer waits only for the reads in progress when
    /// it is ready to apply (see the lock order at `WRITE_LOCK`). A read
    /// that finds a crashed write's journal takes the write lock to roll it
    /// forward.
    pub fn read(&self, verify: Verify) -> Result<Snapshot> {
        let (mut snap, _) = {
            let _lock = self.read_lock()?;
            self.read_unlocked()?
        };
        // After the lock is released: hashing a large notebook takes long, and
        // a writer waiting for the reads in progress would hold up every new
        // read meanwhile. Stored bytes are named by their hash and never
        // rewritten in place by hyp, so what was listed under the lock is
        // what is hashed (`hash_stored`).
        if verify == Verify::Content {
            self.hash_stored(&mut snap);
        }
        Ok(snap)
    }
    /// The notebook's config; fails for a schema this hyp does not read.
    fn config(&self) -> Result<Config> {
        read_config(&self.root.join("hyp"))
    }
    /// The record files of `dir` (one of `DIRECTORIES`), sorted by name:
    /// each `.md` file with its content, or why it cannot be read. A missing
    /// directory has none: copies drop empty directories, and on read-only
    /// media they cannot be recreated (`ensure_layout_if_writable`).
    fn record_files(&self, dir: &str) -> Result<Vec<(PathBuf, Result<String>)>> {
        let path = self.root.join("hyp").join(dir);
        match fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            _ => safe_dir(&path)?,
        }
        let mut files = fs::read_dir(&path)
            .and_then(|entries| entries.collect::<std::io::Result<Vec<_>>>())
            .with_context(|| format!("cannot read {}", path.display()))?;
        files.sort_by_key(|f| f.file_name());
        Ok(files
            .into_iter()
            .filter(|f| f.path().extension().is_some_and(|x| x == "md"))
            .map(|f| {
                let content = (|| -> Result<String> {
                    ensure!(f.file_type()?.is_file(), "record is not a regular file");
                    Ok(fs::read_to_string(f.path())?)
                })();
                (f.path(), content)
            })
            .collect())
    }
    /// The `Files` of the notebook: config.toml and every record file,
    /// hashed, not parsed. Stored bytes are not read.
    fn files(&self) -> Result<Files> {
        let mut files = self.config_files()?;
        for dir in DIRECTORIES {
            for (path, content) in self.record_files(dir)? {
                files.insert(Self::file_key(dir, &path), Self::file_hash(&content));
            }
        }
        Ok(files)
    }
    /// `Files` with only config.toml in it, to which the records are added.
    fn config_files(&self) -> Result<Files> {
        let config = self.root.join("hyp").join(CONFIG);
        let bytes =
            fs::read(&config).with_context(|| format!("cannot read {}", config.display()))?;
        Ok(Files::from([(CONFIG.to_string(), hash(bytes))]))
    }
    fn file_key(dir: &str, path: &Path) -> String {
        format!(
            "{dir}/{}",
            path.file_name().unwrap_or_default().to_string_lossy()
        )
    }
    fn file_hash(content: &Result<String>) -> String {
        match content {
            Ok(raw) => hash(raw),
            Err(e) => format!("unreadable: {e:#}"),
        }
    }
    /// The project, its stored bytes checked as `verify` says, and the
    /// `Files` it was read from. The caller holds a lock: the read lock
    /// (`read_lock`) or the write lock.
    fn read_unlocked(&self) -> Result<(Snapshot, Files)> {
        #[cfg(test)]
        FULL_READS.with(|n| n.set(n.get() + 1));
        // Another hyp may have raised the schema since `open` (a long-running
        // `hyp web`): refuse rather than report its records as malformed.
        self.config()?;
        let mut files = self.config_files()?;
        let mut snap = Snapshot::default();
        for dir in DIRECTORIES {
            for (path, content) in self.record_files(dir)? {
                files.insert(Self::file_key(dir, &path), Self::file_hash(&content));
                let read = || -> Result<Entry> {
                    let raw = content?;
                    let r = decode(&raw)?;
                    ensure!(
                        r.data.directory() == *dir,
                        "object kind does not match directory"
                    );
                    ensure!(
                        path.file_name().unwrap_or_default().to_string_lossy()
                            == format!("{}.md", r.id),
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
        // Each stored file is checked once per size a record states for it
        // (normally one): its length, or why it is not intact.
        // The error comes with its code: `Attachment`, or `ChangedBytes` for
        // bytes only hashing tells apart.
        let mut blobs: BTreeMap<(String, u64), std::result::Result<u64, (Code, String)>> =
            BTreeMap::new();
        let mut previews = BTreeMap::new();
        for entry in &snap.objects {
            for v in snap.validate(&entry.record) {
                let d = Diagnostic::of(Self::path_of(&entry.record), v);
                snap.diagnostics.push(d);
            }
            snap.diagnostics.extend(observed_at_warning(
                &entry.record,
                Self::path_of(&entry.record),
            ));
            if let Data::Evidence { attachments, .. } = &entry.record.data {
                for a in attachments {
                    // A legacy attachment has no size: any change of its bytes
                    // shows only when they are hashed (`hash_stored`).
                    if let Err(e) = self.attachment_path(a) {
                        snap.diagnostics.push(attachment_diagnostic(
                            a,
                            Code::Attachment,
                            &format!("{e:#}"),
                        ));
                    }
                }
            }
            if let Data::Captured {
                sha256,
                size,
                media_type,
                ..
            } = &entry.record.data
            {
                // An invalid hash is reported by `validate`; it names no file.
                if crate::data::is_sha256(sha256) {
                    let stored = blobs
                        .entry((sha256.clone(), *size))
                        .or_insert_with(|| {
                            self.stored_len(sha256, *size)
                                .map_err(|e| (Code::Attachment, format!("{e:#}")))
                        })
                        .clone();
                    let id = &entry.record.id;
                    let path = Self::path_of(&entry.record);
                    // The start of intact text bytes, for a preview.
                    let start = match &stored {
                        Ok(len) if len == size && media_type.starts_with("text/") => self
                            .blob_start(sha256, crate::data::PREVIEW_BYTES + 1)
                            .map(Some)
                            .map_err(|e| (Code::Attachment, format!("{e:#}"))),
                        _ => Ok(None),
                    };
                    let d = match (stored, start) {
                        // Missing or changed bytes: restore them.
                        (Err((code, why)), _) | (_, Err((code, why))) => {
                            Some(data_diagnostic(&entry.record, code, &why))
                        }
                        // Intact bytes, but the record describes them wrongly:
                        // the record file is what changed.
                        (Ok(len), _) if len != *size => {
                            let mut d = Diagnostic::new(
                                path.clone(),
                                Code::Invalid,
                                format!(
                                    "data record {id} says size {size}, but its stored bytes \
                                     (intact, sha256 {sha256}) are {len} bytes"
                                ),
                            );
                            d.repair = Some(Repair {
                                note: Some(format!(
                                    "The record file {path} was changed, not the bytes: restore \
                                     it from version control or the source of the merge or sync, \
                                     or set its size to {len} by hand (hyp cannot change a data \
                                     record). hyp check confirms the fix."
                                )),
                                commands: vec![],
                            });
                            Some(d)
                        }
                        (Ok(_), Ok(start)) => {
                            if let Some(start) = start {
                                previews.insert(id.clone(), crate::data::preview(&start));
                            }
                            None
                        }
                    };
                    if let Some(d) = d {
                        snap.diagnostics.push(d);
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
                        Self::path_of(&entry.record),
                        Code::NoCriterion,
                        "no active falsification criterion",
                    ));
                }
            }
        }
        // A symlink among the stored files (entries named like a SHA-256,
        // which hyp reads and writes; it ignores anything else there), named
        // by a record or not, would make a later capture or migration of
        // those bytes fail: report it, unless a data record or legacy
        // attachment check above reported it already.
        let assets = self.root.join("hyp/assets");
        let entries = match fs::read_dir(&assets) {
            // Copies drop an empty assets/; see `record_files`.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![],
            listed => listed
                .and_then(|entries| entries.collect::<std::io::Result<Vec<_>>>())
                .with_context(|| format!("cannot read {}", assets.display()))?,
        };
        for entry in entries {
            let shown = format!("hyp/assets/{}", entry.file_name().to_string_lossy());
            let name = entry.file_name().to_string_lossy().into_owned();
            let reported = blobs
                .iter()
                .any(|((sha256, _), b)| *sha256 == name && b.is_err())
                || snap
                    .diagnostics
                    .iter()
                    .any(|d| d.path == shown && d.code == Code::Attachment);
            if crate::data::is_sha256(&name) && entry.file_type()?.is_symlink() && !reported {
                let mut d = Diagnostic::new(
                    &shown,
                    Code::Attachment,
                    format!("{shown} is a symlink; hyp keeps stored bytes only as regular files"),
                );
                d.repair = Some(Repair {
                    note: Some(format!(
                        "Replace {shown} with a copy of the file it points to (or remove it if \
                         nothing needs those bytes). hyp check confirms the fix."
                    )),
                    commands: vec![],
                });
                snap.diagnostics.push(d);
            }
        }
        snap.previews = previews;
        snap.derive();
        Ok((snap, files))
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
    /// invalid records) reject every write, except that invalid records
    /// alone let a write through that repairs them (`repair_scope`).
    /// Stored bytes are checked by metadata there (`Verify::Metadata`);
    /// those a change newly cites are hashed as well (`verify_cited`). Other
    /// errors, between loaded records, arrive from merges, syncs and hand
    /// edits; a write is accepted if it adds no error, identified by
    /// `Diagnostic::identity`, that the project did not already have, and a
    /// rejected one names each it would add. So a write may repair such
    /// errors, and an unrelated write that leaves them as they are is not
    /// blocked by them.
    pub fn commit_written(
        &self,
        changes: Vec<Change>,
        expected_project: Option<&str>,
    ) -> Result<Committed> {
        self.commit_planned(|_| Ok(changes), expected_project)
    }
    /// `commit_written` with the changes `plan` makes from the project as it
    /// is under the write lock, for writes that decide what to write by what
    /// exists (reusing a record rather than duplicating it). No other writer
    /// can come between that read and the write.
    fn commit_planned(
        &self,
        plan: impl FnOnce(&Snapshot) -> Result<Vec<Change>>,
        expected_project: Option<&str>,
    ) -> Result<Committed> {
        let _lock = self.lock()?;
        // Read 1 of 2. Stored bytes are checked by metadata here; those the
        // write newly relies on are hashed below (`verify_cited`).
        let (before, files) = self.read_unlocked()?;
        // Invalid records block every write but their repair, which is
        // known only once the changes are; other blocking errors block
        // before anything else.
        if before.blocking().iter().any(|d| d.code != Code::Invalid) {
            before.assert_writable()?;
        }
        let changes = plan(&before)?;
        let repaired = Self::repair_scope(&before, &changes)?;
        if let Some(expected) = expected_project {
            if expected != before.revision {
                bail!(Conflict::new("project changed; reload and retry"));
            }
        }
        let mut after = before.clone();
        let mut writes = BTreeMap::new();
        let mut created = Vec::new();
        // The hashes of stored bytes this commit has hashed.
        let mut verified = BTreeSet::new();
        // Values the write may not store (`observed_at_refusal`).
        let mut refused = Vec::new();
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
                    references_exist(&before, &after, &record)?;
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
                            self.verify_basis(&after, &basis, &mut verified)?;
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
                                        Classified::not_found(
                                            &t.id,
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
                                Classified::not_found(
                                    experiment,
                                    format!("experiment {experiment} does not exist"),
                                )
                            })?;
                            *plan = e.frozen();
                            // Include the complete plan metadata as well as prose in the historical snapshot.
                            plan.body = encode(&e.record)?;
                        }
                        // Captured bytes are stored before their record: the
                        // record describes them as they are.
                        Data::Captured {
                            sha256,
                            size,
                            media_type,
                            captured_at,
                            ..
                        } => {
                            let bytes = self.blob(sha256).with_context(|| {
                                format!(
                                    "a data record describes bytes hyp has stored; capture \
                                     them with hyp capture FILE --origin \"...\" (sha256 {sha256})"
                                )
                            })?;
                            verified.insert(sha256.clone());
                            *size = bytes.len() as u64;
                            if media_type.is_empty() {
                                *media_type = crate::data::guess_media_type(None, &bytes);
                            }
                            captured_at.clone_from(&record.created_at);
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
                        &before,
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
                    let old = stated(
                        &before,
                        &after,
                        &id,
                        &expected_revision,
                        &created,
                        &references,
                    )?;
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
                    let old = stated(
                        &before,
                        &after,
                        &id,
                        &expected_revision,
                        &created,
                        &references,
                    )?;
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
                    let old = stated(
                        &before,
                        &after,
                        &id,
                        &expected_revision,
                        &created,
                        &references,
                    )?
                    .clone();
                    named.push(name(&old.record, None));
                    let id = &old.record.id;
                    ensure!(
                        !matches!(old.record.data, Data::Assessment { .. } | Data::Run { .. }),
                        "historical records cannot be deleted"
                    );
                    let referrers: Vec<&Entry> = after
                        .objects
                        .iter()
                        .filter(|e| e.record.references().contains(&id.as_str()))
                        .collect();
                    // While invalid records block writes, only this invalid
                    // record may change, and it stays invalid if archived:
                    // repairing it is what remains.
                    let blocked = !repaired.is_empty();
                    let repair = format!(
                        "while invalid records block writes, repair {id} instead \
                         (hyp check says how)"
                    );
                    if !referrers.is_empty() {
                        let next = if blocked {
                            repair
                        } else if old.record.archived {
                            "Delete them first (archive, then delete; assessments and runs \
                             cannot be deleted), or keep it: archived, it is already out of \
                             lists and the graph"
                                .to_string()
                        } else {
                            format!(
                                "Delete them first (archive, then delete; assessments and runs \
                                 cannot be deleted), or archive it instead of deleting it: \
                                 hyp archive {id}"
                            )
                        };
                        bail!(
                            "cannot delete {id}: these records refer to it:\n{}\n{next}",
                            listing(&referrers)
                        );
                    }
                    ensure!(
                        old.record.archived,
                        "archive {id} before deleting it: {}",
                        if blocked {
                            repair
                        } else {
                            format!("hyp archive {id}")
                        }
                    );
                    writes.insert(Self::relative(&old.record), None);
                    after.objects.retain(|e| e.record.id != old.record.id);
                    continue;
                }
            };
            let mut record = record;
            // A new or changed observed_at must be readable and not in the
            // future (`observed_at_refusal`); reported with the new errors.
            if let Data::Evidence { observed_at, .. } = &record.data {
                let stored = match after.get(&record.id).map(|e| &e.record.data) {
                    Some(Data::Evidence { observed_at, .. }) => Some(observed_at.as_str()),
                    _ => None,
                };
                if let Some(why) = observed_at_refusal(stored, observed_at, chrono::Utc::now()) {
                    refused.push(Diagnostic::new(
                        Self::path_of(&record),
                        Code::BadObservedAt,
                        why,
                    ));
                }
            }
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
        // Raise the schema to what the records need (decision-0004), in the
        // same journal as the records, so a crash leaves both or neither.
        // Only a write raises it, and never lowers it. A notebook at the
        // data schema has no attachments: the raise converts them, and so
        // does any later write that finds one (merged from an older branch).
        let config = self.config()?;
        let mut migrated = Vec::new();
        let mut raised_to = None;
        if !writes.is_empty() {
            let needed = |after: &Snapshot| {
                after
                    .objects
                    .iter()
                    .map(|e| e.record.schema())
                    .max()
                    .unwrap_or(config.schema_version)
                    .max(config.schema_version)
            };
            if needed(&after) >= DATA_SCHEMA {
                migrated = self.migrate_attachments(&mut after, &mut writes)?;
                // The migration hashed the bytes of the records it created.
                for id in &migrated {
                    if let Some(Data::Captured { sha256, .. }) =
                        after.get(id).map(|e| &e.record.data)
                    {
                        verified.insert(sha256.clone());
                    }
                }
            }
            let needed = needed(&after);
            if needed > config.schema_version {
                raised_to = Some(needed);
                let raised = raised(config, needed, SCHEMAS)?;
                writes.insert(CONFIG.to_string(), Some(toml::to_string_pretty(&raised)?));
            }
        }
        let known: Vec<(&str, Code)> = before
            .diagnostics
            .iter()
            .filter(|d| d.severity == "error")
            .map(Diagnostic::identity)
            .collect();
        // Errors the write would add, and those repaired records still have.
        let (mut added, mut still) = (Vec::new(), Vec::new());
        for e in &after.objects {
            let path = Self::path_of(&e.record);
            let repairing = repaired.contains(&e.record.id);
            for v in after.validate(&e.record) {
                if !known.contains(&(path.as_str(), v.code)) {
                    // Nothing is stored: no repair for a stored record applies.
                    added.push(Diagnostic {
                        repair: None,
                        ..Diagnostic::of(&path, v)
                    });
                } else if repairing && v.code == Code::Invalid {
                    still.push(Diagnostic::of(&path, v));
                }
            }
        }
        // Which change named each record, for diagnostics of a rejected write.
        let changed: Vec<(&str, Option<&str>)> = named
            .iter()
            .map(|(id, _, _, reference)| (id.as_str(), reference.as_deref()))
            .collect();
        let locate = |mut diagnostics: Vec<Diagnostic>| -> Vec<Diagnostic> {
            locate_changes(&mut diagnostics, &changed);
            diagnostics
        };
        if !refused.is_empty() {
            bail!(Classified::about(
                ErrorKind::InvalidInput,
                format!("nothing was written: {}", listed_items(&refused)),
                locate(refused),
            ));
        }
        if !added.is_empty() {
            bail!(Classified::about(
                ErrorKind::InvalidInput,
                format!(
                    "nothing was written: the write would add {}",
                    listed(&added)
                ),
                locate(added),
            ));
        }
        Self::assert_repaired(locate(still))?;
        self.verify_cited(&before, &after, &writes, &mut verified)?;
        // Check the files once more immediately before writing, to detect an
        // editor save or a sync meanwhile: their bytes, not parsed again.
        if self.files()? != files {
            bail!(Conflict::new("files changed during transaction"));
        }
        {
            // No read is in progress while the journal exists (`read_lock`).
            let _applying = self.applying()?;
            let journal = self.root.join(JOURNAL);
            atomic(&journal, &serde_json::to_vec(&writes)?).with_context(|| {
                format!(
                    "cannot write the journal {}: hyp needs write access to {}; nothing was written",
                    journal.display(),
                    self.root.join(".hyp").display()
                )
            })?;
            self.recover()?;
        }
        // Read 2 of 2.
        let (snapshot, _) = self.read_unlocked()?;
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
        let migrated = migrated
            .into_iter()
            .filter_map(|id| snapshot.get(&id).map(Written::of))
            .collect();
        Ok(Committed {
            written,
            snapshot,
            migrated,
            raised_to,
        })
    }
    /// Where the stored bytes with hash `sha256` live, after checking that
    /// the hash has the form hyp writes (it becomes a file name).
    fn blob_path(&self, sha256: &str) -> Result<PathBuf> {
        ensure!(
            crate::data::is_sha256(sha256),
            "invalid sha256 {sha256:?}: expected 64 lowercase hex digits"
        );
        Ok(self.root.join("hyp/assets").join(sha256))
    }
    /// The length of the stored bytes with hash `sha256`, from metadata
    /// only: a regular file (not a symlink) at `hyp/assets/<sha256>`. Its
    /// content is not read; `blob` checks that.
    fn blob_len(&self, sha256: &str) -> Result<u64> {
        let path = self.blob_path(sha256)?;
        let shown = format!("hyp/assets/{sha256}");
        let meta = match fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                bail!("{shown} is missing")
            }
            other => other.with_context(|| format!("cannot inspect {shown}"))?,
        };
        ensure!(
            meta.is_file(),
            "{shown} is not a regular file (a symlink or directory)"
        );
        Ok(meta.len())
    }
    /// The length of the stored bytes with hash `sha256`, by metadata. A
    /// length other than `size` (what the record states) is checked by
    /// content as well: that tells changed bytes (an error) from a changed
    /// record (intact bytes of another length), and is rare.
    fn stored_len(&self, sha256: &str, size: u64) -> Result<u64> {
        let len = self.blob_len(sha256)?;
        if len == size {
            return Ok(len);
        }
        Ok(self.blob(sha256)?.len() as u64)
    }
    /// At most the first `n` stored bytes with hash `sha256`, unchecked
    /// beyond `blob_len`: for previews.
    fn blob_start(&self, sha256: &str, n: usize) -> Result<Vec<u8>> {
        use std::io::Read;
        self.blob_len(sha256)?;
        let path = self.blob_path(sha256)?;
        let mut start = Vec::with_capacity(n);
        File::open(&path)
            .and_then(|f| f.take(n as u64).read_to_end(&mut start))
            .with_context(|| format!("cannot read hyp/assets/{sha256}"))?;
        Ok(start)
    }
    /// The stored bytes with hash `sha256`, checked: a regular file (not a
    /// symlink) at `hyp/assets/<sha256>` whose content has that hash.
    pub fn blob(&self, sha256: &str) -> Result<Vec<u8>> {
        self.blob_len(sha256)?;
        let shown = format!("hyp/assets/{sha256}");
        let bytes =
            fs::read(self.blob_path(sha256)?).with_context(|| format!("cannot read {shown}"))?;
        ensure!(
            hash(&bytes) == sha256,
            "{shown} changed: its content no longer has that sha256"
        );
        Ok(bytes)
    }
    /// Stores `bytes` under their hash, once: an intact copy already there is
    /// left as it is, a changed one is replaced. Content-addressed, so this is
    /// idempotent and needs no lock; bytes no record names yet are harmless.
    fn store_blob(&self, bytes: &[u8]) -> Result<String> {
        let sha256 = hash(bytes);
        let path = self.blob_path(&sha256)?;
        if let Ok(meta) = fs::symlink_metadata(&path) {
            ensure!(
                !meta.file_type().is_symlink(),
                "refusing symlink: {}",
                path.display()
            );
            if self.blob(&sha256).is_ok() {
                return Ok(sha256);
            }
        }
        atomic(&path, bytes).with_context(|| format!("cannot store {}", path.display()))?;
        Ok(sha256)
    }
    /// Where a legacy attachment's bytes are, checked by metadata only: a
    /// regular file (not a symlink) inside `hyp/assets/`.
    fn attachment_path(&self, a: &Attachment) -> Result<PathBuf> {
        let joined = self.root.join("hyp").join(&a.path);
        let meta = fs::symlink_metadata(&joined)
            .with_context(|| format!("attachment {} does not exist", a.path))?;
        ensure!(
            meta.is_file(),
            "attachment {} is not a regular file (a symlink or directory)",
            a.path
        );
        let path = joined
            .canonicalize()
            .with_context(|| format!("attachment {} does not exist", a.path))?;
        ensure!(
            path.starts_with(self.root.join("hyp/assets")),
            "attachment {} escapes the assets directory",
            a.path
        );
        Ok(path)
    }
    /// The bytes of a legacy attachment, checked: `attachment_path`, and the
    /// recorded hash. `read_unlocked` reports an attachment that fails this
    /// (by metadata only, except for `hyp check`), and the schema-3
    /// migration reads its bytes through it.
    fn attachment_bytes(&self, a: &Attachment) -> Result<Vec<u8>> {
        let path = self.attachment_path(a)?;
        let bytes = fs::read(&path).with_context(|| format!("cannot read {}", path.display()))?;
        ensure!(
            hash(&bytes) == a.sha256,
            "attachment {} does not match its hash",
            a.path
        );
        Ok(bytes)
    }
    /// `Verify::Content`, after `read_unlocked`: hashes every stored file
    /// that the read found in place (by metadata), once per hash, and adds a
    /// diagnostic for each that is not intact: `ChangedBytes` for content
    /// that no longer has its hash, `Attachment` for a file that went
    /// missing or became unreadable since the read. Runs without a lock: hyp
    /// stores bytes under their hash and never rewrites them in place, so a
    /// concurrent write cannot change what is hashed; a file removed or
    /// replaced meanwhile is reported as such.
    fn hash_stored(&self, snap: &mut Snapshot) {
        // Records and attachments already reported (missing, unsafe, or a
        // size that does not match) are not hashed again.
        let reported: BTreeSet<String> = snap
            .diagnostics
            .iter()
            .filter(|d| matches!(d.code, Code::Attachment | Code::Invalid))
            .map(|d| d.path.clone())
            .collect();
        type Checked = std::result::Result<(), (Code, String)>;
        let check = |path: &Path, sha256: &str, shown: &str| -> Checked {
            match hash_file(path) {
                Err(e) => Err((Code::Attachment, format!("cannot read {shown}: {e}"))),
                Ok(h) if h != sha256 => Err((
                    Code::ChangedBytes,
                    format!("{shown} changed: its content no longer has that sha256"),
                )),
                Ok(_) => Ok(()),
            }
        };
        let mut hashed: BTreeMap<String, Checked> = BTreeMap::new();
        let mut found = Vec::new();
        for entry in &snap.objects {
            let r = &entry.record;
            match &r.data {
                Data::Captured { sha256, .. }
                    if crate::data::is_sha256(sha256) && !reported.contains(&Self::path_of(r)) =>
                {
                    let result = hashed.entry(sha256.clone()).or_insert_with(|| {
                        let shown = format!("hyp/assets/{sha256}");
                        self.blob_len(sha256)
                            .map_err(|e| (Code::Attachment, format!("{e:#}")))?;
                        check(&self.root.join("hyp/assets").join(sha256), sha256, &shown)
                    });
                    if let Err((code, why)) = result {
                        found.push((data_diagnostic(r, *code, why), Some(r.id.clone())));
                    }
                }
                Data::Evidence { attachments, .. } => {
                    for a in attachments
                        .iter()
                        .filter(|a| !reported.contains(&attachment_shown(a)))
                    {
                        let result = match self.attachment_path(a) {
                            Err(e) => Err((Code::Attachment, format!("{e:#}"))),
                            Ok(path) => check(&path, &a.sha256, &a.path),
                        };
                        if let Err((code, why)) = result {
                            found.push((attachment_diagnostic(a, code, &why), None));
                        }
                    }
                }
                _ => {}
            }
        }
        if found.is_empty() {
            return;
        }
        for (d, data_id) in found {
            // No preview of bytes that are not what the record names.
            if let Some(id) = data_id {
                snap.previews.remove(&id);
            }
            snap.diagnostics.push(d);
        }
        // The diagnostics are part of the revision.
        snap.derive();
    }
    /// Hashes the stored bytes that a record this write stores cites and
    /// did not cite before the write (in `before`): a data reference new to
    /// the record, or a legacy attachment new to the evidence. Reads check
    /// stored bytes by metadata only (`Verify::Metadata`), so this is where a
    /// write that would rely on bytes changed in place is refused. Bytes
    /// already hashed by this commit (`verified`) are not hashed again.
    /// Citations a record already had are not re-checked, so a write that
    /// removes one to broken bytes is not blocked by them.
    fn verify_cited(
        &self,
        before: &Snapshot,
        after: &Snapshot,
        writes: &BTreeMap<String, Option<String>>,
        verified: &mut BTreeSet<String>,
    ) -> Result<()> {
        let written = after.objects.iter().filter(|e| {
            writes
                .get(&Self::relative(&e.record))
                .is_some_and(Option::is_some)
        });
        for e in written {
            let old = before.get(&e.record.id).map(|o| &o.record);
            let id = &e.record.id;
            for data_id in &e.record.data_refs {
                if old.is_some_and(|o| o.data_refs.contains(data_id)) {
                    continue;
                }
                // A reference to no data record is `validate`'s to report.
                let Some(Data::Captured { sha256, .. }) =
                    after.get(data_id).map(|d| &d.record.data)
                else {
                    continue;
                };
                if verified.contains(sha256) {
                    continue;
                }
                self.blob(sha256).with_context(|| {
                    format!(
                        "{id} would cite data record {data_id}, whose stored bytes are not \
                         intact; hyp check shows the repair"
                    )
                })?;
                verified.insert(sha256.clone());
            }
            if let Data::Evidence { attachments, .. } = &e.record.data {
                let had = |a: &Attachment| {
                    old.is_some_and(|o| {
                        matches!(&o.data, Data::Evidence { attachments: had, .. }
                            if had.iter().any(|h| h.path == a.path && h.sha256 == a.sha256))
                    })
                };
                for a in attachments.iter().filter(|a| !had(a)) {
                    self.attachment_bytes(a).with_context(|| {
                        format!("{id} would cite attachment {}, which is not intact", a.path)
                    })?;
                }
            }
        }
        Ok(())
    }
    /// Hashes the stored bytes that the records of an assessment's `basis`
    /// (`Snapshot::basis`, in `after`) cite: their data references and legacy
    /// attachments. An assessment judges them, and reads check stored bytes
    /// by metadata only, so this keeps a judgment from being recorded over
    /// bytes changed in place. Bounded by the basis; judgments are rare.
    fn verify_basis(
        &self,
        after: &Snapshot,
        basis: &BTreeMap<String, serde_json::Value>,
        verified: &mut BTreeSet<String>,
    ) -> Result<()> {
        for id in basis.keys() {
            let Some(e) = after.get(id) else { continue };
            for data_id in &e.record.data_refs {
                let Some(Data::Captured { sha256, .. }) =
                    after.get(data_id).map(|d| &d.record.data)
                else {
                    continue;
                };
                if verified.contains(sha256) {
                    continue;
                }
                self.blob(sha256).with_context(|| {
                    format!(
                        "cannot assess: {id}, in the basis, cites data record {data_id}, whose \
                         stored bytes are not intact; hyp check shows the repair"
                    )
                })?;
                verified.insert(sha256.clone());
            }
            if let Data::Evidence { attachments, .. } = &e.record.data {
                for a in attachments {
                    self.attachment_bytes(a).with_context(|| {
                        format!(
                            "cannot assess: {id}, in the basis, has attachment {}, which is \
                             not intact; hyp check shows the repair",
                            a.path
                        )
                    })?;
                }
            }
        }
        Ok(())
    }
    /// decision-0005: every evidence attachment in `after` becomes a data
    /// record, as the write raises the notebook to `DATA_SCHEMA` (or, at it,
    /// finds one a merge brought in). The result depends only on the
    /// notebook, never on the clock or chance, so two copies of a notebook
    /// (parallel worktrees, clones) migrate to the same files and merge
    /// cleanly: one data record per distinct hash, with the ID
    /// `migrated_id(sha256)` (an existing record with that ID is reused),
    /// title "Migrated attachment <first 8 hex digits>", and as its times the
    /// earliest `created_at` of the evidence holding the bytes. The evidence
    /// references them first, in attachment order, then its other data
    /// references, loses its `attachments`, and keeps its `updated_at`, so
    /// its fingerprint stays the same (`Snapshot::basis`). The records go
    /// into `writes`, the same journal as the rest of the write. Returns the
    /// IDs of the records it wrote.
    fn migrate_attachments(
        &self,
        after: &mut Snapshot,
        writes: &mut BTreeMap<String, Option<String>>,
    ) -> Result<Vec<String>> {
        let holders: Vec<Record> = after
            .objects
            .iter()
            .filter(|e| matches!(&e.record.data, Data::Evidence { attachments, .. } if !attachments.is_empty()))
            .map(|e| e.record.clone())
            .collect();
        if holders.is_empty() {
            return Ok(vec![]);
        }
        // Per hash: its first path and the earliest creation time among the
        // evidence holding it (RFC 3339 times of one notebook; compared as
        // instants, the text kept as written).
        let mut first: BTreeMap<String, (String, String)> = BTreeMap::new();
        for e in &holders {
            let Data::Evidence { attachments, .. } = &e.data else {
                unreachable!("only evidence has attachments")
            };
            for a in attachments {
                let earlier = |t: &str, than: &str| match (
                    chrono::DateTime::parse_from_rfc3339(t),
                    chrono::DateTime::parse_from_rfc3339(than),
                ) {
                    (Ok(t), Ok(than)) => t < than,
                    _ => t < than,
                };
                first
                    .entry(a.sha256.clone())
                    .and_modify(|(_, at)| {
                        if earlier(&e.created_at, at) {
                            at.clone_from(&e.created_at);
                        }
                    })
                    .or_insert_with(|| (a.path.clone(), e.created_at.clone()));
            }
        }
        let mut written = Vec::new();
        let mut records = Vec::new();
        for (sha256, (path, at)) in &first {
            let id = migrated_id(sha256);
            if after.get(&id).is_some() {
                continue;
            }
            let bytes = self
                .attachment_bytes(&Attachment {
                    path: path.clone(),
                    sha256: sha256.clone(),
                })
                .with_context(|| format!("cannot turn attachment {path} into a data record"))?;
            // A hand-made attachment may live elsewhere in assets/.
            self.store_blob(&bytes)?;
            records.push(Record {
                id,
                title: format!("Migrated attachment {}", &sha256[..8]),
                body: format!(
                    "Converted from a legacy evidence attachment when the notebook was raised \
                     to schema {DATA_SCHEMA} (decision-0005). Its times are the creation time \
                     of the earliest evidence that held it, not of the original attach."
                ),
                tags: vec![],
                data_refs: vec![],
                archived: false,
                created_at: at.clone(),
                updated_at: at.clone(),
                data: Data::Captured {
                    origin: format!("migrated from evidence attachment {path}"),
                    captured_at: at.clone(),
                    media_type: crate::data::guess_media_type(None, &bytes),
                    size: bytes.len() as u64,
                    sha256: sha256.clone(),
                },
            });
        }
        for mut evidence in holders {
            let attachments = match &mut evidence.data {
                Data::Evidence { attachments, .. } => std::mem::take(attachments),
                _ => unreachable!("only evidence has attachments"),
            };
            let mut refs: Vec<String> = Vec::new();
            let migrated = attachments.iter().map(|a| migrated_id(&a.sha256));
            for id in migrated.chain(std::mem::take(&mut evidence.data_refs)) {
                if !refs.contains(&id) {
                    refs.push(id);
                }
            }
            evidence.data_refs = refs;
            records.push(evidence);
        }
        for r in records {
            let raw = encode(&r)?;
            writes.insert(Self::relative(&r), Some(raw.clone()));
            written.push(r.id.clone());
            after.objects.retain(|e| e.record.id != r.id);
            after.objects.push(Entry {
                record: r,
                revision: hash(raw),
            });
        }
        after.objects.sort_by(|a, b| a.record.id.cmp(&b.record.id));
        Ok(written)
    }
    /// Stores `c.bytes` once per hash and records them as a new data record
    /// (decision-0005). Idempotent: when a data record (not archived) already
    /// has these bytes and the same title, origin, media type and note, that
    /// one is returned, unchanged, and nothing is written; any difference
    /// makes a new record, which shares the stored bytes. The lookup happens
    /// under the write lock, so identical concurrent captures make one record.
    /// Empty bytes are refused unless `c.allow_empty`: an agent whose command
    /// failed and printed nothing must not see success.
    pub fn capture(&self, c: Capture) -> Result<Committed> {
        let what = c.name.clone().unwrap_or_else(|| "the capture".into());
        ensure!(c.bytes.len() <= crate::data::LIMIT, "{}", too_large(&what));
        ensure!(
            !c.bytes.is_empty() || c.allow_empty,
            "{what} is empty (0 bytes), so nothing was captured; if the command that \
             produced it failed, fix that first, or pass --allow-empty to record empty data"
        );
        let media_type = c
            .media_type
            .unwrap_or_else(|| crate::data::guess_media_type(c.name.as_deref(), &c.bytes));
        let sha256 = self.store_blob(&c.bytes)?;
        let mut existing = None;
        let mut committed = self.commit_planned(
            |s| {
                let same = s.objects.iter().find(|e| {
                    let r = &e.record;
                    !r.archived
                        && r.title == c.title
                        && r.body == c.note
                        && matches!(&r.data, Data::Captured { origin, media_type: m, sha256: h, .. }
                            if *origin == c.origin && *m == media_type && *h == sha256)
                });
                if let Some(e) = same {
                    existing = Some(Written {
                        changed: false,
                        ..Written::of(e)
                    });
                    return Ok(vec![]);
                }
                let mut r = Record::new(
                    c.title,
                    Data::Captured {
                        origin: c.origin,
                        captured_at: String::new(),
                        media_type,
                        size: 0,
                        sha256,
                    },
                );
                r.body = c.note;
                Ok(vec![Change::Create {
                    record: r,
                    expected: None,
                }])
            },
            None,
        )?;
        if let Some(w) = existing {
            committed.written = vec![w];
        }
        Ok(committed)
    }
    /// `hyp evidence attach`: stores the file's bytes and makes evidence `id`
    /// reference a data record holding them: one it already references;
    /// else, when a legacy attachment holds these bytes, the record the
    /// migration makes for them (`migrated_id`); else an existing one with the
    /// same bytes (not archived); else a new one (title: the file name;
    /// origin: the path as given). Decided under the write lock, so
    /// concurrent attaches of the same bytes share one record. Lists the
    /// evidence, then the data record. When the evidence holds the bytes
    /// already (by a data reference, or as a legacy attachment), nothing is
    /// written and the evidence keeps its `updated_at`; a legacy attachment
    /// has no data record yet, so then only the evidence is listed.
    pub fn attach(&self, id: &str, path: &Path) -> Result<Committed> {
        let file = File::open(path).with_context(|| format!("cannot read {}", path.display()))?;
        let bytes = read_capped(file, &path.display().to_string())?;
        let sha256 = self.store_blob(&bytes)?;
        let mut data_id = String::new();
        let mut evidence_id = String::new();
        let mut c = self.commit_planned(
            |s| {
                let e = s.find(id)?;
                ensure!(
                    matches!(e.record.data, Data::Evidence { .. }),
                    "attachments require evidence; {} is {} {}",
                    e.record.id,
                    article(e.record.data.kind()),
                    e.record.data.kind()
                );
                evidence_id.clone_from(&e.record.id);
                // It holds these bytes already, as a legacy attachment (hyp
                // 0.2.0's attach): a no-op, as it was, so no data record yet.
                if matches!(&e.record.data, Data::Evidence { attachments, .. }
                    if attachments.iter().any(|a| a.sha256 == sha256))
                {
                    return Ok(vec![]);
                }
                let holds = |d: &Entry| matches!(&d.record.data, Data::Captured { sha256: h, .. } if *h == sha256);
                let legacy = s.objects.iter().any(|x| {
                    matches!(&x.record.data, Data::Evidence { attachments, .. }
                        if attachments.iter().any(|a| a.sha256 == sha256))
                });
                let mut record = e.record.clone();
                let mut changes = Vec::new();
                let referenced = record
                    .data_refs
                    .iter()
                    .filter_map(|d| s.get(d))
                    .find(|d| holds(d))
                    .map(|d| d.record.id.clone());
                data_id = match referenced {
                    Some(d) => d,
                    None => {
                        let reused = s
                            .objects
                            .iter()
                            .find(|d| !d.record.archived && holds(d))
                            .map(|d| d.record.id.clone());
                        let d = match (legacy, reused) {
                            // The migration in this same write creates it.
                            (true, _) => migrated_id(&sha256),
                            (false, Some(d)) => d,
                            (false, None) => {
                                let name =
                                    path.file_name().map(|n| n.to_string_lossy().into_owned());
                                let d = Record::new(
                                    name.clone().unwrap_or_else(|| path.display().to_string()),
                                    Data::Captured {
                                        origin: path.display().to_string(),
                                        captured_at: String::new(),
                                        media_type: crate::data::guess_media_type(
                                            name.as_deref(),
                                            &bytes,
                                        ),
                                        size: 0,
                                        sha256: sha256.clone(),
                                    },
                                );
                                let id = d.id.clone();
                                changes.push(Change::Create {
                                    record: d,
                                    expected: None,
                                });
                                id
                            }
                        };
                        record.data_refs.push(d.clone());
                        d
                    }
                };
                changes.push(Change::Update {
                    record,
                    expected_revision: e.revision.clone(),
                });
                Ok(changes)
            },
            None,
        )?;
        // The evidence first (the contract of hyp 0.2.0), then the data record.
        if data_id.is_empty() {
            let e = c
                .snapshot
                .get(&evidence_id)
                .with_context(|| format!("{evidence_id} is missing after the write"))?;
            c.written = vec![Written {
                changed: false,
                ..Written::of(e)
            }];
            return Ok(c);
        }
        // Created by this write: by the attach, or by the migration within it.
        let created = c.written.iter().chain(&c.migrated).any(|w| w.id == data_id);
        c.written.retain(|w| w.id != data_id);
        c.migrated.retain(|w| w.id != data_id);
        let data = c
            .snapshot
            .get(&data_id)
            .with_context(|| format!("{data_id} is missing after the write"))?;
        let written = Written {
            changed: created,
            ..Written::of(data)
        };
        c.written.push(written);
        Ok(c)
    }
}
/// The ID the schema-3 migration gives the data record for stored bytes with
/// hash `sha256`: a name-based UUID (version 5) of the hash in hyp's own
/// namespace, so every copy of a notebook derives the same one.
pub fn migrated_id(sha256: &str) -> String {
    /// hyp's namespace for migrated attachments; fixed forever, as the IDs
    /// derived from it are stored.
    const NAMESPACE: uuid::Uuid = uuid::uuid!("5c1f4e0a-8d2b-4b7e-9a36-0c0d3a1e7f21");
    let uuid = uuid::Uuid::new_v5(&NAMESPACE, sha256.as_bytes());
    format!("{}-{uuid}", Kind::Data.prefix())
}
/// The bytes to capture, and what to record about them (`Store::capture`).
pub struct Capture {
    pub bytes: Vec<u8>,
    /// One line.
    pub title: String,
    /// Where the bytes came from (`Data::Captured::origin`).
    pub origin: String,
    /// None: guessed from `name` and the bytes (`data::guess_media_type`).
    pub media_type: Option<String>,
    /// The name of the file the bytes came from, for the guess and errors.
    pub name: Option<String>,
    /// The record's body: an optional note.
    pub note: String,
    /// Record empty bytes instead of refusing them.
    pub allow_empty: bool,
}
fn too_large(what: &str) -> String {
    format!(
        "{what} exceeds {} MiB, the most one capture may hold",
        crate::data::LIMIT / (1024 * 1024)
    )
}
/// All of `reader` (a file, or stdin for `hyp capture -`), unless it holds
/// more than `data::LIMIT` bytes; `what` names it in the error.
pub fn read_capped(reader: impl std::io::Read, what: &str) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut bytes = Vec::new();
    reader
        .take(crate::data::LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("cannot read {what}"))?;
    ensure!(bytes.len() <= crate::data::LIMIT, "{}", too_large(what));
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// HYPO-0004: a commit reads the notebook fully twice, before and after
    /// writing; its last-moment check for files changed meanwhile only
    /// hashes the record files.
    #[test]
    fn a_commit_reads_the_notebook_fully_twice() {
        let dir = tempfile::TempDir::new().unwrap();
        let store = Store::init(dir.path()).unwrap();
        let hypothesis = || {
            Record::new(
                "A claim",
                Data::Hypothesis {
                    scope: String::new(),
                    assumptions: String::new(),
                    lifecycle: Lifecycle::Draft,
                    untestable_reason: String::new(),
                },
            )
        };
        let create = |r: Record| Change::Create {
            record: r,
            expected: None,
        };
        store.commit(vec![create(hypothesis())], None).unwrap();
        FULL_READS.with(|n| n.set(0));
        store.commit(vec![create(hypothesis())], None).unwrap();
        assert_eq!(FULL_READS.with(std::cell::Cell::get), 2);
    }
    /// `hyp check` hashes stored bytes after letting go of its lock: a file
    /// that went missing between the read and the hashing is reported as
    /// missing (blocking writes), one changed meanwhile as changed.
    #[test]
    fn bytes_hashed_after_the_read_are_reported_as_they_are_then() {
        let dir = tempfile::TempDir::new().unwrap();
        let store = Store::init(dir.path()).unwrap();
        let capture = |bytes: &[u8]| {
            store
                .capture(Capture {
                    bytes: bytes.to_vec(),
                    title: "Log".into(),
                    origin: "here".into(),
                    media_type: None,
                    name: None,
                    note: String::new(),
                    allow_empty: false,
                })
                .unwrap();
            store.root.join("hyp/assets").join(hash(bytes))
        };
        let (gone, changed) = (capture(b"first"), capture(b"other"));
        let (mut snap, _) = store.read_unlocked().unwrap();
        let revision = snap.revision.clone();
        fs::remove_file(&gone).unwrap();
        fs::write(&changed, "OTHER").unwrap();
        store.hash_stored(&mut snap);
        let codes: Vec<Code> = snap.diagnostics.iter().map(|d| d.code).collect();
        assert!(codes.contains(&Code::Attachment), "{codes:?}");
        assert!(codes.contains(&Code::ChangedBytes), "{codes:?}");
        let missing = snap
            .diagnostics
            .iter()
            .find(|d| d.code == Code::Attachment)
            .unwrap();
        assert!(missing.message.contains("missing"), "{}", missing.message);
        assert!(missing.blocks_writes);
        assert_ne!(
            snap.revision, revision,
            "the revision covers the diagnostics"
        );
    }
    fn config(schema_version: u32) -> Config {
        Config {
            schema_version,
            name: "n".into(),
            min_hyp_version: None,
        }
    }
    /// decision-0004: from schema 3 on, a raise names the hyp that reads the
    /// schema, and before it does not, so hyp 0.1.0 keeps its message.
    #[test]
    fn a_raise_names_the_hyp_version_from_schema_3_on() {
        let schemas = [(1, "0.1.0"), (2, "0.2.0"), (3, "0.3.0"), (4, "0.5.0")];
        assert_eq!(raised(config(1), 2, &schemas).unwrap(), config(2));
        let three = raised(config(2), 3, &schemas).unwrap();
        assert_eq!(three.min_hyp_version.as_deref(), Some("0.3.0"));
        let four = raised(three, 4, &schemas).unwrap();
        assert_eq!(four.min_hyp_version.as_deref(), Some("0.5.0"));
        assert!(raised(config(1), 5, &schemas).is_err());
        // It is left out of config.toml while absent, and read back when present.
        assert!(
            !toml::to_string_pretty(&config(2))
                .unwrap()
                .contains("min_hyp_version")
        );
        let text = toml::to_string_pretty(&four).unwrap();
        assert_eq!(toml::from_str::<Config>(&text).unwrap(), four);
    }
    /// decision-0004 for schema 3 (HYPO-0090): a notebook raised by a real
    /// capture is refused up front by a hyp that reads only schemas 1 and 2
    /// (hyp 0.2.0, played by `read_config_of` with the first two rows), naming
    /// the version the raise wrote; this hyp reads it.
    #[test]
    fn a_hyp_that_reads_schema_2_refuses_a_notebook_raised_by_a_capture() {
        let dir = tempfile::TempDir::new().unwrap();
        let store = Store::init(dir.path()).unwrap();
        let hyp = store.root.join("hyp");
        let older = &SCHEMAS[..2];
        read_config_of(&hyp, older).unwrap();
        store
            .capture(Capture {
                bytes: b"log".to_vec(),
                title: "Log".into(),
                origin: "here".into(),
                media_type: None,
                name: None,
                note: String::new(),
                allow_empty: false,
            })
            .unwrap();
        let err = read_config_of(&hyp, older).unwrap_err();
        assert_eq!(kind_of(&err), ErrorKind::UnsupportedSchema);
        let message = err.to_string();
        for part in [
            "uses schema 3",
            "reads schemas 1 to 2",
            "upgrade hyp to >= 0.3.0",
        ] {
            assert!(message.contains(part), "{part:?} in {message}");
        }
        assert_eq!(read_config(&hyp).unwrap().schema_version, 3);
    }
}
