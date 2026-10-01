//! The transaction engine (`Store::commit`) over arbitrary sequences of
//! batches, valid and not: each batch is applied whole or not at all, and
//! what it leaves is a notebook `hyp check` accepts.
//!
//! Batches are generated as operations on records chosen by position in
//! the snapshot read just before, so most name records that exist and of
//! the right kind; some name the wrong kind, a missing record, a batch
//! reference (`@name`), stale revisions or a stale project revision.
use crate::{
    files,
    generate::{id_of, judgment, relation, text},
};
use hyp::{
    model::*,
    store::{Change, Store, Verify, Written},
};
use proptest::{collection::vec, option, prelude::*, sample::Index};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

/// A record named by a change.
#[derive(Debug, Clone)]
enum Target {
    /// A record of the kind the field takes, if there is one.
    Fitting(Index),
    /// Any record.
    Any(Index),
    /// A batch-local reference, `@r<n>`.
    Batch(u8),
    /// A full ID no record has.
    Missing,
}

fn target() -> impl Strategy<Value = Target> {
    prop_oneof![
        12 => any::<Index>().prop_map(Target::Fitting),
        2 => any::<Index>().prop_map(Target::Any),
        1 => (0u8..3).prop_map(Target::Batch),
        1 => Just(Target::Missing),
    ]
}

impl Target {
    /// The ID this target names in `seen`, for a field that takes `kinds`.
    fn id(&self, seen: &Snapshot, kinds: &[Kind]) -> String {
        let pick = |candidates: Vec<&Entry>, i: &Index| {
            (!candidates.is_empty()).then(|| i.get(&candidates).record.id.clone())
        };
        let fitting = |i: &Index| {
            pick(
                seen.objects
                    .iter()
                    .filter(|e| kinds.contains(&e.record.data.kind_value()))
                    .collect(),
                i,
            )
        };
        let any = |i: &Index| pick(seen.objects.iter().collect(), i);
        let missing = || id_of(kinds[0], 1000);
        match self {
            Self::Fitting(i) => fitting(i).or_else(|| any(i)).unwrap_or_else(missing),
            Self::Any(i) => any(i).unwrap_or_else(missing),
            Self::Batch(n) => format!("@r{n}"),
            Self::Missing => missing(),
        }
    }
}

/// Text that is usually ordinary, sometimes hostile.
fn words() -> BoxedStrategy<String> {
    prop_oneof![
        8 => "[a-zA-Z0-9 ]{1,16}",
        1 => text(),
    ]
    .boxed()
}

/// A field a patch sets, with a value right or wrong for it.
#[derive(Debug, Clone)]
enum Field {
    Title(String),
    Body(String),
    Tags(Vec<String>),
    Archived(bool),
    Lifecycle(Lifecycle),
    Scope(String),
    Conditions(String),
    Relation(Relation),
    Owner(Target),
    Raw(&'static str, &'static str),
}

/// Fields a patch may not set, or values of the wrong type, as JSON.
const RAW: &[(&str, &str)] = &[
    ("id", "\"H-00000000-0000-0000-0000-000000000009\""),
    ("kind", "\"evidence\""),
    ("created_at", "\"2020-01-01T00:00:00Z\""),
    ("updated_at", "\"2020-01-01T00:00:00Z\""),
    ("bogus", "1"),
    ("title", "null"),
    ("tags", "\"not a list\""),
    ("lifecycle", "\"retired\""),
    ("data", "[\"D-00000000-0000-0000-0000-000000000009\"]"),
];

fn field() -> impl Strategy<Value = Field> {
    prop_oneof![
        words().prop_map(Field::Title),
        words().prop_map(Field::Body),
        vec(words(), 0..3).prop_map(Field::Tags),
        any::<bool>().prop_map(Field::Archived),
        prop::sample::select(
            &[
                Lifecycle::Draft,
                Lifecycle::Investigating,
                Lifecycle::Closed
            ][..]
        )
        .prop_map(Field::Lifecycle),
        words().prop_map(Field::Scope),
        words().prop_map(Field::Conditions),
        relation().prop_map(Field::Relation),
        target().prop_map(Field::Owner),
        prop::sample::select(RAW).prop_map(|(k, v)| Field::Raw(k, v)),
    ]
}

impl Field {
    fn set(&self, seen: &Snapshot) -> serde_json::Map<String, Value> {
        let (name, value) = match self {
            Self::Title(s) => ("title", json!(s)),
            Self::Body(s) => ("body", json!(s)),
            Self::Tags(t) => ("tags", json!(t)),
            Self::Archived(a) => ("archived", json!(a)),
            Self::Lifecycle(l) => ("lifecycle", json!(l)),
            Self::Scope(s) => ("scope", json!(s)),
            Self::Conditions(s) => ("conditions", json!(s)),
            Self::Relation(r) => ("relation", json!(r)),
            Self::Owner(t) => ("hypothesis", json!(t.id(seen, &[Kind::Hypothesis]))),
            Self::Raw(name, value) => (*name, serde_json::from_str(value).unwrap()),
        };
        serde_json::Map::from_iter([(name.to_string(), value)])
    }
}

/// One change of a batch, before its targets are resolved.
#[derive(Debug, Clone)]
enum Op {
    Hypothesis {
        name: Option<u8>,
        title: String,
        untestable: bool,
    },
    Criterion {
        name: Option<u8>,
        hypothesis: Target,
    },
    Prediction {
        hypothesis: Target,
        conditions: String,
    },
    Evidence {
        name: Option<u8>,
        source: String,
    },
    Link {
        from: Target,
        to: Target,
        relation: Relation,
    },
    Assessment {
        hypothesis: Target,
        judgment: Judgment,
        evidence: Vec<Target>,
        criterion: Option<Target>,
        stale: bool,
    },
    Experiment {
        name: Option<u8>,
        hypothesis: Target,
    },
    Run {
        experiment: Target,
        evidence: Vec<Target>,
    },
    Gap {
        hypothesis: Target,
        resolved_by: Vec<Target>,
    },
    Update {
        target: Target,
        title: String,
        tags: Vec<String>,
        stale: bool,
    },
    Patch {
        target: Target,
        field: Field,
        stale: bool,
    },
    Archive {
        target: Target,
        archived: bool,
        stale: bool,
    },
    Delete {
        target: Target,
        stale: bool,
    },
}

fn op() -> impl Strategy<Value = Op> {
    let name = || option::of(0u8..3);
    let stale = || prop::bool::weighted(0.15);
    prop_oneof![
        3 => (name(), words(), prop::bool::weighted(0.3)).prop_map(|(name, title, untestable)| {
            Op::Hypothesis { name, title, untestable }
        }),
        2 => (name(), target()).prop_map(|(name, hypothesis)| Op::Criterion { name, hypothesis }),
        1 => (target(), words())
            .prop_map(|(hypothesis, conditions)| Op::Prediction { hypothesis, conditions }),
        3 => (name(), words()).prop_map(|(name, source)| Op::Evidence { name, source }),
        3 => (target(), target(), relation())
            .prop_map(|(from, to, relation)| Op::Link { from, to, relation }),
        2 => (target(), judgment(), vec(target(), 0..3), option::of(target()), stale()).prop_map(
            |(hypothesis, judgment, evidence, criterion, stale)| Op::Assessment {
                hypothesis,
                judgment,
                evidence,
                criterion,
                stale,
            },
        ),
        1 => (name(), target()).prop_map(|(name, hypothesis)| Op::Experiment { name, hypothesis }),
        1 => (target(), vec(target(), 0..2))
            .prop_map(|(experiment, evidence)| Op::Run { experiment, evidence }),
        1 => (target(), vec(target(), 0..2))
            .prop_map(|(hypothesis, resolved_by)| Op::Gap { hypothesis, resolved_by }),
        2 => (target(), words(), vec(words(), 0..3), stale())
            .prop_map(|(target, title, tags, stale)| Op::Update { target, title, tags, stale }),
        3 => (target(), field(), stale())
            .prop_map(|(target, field, stale)| Op::Patch { target, field, stale }),
        2 => (target(), prop::bool::weighted(0.8), stale())
            .prop_map(|(target, archived, stale)| Op::Archive { target, archived, stale }),
        3 => (target(), stale()).prop_map(|(target, stale)| Op::Delete { target, stale }),
    ]
}

const HYPOTHESIS: &[Kind] = &[Kind::Hypothesis];
const EVIDENCE: &[Kind] = &[Kind::Evidence];
const CLAIMS: &[Kind] = &[Kind::Hypothesis, Kind::Criterion, Kind::Prediction];
const MUTABLE: &[Kind] = &[
    Kind::Hypothesis,
    Kind::Criterion,
    Kind::Prediction,
    Kind::Evidence,
    Kind::Link,
    Kind::Experiment,
    Kind::Gap,
];
/// The records of `seen` that may change and that no record refers to.
fn leaves(seen: &Snapshot) -> Vec<&Entry> {
    let referred = |id: &str| {
        seen.objects
            .iter()
            .any(|o| o.record.references().contains(&id))
    };
    seen.objects
        .iter()
        .filter(|e| !e.record.is_immutable() && !referred(&e.record.id))
        .collect()
}
/// A revision no record has.
const STALE: &str = "0000000000000000000000000000000000000000000000000000000000000000";

impl Op {
    /// The change this operation makes, stating what `seen` holds as the
    /// caller's read (or a stale revision where `stale`).
    fn change(&self, seen: &Snapshot) -> Change {
        let ids = |targets: &[Target], kinds: &[Kind]| -> Vec<String> {
            targets.iter().map(|t| t.id(seen, kinds)).collect()
        };
        let named = |mut r: Record, name: &Option<u8>| {
            r.id = name.map_or_else(String::new, |n| format!("@r{n}"));
            r
        };
        let revision = |id: &str, stale: bool| match seen.find(id) {
            Ok(e) if !stale => e.revision.clone(),
            _ => STALE.to_string(),
        };
        let create = |r: Record| Change::create_seen(r, seen);
        match self {
            Self::Hypothesis {
                name,
                title,
                untestable,
            } => create(named(
                Record::new(
                    title.clone(),
                    Data::Hypothesis {
                        scope: String::new(),
                        assumptions: String::new(),
                        lifecycle: Lifecycle::Draft,
                        untestable_reason: if *untestable {
                            "by design".into()
                        } else {
                            String::new()
                        },
                    },
                ),
                name,
            )),
            Self::Criterion { name, hypothesis } => create(named(
                Record::new(
                    "Rejected if it fails",
                    Data::Criterion {
                        hypothesis: hypothesis.id(seen, HYPOTHESIS),
                    },
                ),
                name,
            )),
            Self::Prediction {
                hypothesis,
                conditions,
            } => create(Record::new(
                "Then this happens",
                Data::Prediction {
                    hypothesis: hypothesis.id(seen, HYPOTHESIS),
                    conditions: conditions.clone(),
                },
            )),
            Self::Evidence { name, source } => create(named(
                Record::new(
                    "Observed",
                    Data::Evidence {
                        source: source.clone(),
                        locator: String::new(),
                        observed_at: String::new(),
                        attachments: vec![],
                    },
                ),
                name,
            )),
            Self::Link { from, to, relation } => {
                let mut l = Record::new(
                    "Interpretation",
                    Data::Link {
                        from: from.id(seen, EVIDENCE),
                        to: to.id(seen, CLAIMS),
                        relation: *relation,
                    },
                );
                l.body = "Why the evidence bears on the claim.".into();
                create(l)
            }
            Self::Assessment {
                hypothesis,
                judgment,
                evidence,
                criterion,
                stale,
            } => {
                let mut r = Record::new(
                    "Assessed",
                    Data::Assessment {
                        hypothesis: hypothesis.id(seen, HYPOTHESIS),
                        judgment: *judgment,
                        confidence: Some(0.5),
                        evidence: ids(evidence, EVIDENCE),
                        criterion: criterion.as_ref().map(|t| t.id(seen, &[Kind::Criterion])),
                        based_on: String::new(),
                        supersedes: vec![],
                    },
                );
                r.body = "Because of the evidence.".into();
                let mut change = create(r);
                if *stale {
                    if let Change::Create {
                        expected: Some(expected),
                        ..
                    } = &mut change
                    {
                        for seen in expected.hypotheses.values_mut() {
                            seen.review_token = STALE.into();
                        }
                    }
                }
                change
            }
            Self::Experiment { name, hypothesis } => create(named(
                Record::new(
                    "Try it",
                    Data::Experiment {
                        hypothesis: hypothesis.id(seen, HYPOTHESIS),
                        targets: vec![],
                        status: ExperimentStatus::Planned,
                    },
                ),
                name,
            )),
            Self::Run {
                experiment,
                evidence,
            } => create(Record::new(
                "Ran it",
                Data::Run {
                    experiment: experiment.id(seen, &[Kind::Experiment]),
                    plan: FrozenRef::default(),
                    outcome: Outcome::Observed,
                    evidence: ids(evidence, EVIDENCE),
                },
            )),
            Self::Gap {
                hypothesis,
                resolved_by,
            } => create(Record::new(
                "Unknown yet",
                Data::Gap {
                    hypothesis: hypothesis.id(seen, HYPOTHESIS),
                    resolved: !resolved_by.is_empty(),
                    resolved_by: ids(resolved_by, EVIDENCE),
                },
            )),
            Self::Update {
                target,
                title,
                tags,
                stale,
            } => {
                let id = target.id(seen, MUTABLE);
                match seen.find(&id) {
                    Ok(e) => {
                        let mut record = e.record.clone();
                        record.title.clone_from(title);
                        record.tags.clone_from(tags);
                        Change::Update {
                            record,
                            expected_revision: revision(&id, *stale),
                        }
                    }
                    // A batch reference or missing record: a patch names it.
                    Err(_) => Change::Patch {
                        id,
                        expected_revision: STALE.into(),
                        set: Field::Title(title.clone()).set(seen),
                    },
                }
            }
            Self::Patch {
                target,
                field,
                stale,
            } => {
                let id = target.id(seen, MUTABLE);
                Change::Patch {
                    expected_revision: revision(&id, *stale),
                    set: field.set(seen),
                    id,
                }
            }
            Self::Archive {
                target,
                archived,
                stale,
            } => {
                // Prefer records a later batch can delete.
                let leaves = leaves(seen);
                let id = match target {
                    Target::Fitting(i) if !leaves.is_empty() => i.get(&leaves).record.id.clone(),
                    _ => target.id(seen, MUTABLE),
                };
                Change::Archive {
                    expected_revision: revision(&id, *stale),
                    archived: *archived,
                    id,
                }
            }
            Self::Delete { target, stale } => {
                // Only an archived record nothing refers to can be deleted:
                // prefer those.
                let archived: Vec<&Entry> = leaves(seen)
                    .into_iter()
                    .filter(|e| e.record.archived)
                    .collect();
                let id = match target {
                    Target::Fitting(i) if !archived.is_empty() => {
                        i.get(&archived).record.id.clone()
                    }
                    _ => target.id(seen, MUTABLE),
                };
                Change::Delete {
                    expected_revision: revision(&id, *stale),
                    id,
                }
            }
        }
    }
}

/// Which project revision a batch states.
#[derive(Debug, Clone, Copy)]
enum Project {
    Unstated,
    Fresh,
    Stale,
}

fn project() -> impl Strategy<Value = Project> {
    prop_oneof![
        6 => Just(Project::Unstated),
        2 => Just(Project::Fresh),
        1 => Just(Project::Stale),
    ]
}

/// The paths whose bytes differ between `a` and `b`, or exist in one only.
fn differences(a: &BTreeMap<PathBuf, Vec<u8>>, b: &BTreeMap<PathBuf, Vec<u8>>) -> Vec<PathBuf> {
    a.keys()
        .chain(b.keys())
        .filter(|p| a.get(*p) != b.get(*p))
        .cloned()
        .collect()
}

/// A notebook with a hypothesis, its criterion, evidence that supports
/// the criterion and an experiment: something for generated changes to
/// name from the first batch on.
fn seeded(root: &Path) -> Store {
    let store = Store::init(root).unwrap();
    let with_id = |id: &str, mut r: Record| {
        r.id = id.into();
        Change::create_seen(r, &Snapshot::default())
    };
    let h = "@h".to_string();
    store
        .commit(
            vec![
                with_id(
                    "@h",
                    Record::new(
                        "Seed hypothesis",
                        Data::Hypothesis {
                            scope: String::new(),
                            assumptions: String::new(),
                            lifecycle: Lifecycle::Investigating,
                            untestable_reason: String::new(),
                        },
                    ),
                ),
                with_id(
                    "@f",
                    Record::new(
                        "Seed criterion",
                        Data::Criterion {
                            hypothesis: h.clone(),
                        },
                    ),
                ),
                with_id(
                    "@e",
                    Record::new(
                        "Seed evidence",
                        Data::Evidence {
                            source: "seed.log".into(),
                            locator: String::new(),
                            observed_at: String::new(),
                            attachments: vec![],
                        },
                    ),
                ),
                with_id("@l", {
                    let mut l = Record::new(
                        "Seed link",
                        Data::Link {
                            from: "@e".into(),
                            to: "@f".into(),
                            relation: Relation::Supports,
                        },
                    );
                    l.body = "The log shows the failure the criterion names.".into();
                    l
                }),
                with_id(
                    "@x",
                    Record::new(
                        "Seed experiment",
                        Data::Experiment {
                            hypothesis: h,
                            targets: vec![],
                            status: ExperimentStatus::Planned,
                        },
                    ),
                ),
            ],
            None,
        )
        .expect("the seed notebook is valid");
    store
}

/// Whether `snap`, the project after a successful commit, shows what
/// `change` asked for, on the record `written` names.
fn applied(snap: &Snapshot, change: &Change, written: &Written) -> Result<(), TestCaseError> {
    let entry = snap.get(&written.id);
    if let Change::Delete { .. } = change {
        prop_assert!(entry.is_none(), "deleted {} is still there", written.id);
        return Ok(());
    }
    let Some(entry) = entry else {
        return Err(TestCaseError::fail(format!(
            "{} is missing after {change:?}",
            written.id
        )));
    };
    prop_assert_eq!(Some(&entry.revision), written.revision.as_ref());
    let r = &entry.record;
    match change {
        Change::Create { record, .. } => {
            prop_assert_eq!(&r.title, &record.title);
            prop_assert_eq!(r.data.kind(), record.data.kind());
        }
        Change::Update { record, .. } => {
            prop_assert_eq!(&r.title, &record.title);
            prop_assert_eq!(&r.tags, &record.tags);
        }
        Change::Patch { set, .. } => {
            let fields = serde_json::to_value(r).unwrap();
            // A batch reference (`@name`) is stored as the ID it names.
            for (name, value) in set
                .iter()
                .filter(|(_, v)| !v.as_str().is_some_and(|s| s.starts_with('@')))
            {
                prop_assert_eq!(&fields[name], value, "patched {} of {}", name, written.id);
            }
        }
        Change::Archive { archived, .. } => prop_assert_eq!(r.archived, *archived),
        Change::Delete { .. } => unreachable!(),
    }
    Ok(())
}

proptest! {
    #![proptest_config(crate::cases(16))]

    /// A batch that fails leaves every file of `hyp/` as it was and no
    /// journal; one that succeeds returns the project a fresh read sees,
    /// and leaves no error for `hyp check` (the real binary checks the
    /// final notebook).
    #[test]
    fn commits_are_all_or_nothing(batches in vec((vec(op(), 1..4), project()), 1..10)) {
        let dir = tempfile::TempDir::new().unwrap();
        let store = seeded(dir.path());
        let notebook = dir.path().join("hyp");
        let journal = dir.path().join(".hyp/transaction.json");
        for (ops, project) in &batches {
            let seen = store.snapshot().unwrap();
            let changes: Vec<Change> = ops.iter().map(|op| op.change(&seen)).collect();
            let expected = match project {
                Project::Unstated => None,
                Project::Fresh => Some(seen.revision.as_str()),
                Project::Stale => Some(STALE),
            };
            let before = files(&notebook);
            let result = store.commit_written(changes.clone(), expected);
            let after = files(&notebook);
            prop_assert!(!journal.exists(), "a commit left its journal");
            match result {
                Err(e) => prop_assert!(
                    after == before,
                    "a failed commit changed {:?}: {e:#}\nchanges: {changes:#?}",
                    differences(&before, &after)
                ),
                Ok(committed) => {
                    let snap = &committed.snapshot;
                    prop_assert_eq!(committed.written.len(), changes.len());
                    for (change, written) in changes.iter().zip(&committed.written) {
                        applied(snap, change, written)?;
                    }
                    if committed.written.iter().any(|w| w.changed) {
                        prop_assert!(after != before, "a commit reported changes but wrote nothing");
                    }
                    let check = store.read(Verify::Content).unwrap();
                    prop_assert_eq!(&check.revision, &snap.revision, "the commit returned another project than a read sees");
                    let errors: Vec<&Diagnostic> =
                        check.diagnostics.iter().filter(|d| d.severity == "error").collect();
                    prop_assert!(errors.is_empty(), "a commit left errors: {errors:#?}\nchanges: {changes:#?}");
                }
            }
        }
        let out = Command::new(env!("CARGO_BIN_EXE_hyp"))
            .arg("--project")
            .arg(dir.path())
            .arg("check")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        prop_assert!(
            out.status.success(),
            "hyp check failed: {}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
