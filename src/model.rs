use crate::error::{Classified, ErrorKind};
use anyhow::{Result, bail, ensure};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// An enum of fixed values. `as_str` (and `Display`) is the spelling people
/// read and type, the clap value name; files and JSON use serde's
/// snake_case, which differs only for multi-word values (a relation's
/// `competes-with` is stored as `competes_with`).
macro_rules! values {
    ($name:ident { $($variant:ident => $text:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }
        impl $name {
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $text),+ }
            }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    }
}
values!(Lifecycle { Draft => "draft", Investigating => "investigating", Paused => "paused", Closed => "closed" });
values!(Judgment { Untested => "untested", Inconclusive => "inconclusive", Supported => "supported", Weakened => "weakened", Falsified => "falsified" });
impl Judgment {
    /// What an assessment with this judgment needs besides its rationale
    /// (decision-0003, enforced by `Snapshot::validate_new`).
    pub fn requirement(self) -> &'static str {
        match self {
            Self::Untested => "no evidence needed",
            Self::Inconclusive | Self::Supported | Self::Weakened => {
                "at least one evidence ID linked to the hypothesis or its active criteria \
                 or predictions"
            }
            Self::Falsified => {
                "a falsification criterion of the hypothesis and linked evidence that \
                 meets it (an active supports link from the evidence to that criterion)"
            }
        }
    }
    /// Every judgment's `requirement`, judgments with the same one together:
    /// "untested: …; inconclusive, supported, weakened: …; falsified: …".
    pub fn rule() -> String {
        let mut groups: Vec<(Vec<&str>, &str)> = Vec::new();
        for j in <Self as clap::ValueEnum>::value_variants() {
            match groups.iter_mut().find(|(_, r)| *r == j.requirement()) {
                Some((names, _)) => names.push(j.as_str()),
                None => groups.push((vec![j.as_str()], j.requirement())),
            }
        }
        groups
            .iter()
            .map(|(names, r)| format!("{}: {r}", names.join(", ")))
            .collect::<Vec<_>>()
            .join("; ")
    }
}
values!(Relation { Supports => "supports", Contradicts => "contradicts", Qualifies => "qualifies", DependsOn => "depends-on", CompetesWith => "competes-with", Supersedes => "supersedes" });
values!(ExperimentStatus { Planned => "planned", Running => "running", Completed => "completed", Cancelled => "cancelled" });
values!(Outcome { Observed => "observed", Inconclusive => "inconclusive", Failed => "failed" });
/// What a create that omits them gets, as the CLI sets them (HYPO-0053): a
/// new hypothesis is a draft, a new experiment is planned, a run observed.
/// Serde defaults, so a stored file without the field reads the same way.
pub fn new_lifecycle() -> Lifecycle {
    Lifecycle::Draft
}
pub fn new_experiment_status() -> ExperimentStatus {
    ExperimentStatus::Planned
}
pub fn new_outcome() -> Outcome {
    Outcome::Observed
}
values!(Kind { Hypothesis => "hypothesis", Prediction => "prediction", Criterion => "criterion", Evidence => "evidence", Link => "link", Experiment => "experiment", Run => "run", Assessment => "assessment", Gap => "gap", Data => "data" });
impl Kind {
    pub fn prefix(self) -> &'static str {
        match self {
            Self::Hypothesis => "H",
            Self::Prediction => "P",
            Self::Criterion => "F",
            Self::Evidence => "E",
            Self::Link => "L",
            Self::Experiment => "X",
            Self::Run => "R",
            Self::Assessment => "A",
            Self::Gap => "G",
            Self::Data => "D",
        }
    }
    /// The kind a full ID (`<prefix>-<UUID>`) names; None for anything else,
    /// such as a short ID.
    pub fn of_id(id: &str) -> Option<Self> {
        let (prefix, uuid) = id.split_once('-')?;
        uuid::Uuid::parse_str(uuid).ok()?;
        <Self as clap::ValueEnum>::value_variants()
            .iter()
            .copied()
            .find(|k| k.prefix() == prefix)
    }
}

/// A copy of a record as it was when an experiment or run was created. On a
/// create only `id` is input (an experiment target); the server fills in the
/// rest, so those fields default to empty on input.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenRef {
    pub id: String,
    #[serde(default)]
    pub revision: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub body: String,
}
/// A file copied into `hyp/assets/` and attached to evidence by hyp 0.2.0
/// and earlier. Read from notebooks of schema 1 and 2; a write that raises
/// a notebook to schema 3 turns each into a data record (decision-0005,
/// `Store::commit_written`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attachment {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Data {
    Hypothesis {
        #[serde(default)]
        scope: String,
        #[serde(default)]
        assumptions: String,
        #[serde(default = "new_lifecycle")]
        lifecycle: Lifecycle,
        #[serde(default)]
        untestable_reason: String,
    },
    Prediction {
        hypothesis: String,
        #[serde(default)]
        conditions: String,
    },
    Criterion {
        hypothesis: String,
    },
    Evidence {
        source: String,
        #[serde(default)]
        locator: String,
        #[serde(default)]
        observed_at: String,
        /// Legacy (`Attachment`); not written while empty.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        attachments: Vec<Attachment>,
    },
    Link {
        from: String,
        to: String,
        relation: Relation,
    },
    Experiment {
        hypothesis: String,
        #[serde(default)]
        targets: Vec<FrozenRef>,
        #[serde(default = "new_experiment_status")]
        status: ExperimentStatus,
    },
    Run {
        experiment: String,
        #[serde(default)]
        plan: FrozenRef,
        #[serde(default = "new_outcome")]
        outcome: Outcome,
        #[serde(default)]
        evidence: Vec<String>,
    },
    Assessment {
        hypothesis: String,
        judgment: Judgment,
        #[serde(default)]
        confidence: Option<f64>,
        #[serde(default)]
        evidence: Vec<String>,
        #[serde(default)]
        criterion: Option<String>,
        #[serde(default)]
        based_on: String,
        #[serde(default)]
        supersedes: Vec<String>,
    },
    Gap {
        hypothesis: String,
        #[serde(default)]
        resolved: bool,
        /// The evidence that answered the question (HYPO-0076); only on a
        /// resolved gap. Needs schema 2 (`Data::schema`); not written while
        /// empty, so a notebook without it stays schema 1.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        resolved_by: Vec<String>,
    },
    /// Captured bytes (decision-0005, kind `data`, ID prefix `D-`): stored
    /// once per hash at `hyp/assets/<sha256>`, immutable once captured. The
    /// body is an optional note. `captured_at`, `size` and, when not given,
    /// `media_type` are set by the server from the stored bytes, so a create
    /// may leave them out. Needs schema 3.
    #[serde(rename = "data")]
    Captured {
        /// Where the bytes came from, as text: a path, URL, host or the
        /// command that produced them (hyp never runs it).
        origin: String,
        #[serde(default)]
        captured_at: String,
        #[serde(default)]
        media_type: String,
        #[serde(default)]
        size: u64,
        sha256: String,
    },
}
impl Data {
    pub fn kind(&self) -> &'static str {
        self.kind_value().as_str()
    }
    pub fn kind_value(&self) -> Kind {
        match self {
            Self::Hypothesis { .. } => Kind::Hypothesis,
            Self::Prediction { .. } => Kind::Prediction,
            Self::Criterion { .. } => Kind::Criterion,
            Self::Evidence { .. } => Kind::Evidence,
            Self::Link { .. } => Kind::Link,
            Self::Experiment { .. } => Kind::Experiment,
            Self::Run { .. } => Kind::Run,
            Self::Assessment { .. } => Kind::Assessment,
            Self::Gap { .. } => Kind::Gap,
            Self::Captured { .. } => Kind::Data,
        }
    }
    pub fn directory(&self) -> &'static str {
        match self {
            Self::Hypothesis { .. } => "hypotheses",
            Self::Prediction { .. } => "predictions",
            Self::Criterion { .. } => "criteria",
            Self::Evidence { .. } => "evidence",
            Self::Link { .. } => "links",
            Self::Experiment { .. } => "experiments",
            Self::Run { .. } => "runs",
            Self::Assessment { .. } => "assessments",
            Self::Gap { .. } => "gaps",
            Self::Captured { .. } => "data",
        }
    }
    pub fn prefix(&self) -> &'static str {
        self.kind_value().prefix()
    }
    /// The first notebook schema (`store::SCHEMAS`) whose readers can load
    /// these fields: 3 for a data record, 2 for a gap with `resolved_by`,
    /// otherwise 1. `Record::schema` adds the header's.
    pub fn schema(&self) -> u32 {
        match self {
            Self::Captured { .. } => 3,
            Self::Gap { resolved_by, .. } if !resolved_by.is_empty() => 2,
            _ => 1,
        }
    }
    pub fn owner(&self) -> Option<&str> {
        match self {
            Self::Prediction { hypothesis, .. }
            | Self::Criterion { hypothesis }
            | Self::Experiment { hypothesis, .. }
            | Self::Assessment { hypothesis, .. }
            | Self::Gap { hypothesis, .. } => Some(hypothesis),
            _ => None,
        }
    }
    /// The IDs these fields name. `Record::references` adds the header's
    /// data references; use that for a whole record.
    pub fn references(&self) -> Vec<&str> {
        let mut refs = self.owner().into_iter().collect::<Vec<_>>();
        match self {
            Self::Link { from, to, .. } => refs.extend([from.as_str(), to.as_str()]),
            Self::Experiment { targets, .. } => refs.extend(targets.iter().map(|t| t.id.as_str())),
            Self::Run {
                experiment,
                evidence,
                ..
            } => {
                refs.push(experiment);
                refs.extend(evidence.iter().map(String::as_str));
            }
            Self::Assessment {
                evidence,
                criterion,
                supersedes,
                ..
            } => {
                refs.extend(
                    evidence
                        .iter()
                        .chain(criterion.iter())
                        .chain(supersedes.iter())
                        .map(String::as_str),
                );
            }
            Self::Gap { resolved_by, .. } => refs.extend(resolved_by.iter().map(String::as_str)),
            _ => {}
        }
        refs
    }
    /// The IDs `references` returns, in the same order, to rewrite them: how
    /// `hyp apply` replaces batch-local references with full IDs.
    pub fn references_mut(&mut self) -> Vec<&mut String> {
        use std::iter::once;
        match self {
            Self::Hypothesis { .. } | Self::Evidence { .. } | Self::Captured { .. } => vec![],
            Self::Prediction { hypothesis, .. } | Self::Criterion { hypothesis } => {
                vec![hypothesis]
            }
            Self::Link { from, to, .. } => vec![from, to],
            Self::Experiment {
                hypothesis,
                targets,
                ..
            } => once(hypothesis)
                .chain(targets.iter_mut().map(|t| &mut t.id))
                .collect(),
            Self::Run {
                experiment,
                evidence,
                ..
            } => once(experiment).chain(evidence.iter_mut()).collect(),
            Self::Assessment {
                hypothesis,
                evidence,
                criterion,
                supersedes,
                ..
            } => once(hypothesis)
                .chain(evidence.iter_mut())
                .chain(criterion.iter_mut())
                .chain(supersedes.iter_mut())
                .collect(),
            Self::Gap {
                hypothesis,
                resolved_by,
                ..
            } => once(hypothesis).chain(resolved_by.iter_mut()).collect(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    #[serde(default)]
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub body: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// The data records (`D-` IDs) this record draws on (decision-0005): any
    /// kind but data itself may name some, and one data record may be named
    /// by many. Stored as `data`; left out while empty, as it needs schema 3.
    #[serde(rename = "data", default, skip_serializing_if = "Vec::is_empty")]
    pub data_refs: Vec<String>,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(flatten)]
    pub data: Data,
}
impl Record {
    pub fn new(title: impl Into<String>, data: Data) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            id: format!("{}-{}", data.prefix(), uuid::Uuid::new_v4()),
            title: title.into(),
            body: String::new(),
            tags: vec![],
            data_refs: vec![],
            archived: false,
            created_at: now.clone(),
            updated_at: now,
            data,
        }
    }
    /// Every ID this record names: those of its kind's fields
    /// (`Data::references`), then its data references.
    pub fn references(&self) -> Vec<&str> {
        let mut refs = self.data.references();
        refs.extend(self.data_refs.iter().map(String::as_str));
        refs
    }
    /// The IDs `references` returns, in the same order, to rewrite them: how
    /// `hyp apply` replaces batch-local references with full IDs.
    pub fn references_mut(&mut self) -> Vec<&mut String> {
        let mut refs = self.data.references_mut();
        refs.extend(self.data_refs.iter_mut());
        refs
    }
    /// The first notebook schema whose readers can load this record's file:
    /// `Data::schema`, and 3 while it has data references.
    pub fn schema(&self) -> u32 {
        let header = if self.data_refs.is_empty() { 1 } else { 3 };
        self.data.schema().max(header)
    }
    /// Whether this record's kind cannot be changed once created:
    /// assessments and runs are history, data records captured bytes.
    pub fn is_immutable(&self) -> bool {
        matches!(
            self.data,
            Data::Assessment { .. } | Data::Run { .. } | Data::Captured { .. }
        )
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub record: Record,
    pub revision: String,
}
impl Entry {
    pub fn frozen(&self) -> FrozenRef {
        FrozenRef {
            id: self.record.id.clone(),
            revision: self.revision.clone(),
            title: self.record.title.clone(),
            body: serde_json::to_string_pretty(&self.record).unwrap_or_default(),
        }
    }
}
values!(Code {
    Malformed => "malformed",
    Attachment => "attachment",
    ChangedBytes => "changed_bytes",
    Invalid => "invalid",
    DanglingReference => "dangling_reference",
    Cycle => "cycle",
    Inconsistent => "inconsistent",
    NoCriterion => "no_criterion",
});
impl Code {
    pub fn severity(self) -> &'static str {
        match self {
            Self::NoCriterion => "warning",
            _ => "error",
        }
    }
    /// Whether this problem blocks every write. hyp cannot load a malformed
    /// file (it may hold a record other records depend on) and cannot trust
    /// stored bytes that are missing or changed as every read sees it
    /// (`attachment`: a data record's or a legacy attachment's, missing, not
    /// a regular file, or of another length); an invalid record breaks rules of its own fields,
    /// including naming a record by a short ID or one of the wrong kind (the
    /// ID prefix gives the kind). An invalid record does not block its own
    /// repair through hyp: a write that changes only invalid records and
    /// leaves each valid (`Store::repair_scope`). The other errors are between loaded
    /// records, as a merge, sync or hand edit leaves them; writes that add no
    /// new error may repair them (see `Store::commit_written`).
    /// `changed_bytes` (stored bytes changed in place to bytes of the same
    /// length) is found only by `hyp check`, which hashes them; ordinary
    /// reads, and so writes, do not see it. It blocks only writes that newly
    /// cite those bytes and assessments whose basis holds them
    /// (`Store::verify_cited`, `Store::verify_basis`), so it does not claim
    /// to block every write.
    pub fn blocks_writes(self) -> bool {
        matches!(self, Self::Malformed | Self::Attachment | Self::Invalid)
    }
}
/// How to remove a problem. `note` comes first: where it names a lossless
/// fix (restoring a record), prefer that over the commands. Each command is
/// an argv array, run in the project directory (hyp finds the project from
/// there, as without `--project`), in order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Repair {
    pub note: Option<String>,
    pub commands: Vec<Vec<String>>,
}
/// A problem `hyp check` reports: one per rule a record breaks.
/// `(path, code)` identifies it across reads; the message is for people and
/// may change. `path` is relative to the project root, always under `hyp/`:
/// the record's file (`hyp/<directory>/<id>.md`), or for stored bytes no
/// record file stands for (a legacy attachment, a stray symlink) the file
/// under `hyp/assets/`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub path: String,
    pub message: String,
    pub severity: String,
    pub code: Code,
    pub blocks_writes: bool,
    pub repair: Option<Repair>,
}
impl Diagnostic {
    pub fn new(path: impl Into<String>, code: Code, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
            severity: code.severity().into(),
            code,
            blocks_writes: code.blocks_writes(),
            repair: None,
        }
    }
    /// The diagnostic of `v`, a rule the record at `path` breaks.
    pub fn of(path: impl Into<String>, v: Violation) -> Self {
        Self {
            repair: v.repair,
            ..Self::new(path, v.code, v.message)
        }
    }
    pub fn identity(&self) -> (&str, Code) {
        (&self.path, self.code)
    }
}
/// A rule that `Snapshot::validate` found record-wise broken; with the
/// record's path, a `Diagnostic` (`Diagnostic::of`).
#[derive(Debug)]
pub struct Violation {
    pub code: Code,
    pub message: String,
    pub repair: Option<Repair>,
}
/// The violations of one record, in the order the rules are checked.
#[derive(Default)]
struct Violations(Vec<Violation>);
impl Violations {
    fn require(&mut self, ok: bool, code: Code, message: impl FnOnce() -> String) {
        if !ok {
            self.push(code, message(), None);
        }
    }
    fn push(&mut self, code: Code, message: String, repair: Option<Repair>) {
        self.0.push(Violation {
            code,
            message,
            repair,
        });
    }
}
/// The repair of record `r`'s reference to `missing`, which does not exist
/// (`exists` tells which records do). Restoring it is lossless, and after a
/// partial sync it may only be late, so that comes first. Only a link, which
/// nothing references and which holds no content of its own beyond its
/// explanation, is offered deletion; a gap resolved by the missing evidence
/// is offered to drop it: resolved by the evidence that remains, or, with
/// none, open again.
fn dangling_repair(r: &Record, missing: &str, exists: impl Fn(&str) -> bool) -> Repair {
    let restore = format!(
        "Restore {missing} from the source of the merge or sync: that loses nothing, \
         and after a partial sync it may still be arriving."
    );
    let hyp = |args: &[&str]| {
        let mut argv = vec!["hyp".to_string()];
        argv.extend(args.iter().map(|a| a.to_string()));
        argv
    };
    let (what, commands) = match &r.data {
        Data::Link { .. } => {
            let mut commands = vec![hyp(&["archive", &r.id]), hyp(&["delete", &r.id])];
            if r.archived {
                commands.remove(0);
            }
            (
                "remove this link with the commands; a delete cannot be undone without \
                 version control",
                commands,
            )
        }
        Data::Gap { resolved_by, .. } if resolved_by.iter().any(|id| id == missing) => {
            let remaining: Vec<&str> = resolved_by
                .iter()
                .map(String::as_str)
                .filter(|id| exists(id))
                .collect();
            match remaining.as_slice() {
                [] => (
                    "reopen the gap with the command (nothing else answers it)",
                    vec![hyp(&["set", &r.id, "--resolved", "false"])],
                ),
                _ => (
                    "keep the gap resolved by the evidence that remains with the command",
                    vec![hyp(&["set", &r.id, "--by", &remaining.join(",")])],
                ),
            }
        }
        _ => {
            return Repair {
                note: Some(restore),
                commands: vec![],
            };
        }
    };
    Repair {
        note: Some(format!("{restore} Only if it is gone for good, {what}.")),
        commands,
    }
}
/// The repair of an invalid record `r` (`Code::Invalid`: it breaks rules
/// of its own fields). hyp accepts a write that repairs it while it blocks
/// other writes (`Store::repair_scope`), except for kinds it never changes.
fn invalid_repair(r: &Record) -> Repair {
    let note = if r.is_immutable() {
        format!(
            "hyp never changes {} {} record: restore {} from version control or the source \
             of the merge or sync, or fix it by hand. hyp check confirms the fix.",
            article(r.data.kind()),
            r.data.kind(),
            r.id
        )
    } else {
        format!(
            "Fix it with hyp set {id} … (or a patch in hyp apply, or hyp edit {id}), or restore \
             it from version control. Until then hyp accepts only writes that change invalid \
             records and leave each of them valid. hyp check confirms the fix.",
            id = r.id
        )
    };
    Repair {
        note: Some(note),
        commands: vec![],
    }
}
/// Whether `value` is an `observed_at` hyp accepts: an RFC 3339 timestamp
/// (2026-09-12T14:03:00Z) or a date (2026-09-12). It is stored as given.
pub fn is_observed_at(value: &str) -> bool {
    let date = value.len() == 10
        && value.bytes().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
        && chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok();
    date || chrono::DateTime::parse_from_rfc3339(value).is_ok()
}
/// How far after `now` an `observed_at` may lie: a date is local to whoever
/// observed it, and clocks differ.
const OBSERVED_AT_TOLERANCE: chrono::TimeDelta = chrono::TimeDelta::days(1);
/// Whether `observed_at` (as `is_observed_at` accepts it) lies more than a
/// day after `now`: an observation cannot have been made yet. Checked when
/// it is written, not by `hyp check`, whose verdict must not depend on the
/// clock.
pub fn observed_in_future(observed_at: &str, now: chrono::DateTime<Utc>) -> bool {
    let latest = now + OBSERVED_AT_TOLERANCE;
    match chrono::DateTime::parse_from_rfc3339(observed_at) {
        Ok(t) => t > latest,
        Err(_) => chrono::NaiveDate::parse_from_str(observed_at, "%Y-%m-%d")
            .is_ok_and(|d| d > latest.date_naive()),
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HypothesisState {
    pub judgment: Judgment,
    pub confidence: Option<f64>,
    pub needs_review: bool,
    pub assessment_ids: Vec<String>,
    pub fingerprint: String,
    /// The fingerprint and the current assessments (IDs and revisions) as
    /// one value: what a new assessment states it reviewed
    /// (`hyp assess --reviewed`, `expected.hypotheses`).
    pub review_token: String,
}
/// What evidence means for a hypothesis. A link's relation is to the record
/// it targets; through a criterion it reverses: evidence that supports a
/// falsification criterion (the refuting observation was made) counts
/// against the hypothesis. A single link is never `Mixed`; for an
/// observation's links together see `Stance::combined`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stance {
    For,
    Against,
    Qualifies,
    Mixed,
}
impl Stance {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::For => "for",
            Self::Against => "against",
            Self::Qualifies => "qualifies",
            Self::Mixed => "mixed",
        }
    }
    /// The stance of an observation with links of these stances: `Mixed`
    /// when some count for and some against the hypothesis; otherwise the
    /// direction any of them has (qualifying links do not change it);
    /// `Qualifies` when all qualify.
    pub fn combined(stances: impl IntoIterator<Item = Self>) -> Self {
        let (mut for_h, mut against) = (false, false);
        for s in stances {
            match s {
                Self::For => for_h = true,
                Self::Against => against = true,
                Self::Mixed => (for_h, against) = (true, true),
                Self::Qualifies => {}
            }
        }
        match (for_h, against) {
            (true, true) => Self::Mixed,
            (true, false) => Self::For,
            (false, true) => Self::Against,
            (false, false) => Self::Qualifies,
        }
    }
    /// The stance of evidence linked by `relation` to a record of kind
    /// `target` (the hypothesis, one of its criteria or predictions), and
    /// what that link means in words, `id` being the target. None for a
    /// relation between hypotheses.
    pub fn of_link(target: Kind, id: &str, relation: Relation) -> Option<(Self, String)> {
        let short = id.get(..10).unwrap_or(id);
        let stance = match (target, relation) {
            (_, Relation::Qualifies) => Self::Qualifies,
            (Kind::Criterion, Relation::Supports) => Self::Against,
            (Kind::Criterion, Relation::Contradicts) => Self::For,
            (_, Relation::Supports) => Self::For,
            (_, Relation::Contradicts) => Self::Against,
            _ => return None,
        };
        let meaning = match (target, relation) {
            (Kind::Criterion, Relation::Supports) => {
                format!("meets criterion {short} (counts against H)")
            }
            (Kind::Criterion, Relation::Contradicts) => {
                format!("does not meet criterion {short} (counts for H)")
            }
            (Kind::Criterion, _) => format!("qualifies criterion {short}"),
            (Kind::Prediction, Relation::Supports) => format!("matches prediction {short}"),
            (Kind::Prediction, Relation::Contradicts) => format!("contradicts prediction {short}"),
            (Kind::Prediction, _) => format!("qualifies prediction {short}"),
            (_, relation) => format!("{relation} H"),
        };
        Some((stance, meaning))
    }
}
/// One evidence link to a hypothesis or its criterion or prediction, read
/// for the hypothesis. `meaning` is for people ("meets criterion F-… (counts
/// against H)") and may change; `stance`, `via` and `relation` are the data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bearing {
    pub link: String,
    /// The H-, P- or F- record the link targets.
    pub via: String,
    pub relation: Relation,
    pub stance: Stance,
    pub meaning: String,
}
/// One observation linked to a hypothesis, with every link that makes it
/// part of the hypothesis's basis; `stance` is `Stance::combined` of theirs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceBearing {
    pub evidence: String,
    pub stance: Stance,
    pub bearings: Vec<Bearing>,
}
/// The SHA-256 of a basis as compact JSON with sorted keys.
pub fn fingerprint_of(basis: &BTreeMap<String, serde_json::Value>) -> String {
    hash(serde_json::to_string(basis).expect("JSON values serialize"))
}
/// `HypothesisState::review_token`: a hash of the fingerprint and the sorted
/// `ID:revision` lines of the current assessments.
pub fn review_token(fingerprint: &str, assessments: &[&Entry]) -> String {
    let mut lines: Vec<String> = assessments
        .iter()
        .map(|e| format!("{}:{}", e.record.id, e.revision))
        .collect();
    lines.sort();
    hash(format!("{fingerprint}\n{}", lines.join("\n")))
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Snapshot {
    pub objects: Vec<Entry>,
    pub diagnostics: Vec<Diagnostic>,
    pub hypotheses: BTreeMap<String, HypothesisState>,
    /// Hypothesis ID -> `Snapshot::evidence_bearings`, for the WebUI and
    /// in `hyp export --format json`.
    #[serde(default)]
    pub bearings: BTreeMap<String, Vec<EvidenceBearing>>,
    /// Data record ID -> the start of its bytes as text (`data::preview`),
    /// for `text/*` media types with intact bytes only: what the WebUI
    /// shows, escaped. Derived on read, never stored, not in `revision`.
    #[serde(default)]
    pub previews: BTreeMap<String, String>,
    pub revision: String,
}
pub fn hash(bytes: impl AsRef<[u8]>) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
/// "a" or "an", as `word` needs.
pub fn article(word: &str) -> &'static str {
    if word.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    }
}
/// Entries for an error message, one `ID  kind  title` line each, at most
/// ten, then how many more there are.
pub fn listing(entries: &[&Entry]) -> String {
    const SHOWN: usize = 10;
    let mut lines: Vec<String> = entries
        .iter()
        .take(SHOWN)
        .map(|e| {
            let r = &e.record;
            format!("  {}  {:11} {}", r.id, r.data.kind(), r.title)
        })
        .collect();
    if entries.len() > SHOWN {
        lines.push(format!("  and {} more", entries.len() - SHOWN));
    }
    lines.join("\n")
}
impl Snapshot {
    /// The entry with exactly this ID; no prefix matching.
    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.objects.iter().find(|e| e.record.id == id)
    }
    /// The entry with this ID or unique ID prefix (case-insensitive). The
    /// error lists the candidates of an ambiguous prefix, and explains the
    /// kind letter IDs start with when a prefix without one matches nothing.
    pub fn find(&self, prefix: &str) -> Result<&Entry> {
        if let Some(e) = self.get(prefix) {
            return Ok(e);
        }
        let lower = prefix.to_lowercase();
        let hits: Vec<&Entry> = self
            .objects
            .iter()
            .filter(|e| e.record.id.to_lowercase().starts_with(&lower))
            .collect();
        match hits.as_slice() {
            [e] => Ok(e),
            [] => {
                let kinds = <Kind as clap::ValueEnum>::value_variants();
                let upper = prefix.to_uppercase();
                let has_kind = kinds
                    .iter()
                    .any(|k| upper == k.prefix() || upper.starts_with(&format!("{}-", k.prefix())));
                let hint = if has_kind {
                    String::new()
                } else {
                    let letters: Vec<String> = kinds
                        .iter()
                        .map(|k| format!("{}- {k}", k.prefix()))
                        .collect();
                    format!(
                        " (IDs start with their kind letter: {})",
                        letters.join(", ")
                    )
                };
                bail!(Classified::not_found(
                    prefix,
                    format!("no record with ID (prefix) {prefix:?}{hint}")
                ))
            }
            _ => bail!(Classified::new(
                ErrorKind::AmbiguousId,
                format!(
                    "ID prefix {prefix:?} matches {} records; use a longer prefix:\n{}",
                    hits.len(),
                    listing(&hits)
                )
            )),
        }
    }
    /// The hypothesis `id` and its criteria and predictions; with
    /// `active_only`, without archived ones.
    fn claim_records<'a>(&'a self, id: &'a str, active_only: bool) -> BTreeSet<&'a str> {
        let mut own = BTreeSet::from([id]);
        own.extend(
            self.objects
                .iter()
                .filter(|e| {
                    e.record.data.owner() == Some(id)
                        && matches!(
                            e.record.data,
                            Data::Criterion { .. } | Data::Prediction { .. }
                        )
                        && !(active_only && e.record.archived)
                })
                .map(|e| e.record.id.as_str()),
        );
        own
    }
    /// The active evidence links (supports, contradicts, qualifies) to
    /// hypothesis `id` or one of its active criteria or predictions.
    pub fn evidence_links<'a>(&'a self, id: &'a str) -> Vec<&'a Entry> {
        let targets = self.claim_records(id, true);
        self.objects
            .iter()
            .filter(|e| {
                !e.record.archived
                    && matches!(&e.record.data, Data::Link {
                        to,
                        relation: Relation::Supports | Relation::Contradicts | Relation::Qualifies,
                        ..
                    } if targets.contains(to.as_str()))
            })
            .collect()
    }
    /// The evidence `evidence_links` start at: what an assessment of `id`
    /// may cite.
    pub fn linked_evidence<'a>(&'a self, id: &'a str) -> BTreeSet<&'a str> {
        self.evidence_links(id)
            .into_iter()
            .filter_map(|e| match &e.record.data {
                Data::Link { from, .. } => Some(from.as_str()),
                _ => None,
            })
            .collect()
    }
    /// Each observation `linked_evidence` holds, once, in the order of its
    /// first link, with what each of its `evidence_links` means for
    /// hypothesis `id` (`Stance::of_link`).
    pub fn evidence_bearings(&self, id: &str) -> Vec<EvidenceBearing> {
        let mut out: Vec<EvidenceBearing> = Vec::new();
        for l in self.evidence_links(id) {
            let Data::Link { from, to, relation } = &l.record.data else {
                continue;
            };
            let Some(target) = Kind::of_id(to) else {
                continue;
            };
            let Some((stance, meaning)) = Stance::of_link(target, to, *relation) else {
                continue;
            };
            let bearing = Bearing {
                link: l.record.id.clone(),
                via: to.clone(),
                relation: *relation,
                stance,
                meaning,
            };
            match out.iter_mut().find(|e| e.evidence == *from) {
                Some(e) => {
                    e.stance = Stance::combined([e.stance, stance]);
                    e.bearings.push(bearing);
                }
                None => out.push(EvidenceBearing {
                    evidence: from.clone(),
                    stance,
                    bearings: vec![bearing],
                }),
            }
        }
        out
    }
    /// The hypotheses (archived ones included) that evidence `id` bears on,
    /// in project order, each with what the evidence means for it
    /// (`evidence_bearings`). Reads the derived `bearings` (`derive`).
    pub fn bears_on(&self, id: &str) -> Vec<(&Entry, &EvidenceBearing)> {
        self.objects
            .iter()
            .filter_map(|h| {
                let list = self.bearings.get(&h.record.id)?;
                list.iter().find(|b| b.evidence == id).map(|b| (h, b))
            })
            .collect()
    }
    /// Whether hypothesis `id` is live: not archived, and its current
    /// judgment is not falsified. Reads the derived `hypotheses`.
    pub fn is_live(&self, id: &str) -> bool {
        self.get(id).is_some_and(|e| !e.record.archived)
            && self
                .hypotheses
                .get(id)
                .is_some_and(|state| state.judgment != Judgment::Falsified)
    }
    /// The observations no live hypothesis accounts for (HYPO-0091):
    /// evidence, not archived, without a bearing of stance `For` or
    /// `Qualifies` (`Stance::of_link`: an active link to the hypothesis or
    /// to an active criterion or prediction of it) on a hypothesis that
    /// `is_live`. So an observation is unexplained again once every
    /// explanation of it is falsified or archived, and one that only counts
    /// against live hypotheses, or only falsified one, is unexplained too;
    /// so is evidence only a run or a gap names. In project order. Reads
    /// the derived `bearings` and `hypotheses` (`derive`).
    pub fn unexplained(&self) -> Vec<&Entry> {
        let accounts = |b: &Bearing| matches!(b.stance, Stance::For | Stance::Qualifies);
        let explained: BTreeSet<&str> = self
            .bearings
            .iter()
            .filter(|(h, _)| self.is_live(h))
            .flat_map(|(_, list)| list.iter())
            .filter(|e| e.bearings.iter().any(accounts))
            .map(|e| e.evidence.as_str())
            .collect();
        self.objects
            .iter()
            .filter(|e| {
                !e.record.archived
                    && matches!(e.record.data, Data::Evidence { .. })
                    && !explained.contains(e.record.id.as_str())
            })
            .collect()
    }
    /// What an assessment of hypothesis `id` is based on (decision-0003):
    /// record ID -> the fields of that record that count. Covered: the claim
    /// (title, body, scope, assumptions, archived), its criteria and
    /// predictions, links touching any of them (for a link to another
    /// hypothesis only the link, not that hypothesis), `linked_evidence`, and
    /// runs of its experiments with the evidence they cite. Evidence counts
    /// with its provenance (source, locator, attachment hashes). Any of these
    /// records that references data records (`Record::data_refs`) counts with
    /// their sha256s, in order (decision-0005): under `data`, left out while
    /// there are none, so a record without data keeps its fingerprint; for
    /// evidence after its attachment hashes under `attachments`, so turning
    /// an attachment into a data record (the schema-3 migration) keeps the
    /// fingerprint too. A reference to a missing data record counts as its
    /// ID. Not covered: lifecycle, tags, the untestable reason, experiments
    /// themselves, gaps, assessments, the data records' other metadata and
    /// timestamps. `fingerprint` hashes it; `hyp --json show` prints it.
    pub fn basis(&self, id: &str) -> BTreeMap<String, serde_json::Value> {
        use serde_json::json;
        let data_hashes = |r: &Record| -> Vec<String> {
            r.data_refs
                .iter()
                .map(|d| match self.get(d).map(|e| &e.record.data) {
                    Some(Data::Captured { sha256, .. }) => sha256.clone(),
                    _ => d.clone(),
                })
                .collect()
        };
        let own = self.claim_records(id, false);
        let experiments: BTreeSet<&str> = self
            .objects
            .iter()
            .filter(|e| matches!(&e.record.data, Data::Experiment { hypothesis, .. } if hypothesis == id))
            .map(|e| e.record.id.as_str())
            .collect();
        let mut basis = BTreeMap::new();
        let mut evidence_ids = self.linked_evidence(id);
        for e in &self.objects {
            let r = &e.record;
            let (title, body, archived) = (&r.title, &r.body, r.archived);
            let fields = match &r.data {
                Data::Hypothesis {
                    scope, assumptions, ..
                } if r.id == id => {
                    json!({"title": title, "body": body, "scope": scope, "assumptions": assumptions, "archived": archived})
                }
                Data::Criterion { .. } if own.contains(r.id.as_str()) => {
                    json!({"title": title, "body": body, "archived": archived})
                }
                Data::Prediction { conditions, .. } if own.contains(r.id.as_str()) => {
                    json!({"title": title, "body": body, "conditions": conditions, "archived": archived})
                }
                Data::Link { from, to, relation }
                    if own.contains(from.as_str()) || own.contains(to.as_str()) =>
                {
                    json!({"from": from, "to": to, "relation": relation, "body": body, "archived": archived})
                }
                Data::Run {
                    experiment,
                    outcome,
                    evidence,
                    ..
                } if experiments.contains(experiment.as_str()) => {
                    evidence_ids.extend(evidence.iter().map(String::as_str));
                    json!({"title": title, "body": body, "outcome": outcome, "evidence": evidence})
                }
                _ => continue,
            };
            let mut fields = fields;
            if !r.data_refs.is_empty() {
                fields["data"] = json!(data_hashes(r));
            }
            basis.insert(r.id.clone(), fields);
        }
        for e in &self.objects {
            let r = &e.record;
            if let Data::Evidence {
                source,
                locator,
                attachments,
                ..
            } = &r.data
            {
                if evidence_ids.contains(r.id.as_str()) {
                    let hashes: Vec<String> = attachments
                        .iter()
                        .map(|a| a.sha256.clone())
                        .chain(data_hashes(r))
                        .collect();
                    basis.insert(
                        r.id.clone(),
                        json!({"title": r.title, "body": r.body, "archived": r.archived,
                               "source": source, "locator": locator, "attachments": hashes}),
                    );
                }
            }
        }
        basis
    }
    /// The fingerprint of hypothesis `id`: `fingerprint_of(basis(id))`.
    pub fn fingerprint(&self, id: &str) -> String {
        fingerprint_of(&self.basis(id))
    }
    pub fn has_active_criterion(&self, id: &str) -> bool {
        self.objects.iter().any(|e| {
            !e.record.archived
                && matches!(&e.record.data, Data::Criterion { hypothesis } if hypothesis == id)
        })
    }
    pub fn assessment_heads(&self, id: &str) -> Vec<&Entry> {
        let assessments: Vec<_> = self
            .objects
            .iter()
            .filter(
                |e| matches!(&e.record.data, Data::Assessment{hypothesis,..} if hypothesis == id),
            )
            .collect();
        let superseded: BTreeSet<_> = assessments
            .iter()
            .flat_map(|e| match &e.record.data {
                Data::Assessment { supersedes, .. } => supersedes.clone(),
                _ => vec![],
            })
            .collect();
        assessments
            .into_iter()
            .filter(|e| !superseded.contains(&e.record.id))
            .collect()
    }
    pub fn derive(&mut self) {
        self.hypotheses.clear();
        self.bearings.clear();
        for e in &self.objects {
            if !matches!(e.record.data, Data::Hypothesis { .. }) {
                continue;
            }
            let id = &e.record.id;
            let bearings = self.evidence_bearings(id);
            self.bearings.insert(id.clone(), bearings);
            let fingerprint = self.fingerprint(id);
            let heads = self.assessment_heads(id);
            let (judgment, confidence, needs_review) = if heads.len() == 1 {
                match &heads[0].record.data {
                    Data::Assessment {
                        judgment,
                        confidence,
                        based_on,
                        ..
                    } => (*judgment, *confidence, *based_on != fingerprint),
                    _ => unreachable!(),
                }
            } else {
                (Judgment::Untested, None, !heads.is_empty())
            };
            let assessment_ids: Vec<String> = heads.iter().map(|e| e.record.id.clone()).collect();
            let review_token = review_token(&fingerprint, &heads);
            self.hypotheses.insert(
                id.clone(),
                HypothesisState {
                    judgment,
                    confidence,
                    needs_review,
                    review_token,
                    assessment_ids,
                    fingerprint,
                },
            );
        }
        self.revision =
            hash(serde_json::to_vec(&(&self.objects, &self.diagnostics)).unwrap_or_default());
    }
    /// Every rule record `r` breaks in this state: first rules of its own
    /// fields (`Code::Invalid`), then rules between it and other records
    /// (`DanglingReference`, `Cycle`, `Inconsistent`). Empty if it is valid.
    pub fn validate(&self, r: &Record) -> Vec<Violation> {
        let mut out = Violations::default();
        Self::validate_fields(r, &mut out);
        for v in &mut out.0 {
            if v.repair.is_none() {
                v.repair = Some(invalid_repair(r));
            }
        }
        self.validate_relations(r, &mut out);
        out.0
    }
    /// The rules record `r` must satisfy on its own. Referenced records are
    /// typed by their ID prefix, which every record's own ID must carry, so a
    /// reference of the wrong kind is found even while its target is missing.
    fn validate_fields(r: &Record, out: &mut Violations) {
        use Code::Invalid;
        // A reference that is not a full ID is reported once, below.
        let is = |id: &str, kinds: &[Kind]| Kind::of_id(id).is_none_or(|k| kinds.contains(&k));
        out.require(!r.title.trim().is_empty(), Invalid, || {
            "title is required".into()
        });
        if r.title.contains(['\n', '\r']) {
            // `validate` gives an immutable record the general repair.
            let repair = (!r.is_immutable()).then(|| Repair {
                note: Some(format!(
                    "Keep the first line as the title and move the rest into the body: \
                     hyp edit {}. hyp check confirms the fix.",
                    r.id
                )),
                commands: vec![],
            });
            out.push(
                Invalid,
                "title must be a single line; put the rest in the body".into(),
                repair,
            );
        }
        out.require(r.title.len() <= 2000, Invalid, || {
            "title is too long".into()
        });
        out.require(
            Kind::of_id(&r.id) == Some(r.data.kind_value()),
            Invalid,
            || "invalid object ID".into(),
        );
        for field in [&r.created_at, &r.updated_at] {
            if let Err(e) = chrono::DateTime::parse_from_rfc3339(field) {
                out.push(Invalid, e.to_string(), None);
            }
        }
        for id in r.references() {
            out.require(Kind::of_id(id).is_some(), Invalid, || {
                "stored references must use full IDs".into()
            });
        }
        if let Some(owner) = r.data.owner() {
            out.require(is(owner, &[Kind::Hypothesis]), Invalid, || {
                "owner must be a hypothesis".into()
            });
        }
        let evidence_check = |out: &mut Violations, ids: &[String]| {
            for id in ids {
                out.require(is(id, &[Kind::Evidence]), Invalid, || {
                    format!("{id} is not evidence")
                });
            }
        };
        for (i, id) in r.data_refs.iter().enumerate() {
            out.require(is(id, &[Kind::Data]), Invalid, || {
                format!("{id} is not a data record (data references name D- records)")
            });
            out.require(!r.data_refs[..i].contains(id), Invalid, || {
                format!("data names {id} twice; list each data record once")
            });
        }
        let claim = [Kind::Hypothesis, Kind::Prediction, Kind::Criterion];
        match &r.data {
            Data::Captured {
                origin,
                captured_at,
                media_type,
                sha256,
                ..
            } => {
                out.require(r.data_refs.is_empty(), Invalid, || {
                    "a data record cannot reference data records".into()
                });
                out.require(!origin.trim().is_empty(), Invalid, || {
                    "a data record needs its origin: where the bytes came from".into()
                });
                if let Err(e) = chrono::DateTime::parse_from_rfc3339(captured_at) {
                    out.push(Invalid, format!("captured_at: {e}"), None);
                }
                out.require(crate::data::is_media_type(media_type), Invalid, || {
                    format!("media_type {media_type:?} is not of the form type/subtype")
                });
                out.require(crate::data::is_sha256(sha256), Invalid, || {
                    "sha256 must be 64 lowercase hex digits".into()
                });
            }
            Data::Evidence {
                source,
                observed_at,
                attachments,
                ..
            } => {
                out.require(!source.trim().is_empty(), Invalid, || {
                    "evidence source is required".into()
                });
                out.require(
                    observed_at.is_empty() || is_observed_at(observed_at),
                    Invalid,
                    || {
                        format!(
                            "observed_at {observed_at:?} is neither an RFC 3339 timestamp \
                             (2026-09-12T14:03:00Z) nor a date (2026-09-12)"
                        )
                    },
                );
                for a in attachments {
                    out.require(
                        a.path.starts_with("assets/")
                            && !a.path.contains("..")
                            && !a.path.contains('\\'),
                        Invalid,
                        || "unsafe attachment path".into(),
                    );
                    out.require(
                        a.sha256.len() == 64 && a.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
                        Invalid,
                        || "invalid attachment hash".into(),
                    );
                }
            }
            Data::Link { from, to, relation } => {
                out.require(from != to, Invalid, || {
                    "cannot link an object to itself".into()
                });
                match relation {
                    Relation::Supports | Relation::Contradicts | Relation::Qualifies => {
                        out.require(is(from, &[Kind::Evidence]), Invalid, || {
                            "evidence links must start at evidence".into()
                        });
                        out.require(is(to, &claim), Invalid, || {
                            "evidence must target a hypothesis, prediction or criterion".into()
                        });
                        out.require(!r.body.trim().is_empty(), Invalid, || {
                            "evidence links require an explanation".into()
                        });
                    }
                    _ => {
                        out.require(
                            is(from, &[Kind::Hypothesis]) && is(to, &[Kind::Hypothesis]),
                            Invalid,
                            || "hypothesis relations require two hypotheses".into(),
                        );
                    }
                }
            }
            Data::Experiment { targets, .. } => {
                for t in targets {
                    out.require(is(&t.id, &claim), Invalid, || {
                        "invalid experiment target".into()
                    });
                    out.require(
                        t.revision.len() == 64 && !t.title.is_empty() && !t.body.is_empty(),
                        Invalid,
                        || {
                            format!(
                                "experiment target {} is missing its frozen revision, title or body",
                                t.id
                            )
                        },
                    );
                }
            }
            Data::Run {
                experiment,
                plan,
                evidence,
                ..
            } => {
                out.require(is(experiment, &[Kind::Experiment]), Invalid, || {
                    "run requires an experiment".into()
                });
                out.require(
                    plan.id == *experiment
                        && plan.revision.len() == 64
                        && !plan.title.is_empty()
                        && !plan.body.is_empty(),
                    Invalid,
                    || {
                        "invalid frozen experiment plan: it must name the experiment and keep \
                         its revision, title and body"
                            .into()
                    },
                );
                evidence_check(out, evidence);
            }
            Data::Gap {
                resolved,
                resolved_by,
                ..
            } => {
                evidence_check(out, resolved_by);
                out.require(*resolved || resolved_by.is_empty(), Invalid, || {
                    "resolved_by names what answered a resolved gap; an open gap has none".into()
                });
            }
            Data::Assessment {
                judgment,
                confidence,
                evidence,
                criterion,
                based_on,
                supersedes,
                ..
            } => {
                out.require(!r.body.trim().is_empty(), Invalid, || {
                    "assessment requires a rationale".into()
                });
                out.require(
                    confidence.is_none_or(|x| x.is_finite() && (0.0..=1.0).contains(&x)),
                    Invalid,
                    || "confidence must be between 0 and 1".into(),
                );
                evidence_check(out, evidence);
                out.require(based_on.len() == 64, Invalid, || {
                    "assessment requires a state fingerprint".into()
                });
                // Only presence: whether the evidence meets the criterion
                // depends on links, which may change later (`validate_new`).
                if *judgment == Judgment::Falsified {
                    out.require(criterion.is_some() && !evidence.is_empty(), Invalid, || {
                        format!("falsified requires {}", Judgment::Falsified.requirement())
                    });
                }
                if let Some(c) = criterion {
                    out.require(is(c, &[Kind::Criterion]), Invalid, || {
                        format!("{c} is not a criterion")
                    });
                }
                for s in supersedes {
                    out.require(s != &r.id, Invalid, || {
                        "assessment cannot supersede itself".into()
                    });
                    out.require(is(s, &[Kind::Assessment]), Invalid, || {
                        format!("superseded {s} is not an assessment")
                    });
                }
            }
            _ => {}
        }
    }
    /// The rules between record `r` and the other records of this state.
    /// A missing record is reported once, as `DanglingReference`; rules that
    /// would need it are skipped.
    fn validate_relations(&self, r: &Record, out: &mut Violations) {
        for id in r.references() {
            if Kind::of_id(id).is_some() && self.get(id).is_none() {
                out.push(
                    Code::DanglingReference,
                    format!("references {id}, which does not exist"),
                    Some(dangling_repair(r, id, |id| self.get(id).is_some())),
                );
            }
        }
        let owner_of = |id: &str| self.get(id).map(|e| e.record.data.owner());
        match &r.data {
            Data::Hypothesis {
                lifecycle: Lifecycle::Investigating,
                untestable_reason,
                ..
            } => {
                out.require(
                    !untestable_reason.trim().is_empty() || self.has_active_criterion(&r.id),
                    Code::Inconsistent,
                    || {
                        "investigating requires a falsification criterion or untestable_reason"
                            .into()
                    },
                );
            }
            // Archived links are not part of the graph, so they close no cycle.
            Data::Link { from, to, relation }
                if !r.archived
                    && matches!(relation, Relation::DependsOn | Relation::Supersedes) =>
            {
                let mut pending = vec![to.as_str()];
                let mut visited = BTreeSet::new();
                while let Some(n) = pending.pop() {
                    if n == from {
                        out.push(
                            Code::Cycle,
                            format!("{relation} cycle detected"),
                            Some(Repair {
                                note: Some("Archiving any one link of the cycle breaks it.".into()),
                                commands: vec![vec!["hyp".into(), "archive".into(), r.id.clone()]],
                            }),
                        );
                        break;
                    }
                    if !visited.insert(n) {
                        continue;
                    }
                    for e in &self.objects {
                        if e.record.id == r.id || e.record.archived {
                            continue;
                        }
                        if let Data::Link {
                            from: a,
                            to: b,
                            relation: rel,
                        } = &e.record.data
                        {
                            if a == n && rel == relation {
                                pending.push(b);
                            }
                        }
                    }
                }
            }
            Data::Experiment {
                hypothesis,
                targets,
                ..
            } => {
                for t in targets {
                    if let Some(owner) = owner_of(&t.id) {
                        out.require(
                            t.id == *hypothesis || owner == Some(hypothesis),
                            Code::Inconsistent,
                            || "experiment target belongs to a different hypothesis".into(),
                        );
                    }
                }
            }
            Data::Assessment {
                hypothesis,
                criterion,
                supersedes,
                ..
            } => {
                if let Some(owner) = criterion.as_deref().and_then(owner_of) {
                    out.require(owner == Some(hypothesis), Code::Inconsistent, || {
                        "criterion must belong to the assessed hypothesis".into()
                    });
                }
                for s in supersedes {
                    if let Some(owner) = owner_of(s) {
                        out.require(owner == Some(hypothesis), Code::Inconsistent, || {
                            "superseded assessment belongs to another hypothesis".into()
                        });
                    }
                }
                let mut todo: Vec<&str> = supersedes.iter().map(String::as_str).collect();
                let mut seen = BTreeSet::new();
                while let Some(id) = todo.pop() {
                    if id == r.id {
                        out.push(Code::Cycle, "assessment supersession cycle".into(), None);
                        break;
                    }
                    if seen.insert(id) {
                        if let Some(Data::Assessment { supersedes, .. }) =
                            self.get(id).map(|e| &e.record.data)
                        {
                            todo.extend(supersedes.iter().map(String::as_str));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    /// Rules for creating record `r` in this state, on top of `validate`.
    /// They are not checked on stored records: assessments are immutable
    /// history, and archiving a link later must not invalidate one. Every
    /// judgment except untested cites evidence, and only `linked_evidence`;
    /// falsified cites evidence that meets its criterion (HYPO-0082). Each
    /// error quotes `Judgment::rule`.
    pub fn validate_new(&self, r: &Record) -> Result<()> {
        let Data::Assessment {
            hypothesis,
            judgment,
            evidence,
            criterion,
            ..
        } = &r.data
        else {
            return Ok(());
        };
        let article = article(judgment.as_str());
        let rule = Judgment::rule();
        ensure!(
            *judgment == Judgment::Untested || !evidence.is_empty(),
            "{article} {judgment} assessment must cite evidence linked to {hypothesis} \
             (hyp assess --evidence E-…). The rule: {rule}"
        );
        ensure!(
            *judgment != Judgment::Falsified || criterion.is_some(),
            "a falsified assessment must name the falsification criterion of {hypothesis} \
             that its evidence meets (hyp assess --criterion F-…). The rule: {rule}"
        );
        let linked = self.linked_evidence(hypothesis);
        for id in evidence {
            // Unknown IDs and prefixes are reported by `validate`.
            if self.get(id).is_some() {
                ensure!(
                    linked.contains(id.as_str()),
                    "{id} is not linked to {hypothesis} or its active criteria or predictions, \
                     and an assessment may cite only linked evidence. Link it first \
                     (hyp link {id} {hypothesis} --relation supports|contradicts|qualifies \
                     --reason \"...\"), then re-read and assess"
                );
            }
        }
        // A missing criterion or cited evidence, or a criterion of another
        // hypothesis, is reported by `validate`, which names the actual
        // problem; whether the evidence meets the criterion is not a question yet.
        let all_cited_exist = evidence.iter().all(|id| self.get(id).is_some());
        if let (Judgment::Falsified, Some(criterion), true) = (judgment, criterion, all_cited_exist)
        {
            if let Some(c) = self
                .get(criterion)
                .filter(|c| c.record.data.owner() == Some(hypothesis))
            {
                ensure!(
                    !c.record.archived,
                    "criterion {criterion} is archived, and a falsified assessment rests on an \
                     active criterion (hyp restore {criterion}). The rule: {rule}"
                );
                let meets = |b: &Bearing| b.via == *criterion && b.stance == Stance::Against;
                ensure!(
                    self.evidence_bearings(hypothesis).iter().any(|e| {
                        evidence.contains(&e.evidence) && e.bearings.iter().any(meets)
                    }),
                    "a falsified assessment must cite evidence that meets its criterion \
                     {criterion}: none of the cited evidence has an active supports link to it \
                     (evidence linked to the hypothesis, or against the criterion, does not \
                     meet it). Link the observation that meets it (hyp link E-… {criterion} \
                     --relation supports --reason \"...\"), then re-read and assess. \
                     The rule: {rule}"
                );
            }
        }
        Ok(())
    }
    /// The errors that block writes (`Code::blocks_writes`).
    pub fn blocking(&self) -> Vec<&Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == "error" && d.blocks_writes)
            .collect()
    }
    /// Fails while a diagnostic blocks writes (`Code::blocks_writes`), with
    /// kind `Blocked` and the blocking diagnostics. Other errors do not
    /// block here; `Store::commit_written` rejects only writes that add one,
    /// and lets a write through that repairs invalid records only.
    pub fn assert_writable(&self) -> Result<()> {
        let blocking = self.blocking();
        let Some(first) = blocking.first() else {
            return Ok(());
        };
        let errors = self.diagnostics.iter().filter(|d| d.severity == "error");
        let others = errors.count() - blocking.len();
        let n = match blocking.len() {
            1 => "1 error blocks".to_string(),
            n => format!("{n} errors block"),
        };
        let more = match others {
            0 => String::new(),
            1 => " (1 more does not)".into(),
            m => format!(" ({m} more do not)"),
        };
        // A record hyp can change, by the ID its file is named by.
        let changeable = |d: &Diagnostic| {
            let id = d.path.rsplit('/').next().unwrap_or_default();
            let id = id.strip_suffix(".md").unwrap_or(id);
            self.get(id).is_some_and(|e| !e.record.is_immutable())
        };
        let only_invalid = blocking.iter().all(|d| d.code == Code::Invalid);
        let repairable = match only_invalid && blocking.iter().any(|d| changeable(d)) {
            true => {
                ". Invalid records can be repaired through hyp: a write that changes only \
                 invalid records and leaves each of them valid is accepted"
            }
            false => "",
        };
        bail!(Classified::about(
            ErrorKind::Blocked,
            format!(
                "{n} writes{more}; run hyp check and repair files before writing: {}: {}{repairable}",
                first.path, first.message
            ),
            blocking.into_iter().cloned().collect(),
        ));
    }
}
