use crate::{
    error::{Classified, ErrorKind, kind_of},
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
    "data",
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
    /// The records the write converted, beyond those its changes named:
    /// the data records it created from legacy evidence attachments and the
    /// evidence it made reference them (decision-0005); usually empty.
    pub migrated: Vec<Written>,
    /// The schema the write raised the notebook to, if it did.
    pub raised_to: Option<u32>,
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
/// does. The README's schema table lists the same rows. From schema 3 on
/// a raise also writes the row's version to config.toml (`raised`).
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
    /// The notebook's config; fails for a schema this hyp does not read.
    fn config(&self) -> Result<Config> {
        read_config(&self.root.join("hyp"))
    }
    fn read_unlocked(&self) -> Result<Snapshot> {
        // Another hyp may have raised the schema since `open` (a long-running
        // `hyp web`): refuse rather than report its records as malformed.
        self.config()?;
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
        // Each stored file is read and hashed once, however many data
        // records name it: its length and start, or why it is not intact.
        let mut blobs: BTreeMap<String, std::result::Result<(usize, Vec<u8>), String>> =
            BTreeMap::new();
        let mut previews = BTreeMap::new();
        for entry in &snap.objects {
            for v in snap.validate(&entry.record) {
                let mut d = Diagnostic::new(Self::path_of(&entry.record), v.code, v.message);
                d.repair = v.repair;
                snap.diagnostics.push(d);
            }
            if let Data::Evidence { attachments, .. } = &entry.record.data {
                for a in attachments {
                    if let Err(e) = self.attachment_bytes(a) {
                        let mut d = Diagnostic::new(
                            &a.path,
                            Code::Attachment,
                            format!("attachment missing, unsafe, or hash mismatch: {e:#}"),
                        );
                        d.repair = Some(Repair {
                            note: Some(format!(
                                "Restore hyp/{} as a regular file with the recorded bytes \
                                 (sha256 {}), from a backup, version control or the source of \
                                 the merge or sync; a symlink there must be replaced by a copy \
                                 of what it points to. hyp check confirms the fix.",
                                a.path, a.sha256
                            )),
                            commands: vec![],
                        });
                        snap.diagnostics.push(d);
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
                    let stored = blobs.entry(sha256.clone()).or_insert_with(|| {
                        // The length and enough of the start for a preview.
                        let keep = crate::data::PREVIEW_BYTES + 1;
                        self.blob(sha256)
                            .map(|b| (b.len(), b[..b.len().min(keep)].to_vec()))
                            .map_err(|e| format!("{e:#}"))
                    });
                    let id = &entry.record.id;
                    let path = Self::path_of(&entry.record);
                    let d = match stored {
                        // Missing or changed bytes: restore them.
                        Err(why) => {
                            let mut d = Diagnostic::new(
                                path,
                                Code::Attachment,
                                format!("the bytes of data record {id} are not intact: {why}"),
                            );
                            d.repair = Some(Repair {
                                note: Some(format!(
                                    "Restore hyp/assets/{sha256} ({size} bytes, {media_type}) as a \
                                     regular file from a backup, version control or the source \
                                     of the merge or sync; or capture the original again (hyp \
                                     capture FILE --origin \"...\"), which stores the same bytes \
                                     at the same path. hyp check confirms the fix."
                                )),
                                commands: vec![],
                            });
                            Some(d)
                        }
                        // Intact bytes, but the record describes them wrongly:
                        // the record file is what changed.
                        Ok((len, _)) if *len as u64 != *size => {
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
                        Ok((_, start)) => {
                            if media_type.starts_with("text/") {
                                previews.insert(id.clone(), crate::data::preview(start));
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
                        &entry.record.id,
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
        let entries = fs::read_dir(&assets)
            .and_then(|entries| entries.collect::<std::io::Result<Vec<_>>>())
            .with_context(|| format!("cannot read {}", assets.display()))?;
        for entry in entries {
            let shown = format!("hyp/assets/{}", entry.file_name().to_string_lossy());
            let name = entry.file_name().to_string_lossy().into_owned();
            let reported = blobs.get(&name).is_some_and(|b| b.is_err())
                || snap
                    .diagnostics
                    .iter()
                    .any(|d| format!("hyp/{}", d.path) == shown && d.code == Code::Attachment);
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
        let before = self.read_unlocked()?;
        before.assert_writable()?;
        if let Some(expected) = expected_project {
            if expected != before.revision {
                bail!(Conflict::new("project changed; reload and retry"));
            }
        }
        let changes = plan(&before)?;
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
    /// The stored bytes with hash `sha256`, checked: a regular file (not a
    /// symlink) at `hyp/assets/<sha256>` whose content has that hash.
    pub fn blob(&self, sha256: &str) -> Result<Vec<u8>> {
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
        let bytes = fs::read(&path).with_context(|| format!("cannot read {shown}"))?;
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
    /// The bytes of a legacy attachment, checked: a regular file (not a
    /// symlink) inside `hyp/assets/` with the recorded hash. `read_unlocked`
    /// reports an attachment that fails this, and the schema-3 migration
    /// reads its bytes through it.
    fn attachment_bytes(&self, a: &Attachment) -> Result<Vec<u8>> {
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
        let bytes = fs::read(&path)?;
        ensure!(
            hash(&bytes) == a.sha256,
            "attachment {} does not match its hash",
            a.path
        );
        Ok(bytes)
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
