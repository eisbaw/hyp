//! The review basis (decision-0003, `Snapshot::basis`): an assessment
//! records the fingerprint of what it was based on, and is "needs review"
//! once that changes. Cosmetic edits (lifecycle, tags, timestamps, the
//! untestable reason, experiments, gaps, assessments, data record metadata)
//! must not flag it; any change to a record in the basis must.
use crate::generate::{BLOBS, distinct, id_of, notebook, snapshot_of, text};
use hyp::{
    model::*,
    store::{Change, Store},
};
use proptest::{collection::vec, prelude::*, sample::Index};

/// An edit `Snapshot::basis` documents as not covered.
#[derive(Debug, Clone)]
enum Cosmetic {
    Tags(Vec<String>),
    Timestamps(String),
    Lifecycle(Lifecycle),
    UntestableReason(String),
    /// An experiment's title, body and status.
    Experiment(String, ExperimentStatus),
    /// A gap's title, body and whether it is resolved.
    Gap(String, bool),
    /// An assessment's title, body and confidence.
    Assessment(String, Option<f64>),
    /// A data record's origin, media type and capture time.
    DataMetadata(String),
}

fn cosmetic() -> impl Strategy<Value = Cosmetic> {
    prop_oneof![
        vec(text(), 0..3).prop_map(Cosmetic::Tags),
        text().prop_map(Cosmetic::Timestamps),
        prop::sample::select(&[Lifecycle::Draft, Lifecycle::Paused, Lifecycle::Closed][..])
            .prop_map(Cosmetic::Lifecycle),
        text().prop_map(Cosmetic::UntestableReason),
        (
            text(),
            prop::sample::select(&[ExperimentStatus::Running, ExperimentStatus::Cancelled][..])
        )
            .prop_map(|(t, s)| Cosmetic::Experiment(t, s)),
        (text(), any::<bool>()).prop_map(|(t, r)| Cosmetic::Gap(t, r)),
        (text(), proptest::option::of(0.0..=1.0f64)).prop_map(|(t, c)| Cosmetic::Assessment(t, c)),
        text().prop_map(Cosmetic::DataMetadata),
    ]
}

impl Cosmetic {
    /// Whether the edit applies to `r`'s kind.
    fn fits(&self, r: &Record) -> bool {
        match self {
            Self::Tags(_) | Self::Timestamps(_) => true,
            Self::Lifecycle(_) | Self::UntestableReason(_) => {
                matches!(r.data, Data::Hypothesis { .. })
            }
            Self::Experiment(..) => matches!(r.data, Data::Experiment { .. }),
            Self::Gap(..) => matches!(r.data, Data::Gap { .. }),
            Self::Assessment(..) => matches!(r.data, Data::Assessment { .. }),
            Self::DataMetadata(_) => matches!(r.data, Data::Captured { .. }),
        }
    }
    /// Applies the edit to `r`; nothing where it does not `fit`.
    fn apply(&self, r: &mut Record) {
        match (self, &mut r.data) {
            (Self::Tags(tags), _) => r.tags.clone_from(tags),
            (Self::Timestamps(t), _) => {
                r.created_at.clone_from(t);
                r.updated_at.clone_from(t);
            }
            (Self::Lifecycle(l), Data::Hypothesis { lifecycle, .. }) => *lifecycle = *l,
            (
                Self::UntestableReason(s),
                Data::Hypothesis {
                    untestable_reason, ..
                },
            ) => {
                untestable_reason.clone_from(s);
            }
            (Self::Experiment(t, s), Data::Experiment { status, .. }) => {
                *status = *s;
                r.title.clone_from(t);
                r.body.clone_from(t);
            }
            (Self::Gap(t, resolved_now), Data::Gap { resolved, .. }) => {
                *resolved = *resolved_now;
                r.title.clone_from(t);
                r.body.clone_from(t);
            }
            (Self::Assessment(t, c), Data::Assessment { confidence, .. }) => {
                *confidence = *c;
                r.title.clone_from(t);
                r.body.clone_from(t);
            }
            (
                Self::DataMetadata(s),
                Data::Captured {
                    origin,
                    media_type,
                    captured_at,
                    ..
                },
            ) => {
                origin.clone_from(s);
                media_type.clone_from(s);
                captured_at.clone_from(s);
            }
            _ => {}
        }
    }
}

/// A change to a record in the basis, which the fingerprint must see.
#[derive(Debug, Clone, Copy)]
enum Content {
    Body,
    Archived,
    /// The field of its kind that the basis covers besides title and body:
    /// a hypothesis's scope, a prediction's conditions, a link's relation,
    /// evidence's source, a run's outcome (a criterion has none: its body).
    KindField,
    /// The bytes (sha256) of the first data record it references; its
    /// body where it references none.
    DataBytes,
}

fn content() -> impl Strategy<Value = Content> {
    prop::sample::select(
        &[
            Content::Body,
            Content::Archived,
            Content::KindField,
            Content::DataBytes,
        ][..],
    )
}

impl Content {
    /// Applies the change to record `id` of `records`.
    fn apply(self, records: &mut [Record], id: &str) {
        let at = |records: &[Record], id: &str| records.iter().position(|r| r.id == id);
        let n = at(records, id).expect("basis IDs are records");
        if let Self::DataBytes = self {
            let data = records[n].data_refs.first().and_then(|d| at(records, d));
            if let Some(Data::Captured { sha256, .. }) = data.map(|d| &mut records[d].data) {
                *sha256 = hash(sha256.as_bytes());
                return;
            }
        }
        let r = &mut records[n];
        match (self, &mut r.data) {
            (Self::Archived, Data::Run { .. }) | (Self::Body | Self::DataBytes, _) => {
                r.body.push('\u{394}')
            }
            (Self::Archived, _) => r.archived = !r.archived,
            (Self::KindField, Data::Hypothesis { scope, .. }) => scope.push('\u{394}'),
            (Self::KindField, Data::Prediction { conditions, .. }) => conditions.push('\u{394}'),
            (Self::KindField, Data::Link { relation, .. }) => {
                *relation = match relation {
                    Relation::Supports => Relation::Contradicts,
                    _ => Relation::Supports,
                };
            }
            (Self::KindField, Data::Evidence { source, .. }) => source.push('\u{394}'),
            (Self::KindField, Data::Run { outcome, .. }) => {
                *outcome = match outcome {
                    Outcome::Observed => Outcome::Failed,
                    _ => Outcome::Observed,
                };
            }
            (Self::KindField, _) => r.body.push('\u{394}'),
        }
    }
}

/// The hypothesis `rich` builds a basis for.
fn rich_hypothesis() -> String {
    id_of(Kind::Hypothesis, 100)
}

/// A notebook whose first record is a hypothesis with a basis of every
/// kind: a criterion and a prediction, evidence linked to each and to the
/// hypothesis, a run of its experiment citing more evidence, and a data
/// record the evidence draws on; then `notebook` records as noise.
/// Random notebooks alone rarely link evidence into a basis.
fn rich() -> impl Strategy<Value = Vec<Record>> {
    (
        vec(text(), 12),
        vec(prop::bool::weighted(0.2), 11),
        prop::sample::select(BLOBS),
        notebook(),
    )
        .prop_map(|(t, archived, bytes, noise)| {
            // Slots past the pool `notebook` draws from: no collisions.
            let id = |k: Kind, n: u128| id_of(k, 100 + n);
            let record = |id: String, n: usize, data: Data| {
                let mut r = Record::new(t[n].clone(), data);
                r.id = id;
                r.body.clone_from(&t[(n + 1) % t.len()]);
                r
            };
            let link = |n: u128, from: String, to: String, relation: Relation| {
                record(
                    id(Kind::Link, n),
                    n as usize,
                    Data::Link { from, to, relation },
                )
            };
            let evidence = |n: u128| {
                record(
                    id(Kind::Evidence, n),
                    3 + n as usize,
                    Data::Evidence {
                        source: t[n as usize].clone(),
                        locator: t[(n + 2) as usize].clone(),
                        observed_at: String::new(),
                        attachments: vec![],
                    },
                )
            };
            let h = rich_hypothesis();
            let mut records = vec![
                record(
                    h.clone(),
                    0,
                    Data::Hypothesis {
                        scope: t[2].clone(),
                        assumptions: t[3].clone(),
                        lifecycle: Lifecycle::Investigating,
                        untestable_reason: String::new(),
                    },
                ),
                record(
                    id(Kind::Criterion, 0),
                    1,
                    Data::Criterion {
                        hypothesis: h.clone(),
                    },
                ),
                record(
                    id(Kind::Prediction, 0),
                    2,
                    Data::Prediction {
                        hypothesis: h.clone(),
                        conditions: t[4].clone(),
                    },
                ),
                {
                    let mut e = evidence(0);
                    e.data_refs = vec![id(Kind::Data, 0)];
                    e
                },
                evidence(1),
                evidence(2),
                link(
                    0,
                    id(Kind::Evidence, 0),
                    id(Kind::Criterion, 0),
                    Relation::Supports,
                ),
                link(
                    1,
                    id(Kind::Evidence, 1),
                    id(Kind::Prediction, 0),
                    Relation::Contradicts,
                ),
                link(2, id(Kind::Evidence, 0), h.clone(), Relation::Qualifies),
                record(
                    id(Kind::Experiment, 0),
                    6,
                    Data::Experiment {
                        hypothesis: h.clone(),
                        targets: vec![],
                        status: ExperimentStatus::Completed,
                    },
                ),
                record(
                    id(Kind::Run, 0),
                    7,
                    Data::Run {
                        experiment: id(Kind::Experiment, 0),
                        plan: FrozenRef::default(),
                        outcome: Outcome::Observed,
                        evidence: vec![id(Kind::Evidence, 2)],
                    },
                ),
                record(
                    id(Kind::Data, 0),
                    8,
                    Data::Captured {
                        origin: t[8].clone(),
                        captured_at: String::new(),
                        media_type: "text/plain".into(),
                        size: bytes.len() as u64,
                        sha256: hash(bytes),
                    },
                ),
            ];
            for (r, a) in records.iter_mut().skip(1).zip(&archived) {
                r.archived = *a;
            }
            records.extend(noise);
            distinct(records)
        })
}

fn hypotheses(snap: &Snapshot) -> Vec<String> {
    snap.objects
        .iter()
        .filter(|e| matches!(e.record.data, Data::Hypothesis { .. }))
        .map(|e| e.record.id.clone())
        .collect()
}

fn fingerprints(snap: &Snapshot) -> Vec<(String, String)> {
    hypotheses(snap)
        .into_iter()
        .map(|h| {
            let f = snap.fingerprint(&h);
            (h, f)
        })
        .collect()
}

/// A plain record of `data`, titled and explained.
fn plain(title: &str, data: Data) -> Record {
    let mut r = Record::new(title, data);
    r.body = format!("{title}, explained.");
    r
}

/// `rich` builds what it says: with nothing archived, the basis of its
/// hypothesis holds the claim, its criterion and prediction, the three links,
/// all three observations (two linked, one cited by the run) and the run,
/// and not the experiment or the data record (whose hash counts under the
/// evidence instead). The fingerprint properties take basis members from
/// `basis` itself, so they alone would not notice a member missing.
#[test]
fn the_rich_basis_holds_every_record_built_for_it() {
    use proptest::{strategy::ValueTree, test_runner::TestRunner};
    let mut runner = TestRunner::deterministic();
    let mut records = rich().new_tree(&mut runner).unwrap().current();
    records.iter_mut().for_each(|r| r.archived = false);
    let snap = snapshot_of(records);
    let basis = snap.basis(&rich_hypothesis());
    let id = |k: Kind, n: u128| id_of(k, 100 + n);
    let expected = [
        id(Kind::Hypothesis, 0),
        id(Kind::Criterion, 0),
        id(Kind::Prediction, 0),
        id(Kind::Evidence, 0),
        id(Kind::Evidence, 1),
        id(Kind::Evidence, 2),
        id(Kind::Link, 0),
        id(Kind::Link, 1),
        id(Kind::Link, 2),
        id(Kind::Run, 0),
    ];
    for want in &expected {
        assert!(
            basis.contains_key(want),
            "{want} is not in the basis: {basis:#?}"
        );
    }
    for not in [id(Kind::Experiment, 0), id(Kind::Data, 0)] {
        assert!(!basis.contains_key(&not), "{not} is in the basis");
    }
    // The data record counts by its bytes, under the evidence that draws on it.
    let Some(Data::Captured { sha256, .. }) = snap.get(&id(Kind::Data, 0)).map(|e| &e.record.data)
    else {
        panic!("rich builds a data record")
    };
    assert_eq!(
        basis[&id(Kind::Evidence, 0)]["attachments"],
        serde_json::json!([sha256])
    );
}

proptest! {
    #![proptest_config(crate::cases(64))]

    /// Cosmetic edits of any records change no hypothesis's fingerprint.
    #[test]
    fn cosmetic_edits_keep_every_fingerprint(
        records in prop_oneof![rich(), notebook().prop_map(distinct)],
        edits in vec((any::<Index>(), cosmetic()), 1..6),
    ) {
        if records.is_empty() {
            return Ok(());
        }
        let before = fingerprints(&snapshot_of(records.clone()));
        let mut edited = records;
        for (i, edit) in &edits {
            // A record the edit fits, so that edits are rarely no-ops.
            let fitting: Vec<usize> = (0..edited.len()).filter(|&n| edit.fits(&edited[n])).collect();
            if !fitting.is_empty() {
                let n = *i.get(&fitting);
                edit.apply(&mut edited[n]);
            }
        }
        prop_assert_eq!(fingerprints(&snapshot_of(edited)), before);
    }

    /// Changing any record in a hypothesis's basis changes its fingerprint.
    #[test]
    fn content_edits_in_the_basis_change_the_fingerprint(
        records in prop_oneof![3 => rich(), 1 => notebook().prop_map(distinct)],
        hypothesis in any::<Index>(),
        member in any::<Index>(),
        edit in content(),
    ) {
        let snap = snapshot_of(records.clone());
        let mut hs = hypotheses(&snap);
        if hs.contains(&rich_hypothesis()) {
            hs = vec![rich_hypothesis()];
        }
        if hs.is_empty() {
            return Ok(());
        }
        let h = hypothesis.get(&hs);
        let basis = snap.basis(h);
        let keys: Vec<&String> = basis.keys().collect();
        prop_assert!(!keys.is_empty(), "a hypothesis is in its own basis");
        let target = member.get(&keys);
        let mut edited = records;
        edit.apply(&mut edited, target);
        let after = snapshot_of(edited);
        prop_assert_ne!(after.fingerprint(h), snap.fingerprint(h), "{:?} of {} kept the fingerprint of {}", edit, target, h);
    }
}

proptest! {
    #![proptest_config(crate::cases(8))]

    /// Through the store: after an assessment, cosmetic writes leave it
    /// current (`needs_review` false), and a write to its evidence makes
    /// it need review.
    #[test]
    fn an_assessment_needs_review_only_after_content_changes(
        tags in vec(vec("[a-z]{1,8}", 0..3), 1..4),
        lifecycle in prop::sample::select(&[Lifecycle::Investigating, Lifecycle::Paused, Lifecycle::Closed][..]),
        new_source in "[a-z]{1,8}",
    ) {
        let dir = tempfile::TempDir::new().unwrap();
        let store = Store::init(dir.path()).unwrap();
        let named = |id: &str, mut r: Record| {
            r.id = id.into();
            Change::create_seen(r, &Snapshot::default())
        };
        let committed = store.commit_written(vec![
            named("@h", plain("Cache misses cause the stall", Data::Hypothesis {
                scope: String::new(),
                assumptions: String::new(),
                lifecycle: Lifecycle::Draft,
                untestable_reason: String::new(),
            })),
            named("@f", plain("Rejected if the stall persists without misses", Data::Criterion { hypothesis: "@h".into() })),
            named("@e", plain("Stall with a warm cache", Data::Evidence {
                source: "perf.log".into(),
                locator: String::new(),
                observed_at: String::new(),
                attachments: vec![],
            })),
            named("@l", plain("Meets the criterion", Data::Link { from: "@e".into(), to: "@f".into(), relation: Relation::Supports })),
        ], None).unwrap();
        let id = |n: usize| committed.written[n].id.clone();
        let (h, f, e) = (id(0), id(1), id(2));
        let seen = store.snapshot().unwrap();
        store.commit(vec![Change::create_seen(plain("Falsified", Data::Assessment {
            hypothesis: h.clone(),
            judgment: Judgment::Falsified,
            confidence: Some(0.9),
            evidence: vec![e.clone()],
            criterion: Some(f),
            based_on: String::new(),
            supersedes: vec![],
        }), &seen)], None).unwrap();

        let state = |store: &Store| store.snapshot().unwrap().hypotheses[&h].clone();
        let patch = |id: &str, set: serde_json::Value| {
            let snap = store.snapshot().unwrap();
            let revision = snap.get(id).unwrap().revision.clone();
            store.commit(vec![Change::Patch {
                id: id.to_string(),
                expected_revision: revision,
                set: set.as_object().unwrap().clone(),
            }], None).unwrap();
        };
        let assessed = state(&store);
        prop_assert!(!assessed.needs_review);
        for t in &tags {
            patch(&h, serde_json::json!({"tags": t}));
            patch(&e, serde_json::json!({"tags": t}));
        }
        patch(&h, serde_json::json!({"lifecycle": lifecycle}));
        let after_cosmetic = state(&store);
        prop_assert!(!after_cosmetic.needs_review, "cosmetic writes flagged the assessment");
        prop_assert_eq!(&after_cosmetic.fingerprint, &assessed.fingerprint);

        patch(&e, serde_json::json!({"source": new_source}));
        prop_assert!(state(&store).needs_review, "a changed source left the assessment current");
    }
}
