use anyhow::{Result, bail, ensure};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

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
values!(Relation { Supports => "supports", Contradicts => "contradicts", Qualifies => "qualifies", DependsOn => "depends_on", CompetesWith => "competes_with", Supersedes => "supersedes" });
values!(ExperimentStatus { Planned => "planned", Running => "running", Completed => "completed", Cancelled => "cancelled" });
values!(Outcome { Observed => "observed", Inconclusive => "inconclusive", Failed => "failed" });
values!(Kind { Hypothesis => "hypothesis", Prediction => "prediction", Criterion => "criterion", Evidence => "evidence", Link => "link", Experiment => "experiment", Run => "run", Assessment => "assessment", Gap => "gap" });

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
        #[serde(default)]
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
        status: ExperimentStatus,
    },
    Run {
        experiment: String,
        #[serde(default)]
        plan: FrozenRef,
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
        }
    }
    pub fn prefix(&self) -> &'static str {
        match self {
            Self::Hypothesis { .. } => "H",
            Self::Prediction { .. } => "P",
            Self::Criterion { .. } => "F",
            Self::Evidence { .. } => "E",
            Self::Link { .. } => "L",
            Self::Experiment { .. } => "X",
            Self::Run { .. } => "R",
            Self::Assessment { .. } => "A",
            Self::Gap { .. } => "G",
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
            _ => {}
        }
        refs
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
            archived: false,
            created_at: now.clone(),
            updated_at: now,
            data,
        }
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub path: String,
    pub message: String,
    pub severity: String,
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
    pub revision: String,
}
pub fn hash(bytes: impl AsRef<[u8]>) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
impl Snapshot {
    /// The entry with exactly this ID; no prefix matching.
    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.objects.iter().find(|e| e.record.id == id)
    }
    pub fn find(&self, prefix: &str) -> Result<&Entry> {
        if let Some(e) = self.objects.iter().find(|e| e.record.id == prefix) {
            return Ok(e);
        }
        let hits: Vec<_> = self
            .objects
            .iter()
            .filter(|e| {
                e.record
                    .id
                    .to_lowercase()
                    .starts_with(&prefix.to_lowercase())
            })
            .collect();
        ensure!(
            hits.len() == 1,
            "ID {prefix:?} matches {} objects; use a longer prefix",
            hits.len()
        );
        Ok(hits[0])
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
    /// What an assessment of hypothesis `id` is based on (decision-0003):
    /// record ID -> the fields of that record that count. Covered: the claim
    /// (title, body, scope, assumptions, archived), its criteria and
    /// predictions, links touching any of them (for a link to another
    /// hypothesis only the link, not that hypothesis), `linked_evidence`, and
    /// runs of its experiments with the evidence they cite. Evidence counts
    /// with its provenance (source, locator, attachment hashes). Not covered:
    /// lifecycle, tags, the untestable reason, experiments themselves, gaps,
    /// assessments and timestamps. `fingerprint` hashes it; `hyp --json show`
    /// prints it.
    pub fn basis(&self, id: &str) -> BTreeMap<String, serde_json::Value> {
        use serde_json::json;
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
                    let hashes: Vec<&str> = attachments.iter().map(|a| a.sha256.as_str()).collect();
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
        for e in &self.objects {
            if !matches!(e.record.data, Data::Hypothesis { .. }) {
                continue;
            }
            let id = &e.record.id;
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
    pub fn validate(&self, r: &Record) -> Result<()> {
        ensure!(!r.title.trim().is_empty(), "title is required");
        ensure!(r.title.len() <= 2000, "title is too long");
        ensure!(
            r.id.starts_with(&format!("{}-", r.data.prefix()))
                && uuid::Uuid::parse_str(&r.id[2..]).is_ok(),
            "invalid object ID"
        );
        for field in [&r.created_at, &r.updated_at] {
            chrono::DateTime::parse_from_rfc3339(field)?;
        }
        for id in r.data.references() {
            ensure!(
                self.find(id)?.record.id == id,
                "stored references must use full IDs"
            );
        }
        if let Some(owner) = r.data.owner() {
            ensure!(
                matches!(self.find(owner)?.record.data, Data::Hypothesis { .. }),
                "owner must be a hypothesis"
            );
        }
        let evidence_check = |ids: &[String]| -> Result<()> {
            for id in ids {
                ensure!(
                    matches!(self.find(id)?.record.data, Data::Evidence { .. }),
                    "{id} is not evidence"
                );
            }
            Ok(())
        };
        match &r.data {
            Data::Hypothesis {
                lifecycle: Lifecycle::Investigating,
                untestable_reason,
                ..
            } => {
                ensure!(
                    !untestable_reason.trim().is_empty() || self.has_active_criterion(&r.id),
                    "investigating requires a falsification criterion or untestable_reason"
                );
            }
            Data::Evidence {
                source,
                attachments,
                ..
            } => {
                ensure!(!source.trim().is_empty(), "evidence source is required");
                for a in attachments {
                    ensure!(
                        a.path.starts_with("assets/")
                            && !a.path.contains("..")
                            && !a.path.contains('\\'),
                        "unsafe attachment path"
                    );
                    ensure!(
                        a.sha256.len() == 64 && a.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
                        "invalid attachment hash"
                    );
                }
            }
            Data::Link { from, to, relation } => {
                ensure!(from != to, "cannot link an object to itself");
                let a = &self.find(from)?.record;
                let b = &self.find(to)?.record;
                match relation {
                    Relation::Supports | Relation::Contradicts | Relation::Qualifies => {
                        ensure!(
                            matches!(a.data, Data::Evidence { .. }),
                            "evidence links must start at evidence"
                        );
                        ensure!(
                            matches!(
                                b.data,
                                Data::Hypothesis { .. }
                                    | Data::Prediction { .. }
                                    | Data::Criterion { .. }
                            ),
                            "evidence must target a hypothesis, prediction or criterion"
                        );
                        ensure!(
                            !r.body.trim().is_empty(),
                            "evidence links require an explanation"
                        );
                    }
                    _ => {
                        ensure!(
                            matches!(a.data, Data::Hypothesis { .. })
                                && matches!(b.data, Data::Hypothesis { .. }),
                            "hypothesis relations require two hypotheses"
                        );
                    }
                }
                if matches!(relation, Relation::DependsOn | Relation::Supersedes) {
                    let mut pending = vec![to.clone()];
                    let mut visited = BTreeSet::new();
                    while let Some(n) = pending.pop() {
                        ensure!(&n != from, "{relation} cycle detected");
                        if !visited.insert(n.clone()) {
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
                                if a == &n && rel == relation {
                                    pending.push(b.clone());
                                }
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
                    let target = &self.find(&t.id)?.record;
                    ensure!(
                        target.id == *hypothesis || target.data.owner() == Some(hypothesis),
                        "experiment target belongs to a different hypothesis"
                    );
                    ensure!(
                        matches!(
                            target.data,
                            Data::Hypothesis { .. }
                                | Data::Prediction { .. }
                                | Data::Criterion { .. }
                        ),
                        "invalid experiment target"
                    );
                    ensure!(
                        t.revision.len() == 64 && !t.title.is_empty() && !t.body.is_empty(),
                        "experiment target {} is missing its frozen revision, title or body",
                        t.id
                    );
                }
            }
            Data::Run {
                experiment,
                plan,
                evidence,
                ..
            } => {
                ensure!(
                    matches!(self.find(experiment)?.record.data, Data::Experiment { .. }),
                    "run requires an experiment"
                );
                ensure!(
                    plan.id == *experiment
                        && plan.revision.len() == 64
                        && !plan.title.is_empty()
                        && !plan.body.is_empty(),
                    "invalid frozen experiment plan: it must name the experiment and keep \
                     its revision, title and body"
                );
                evidence_check(evidence)?;
            }
            Data::Assessment {
                hypothesis,
                judgment,
                confidence,
                evidence,
                criterion,
                based_on,
                supersedes,
            } => {
                ensure!(!r.body.trim().is_empty(), "assessment requires a rationale");
                ensure!(
                    confidence.is_none_or(|x| x.is_finite() && (0.0..=1.0).contains(&x)),
                    "confidence must be between 0 and 1"
                );
                evidence_check(evidence)?;
                ensure!(
                    based_on.len() == 64,
                    "assessment requires a state fingerprint"
                );
                if *judgment == Judgment::Falsified {
                    ensure!(
                        criterion.is_some() && !evidence.is_empty(),
                        "falsified requires a criterion and evidence"
                    );
                }
                if let Some(c) = criterion {
                    ensure!(
                        matches!(&self.find(c)?.record.data,Data::Criterion{hypothesis:h} if h==hypothesis),
                        "criterion must belong to the assessed hypothesis"
                    );
                }
                for s in supersedes {
                    ensure!(s != &r.id, "assessment cannot supersede itself");
                    ensure!(
                        matches!(&self.find(s)?.record.data,Data::Assessment{hypothesis:h,..} if h==hypothesis),
                        "superseded assessment belongs to another hypothesis"
                    );
                    let mut todo = vec![s.as_str()];
                    let mut seen = BTreeSet::new();
                    while let Some(id) = todo.pop() {
                        ensure!(id != r.id, "assessment supersession cycle");
                        if seen.insert(id) {
                            if let Data::Assessment { supersedes, .. } = &self.find(id)?.record.data
                            {
                                todo.extend(supersedes.iter().map(String::as_str));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
    /// Rules for creating record `r` in this state, on top of `validate`.
    /// They are not checked on stored records: assessments are immutable
    /// history, and archiving a link later must not invalidate one. Every
    /// judgment except untested cites evidence, and only `linked_evidence`.
    pub fn validate_new(&self, r: &Record) -> Result<()> {
        let Data::Assessment {
            hypothesis,
            judgment,
            evidence,
            ..
        } = &r.data
        else {
            return Ok(());
        };
        let article = if judgment.to_string().starts_with(['a', 'e', 'i', 'o', 'u']) {
            "an"
        } else {
            "a"
        };
        ensure!(
            *judgment == Judgment::Untested || !evidence.is_empty(),
            "{article} {judgment} assessment must cite evidence: every judgment except \
             untested needs at least one evidence ID linked to {hypothesis}"
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
        Ok(())
    }
    pub fn assert_healthy(&self) -> Result<()> {
        let errors: Vec<_> = self
            .diagnostics
            .iter()
            .filter(|d| d.severity == "error")
            .collect();
        if !errors.is_empty() {
            bail!(
                "project has {} validation errors; run hyp check and repair files before writing: {}",
                errors.len(),
                errors[0].message
            );
        }
        Ok(())
    }
}
