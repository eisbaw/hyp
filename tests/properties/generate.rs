//! Strategies shared by the properties: text that is hostile to YAML and
//! Markdown front matter, and records of every kind whose references are
//! drawn from a small pool of IDs, so that generated sets have matches,
//! dangling references, wrong kinds, duplicates and cycles in useful
//! proportions.
use hyp::model::*;
use proptest::{collection::vec, option, prelude::*, sample::select};

/// IDs come from this many slots per kind: few enough that references
/// often hit a generated record, some enough that some dangle.
pub const SLOTS: u128 = 6;

pub const KINDS: &[Kind] = &[
    Kind::Hypothesis,
    Kind::Prediction,
    Kind::Criterion,
    Kind::Evidence,
    Kind::Link,
    Kind::Experiment,
    Kind::Run,
    Kind::Assessment,
    Kind::Gap,
    Kind::Data,
];

/// Stored bytes that data records generated with `sha` may name; tests that
/// build notebooks on disk store these under `hyp/assets/`.
pub const BLOBS: &[&[u8]] = &[b"", b"hello\n", b"\x00\xffnot text"];

/// Fragments with a meaning in YAML, front matter, Markdown or Unicode line
/// breaking.
const NASTY: &[&str] = &[
    "---",
    "\n---\n",
    "---\n",
    "\n",
    "\r\n",
    "\r",
    ":",
    ": ",
    "- ",
    "#",
    "'",
    "\"",
    "\\",
    "{",
    "}",
    "[",
    "]",
    "&a ",
    "*a",
    "!!str ",
    "!tag ",
    "|",
    ">",
    "%YAML 1.2",
    "...",
    "\t",
    " ",
    "\u{feff}",
    "\u{0}",
    "\u{1b}",
    "\u{7f}",
    "\u{85}",
    "\u{2028}",
    "\u{2029}",
    "null",
    "~",
    "true",
    "yes",
    "on",
    "1e3",
    "0x1F",
    "0o17",
    ".nan",
    "-.inf",
    "kind: hypothesis",
    "id: H-1",
    "<<",
    "é",
    "\u{1F600}",
    "\u{10FFFF}",
];

/// Any text, biased towards what breaks serializers; also long lines of
/// words, which a YAML emitter may fold. Used for titles, scopes and every
/// other text field.
pub fn text() -> BoxedStrategy<String> {
    let nasty = || select(NASTY).prop_map(String::from);
    prop_oneof![
        6 => "[a-zA-Z0-9 .,]{0,24}",
        1 => "[a-z ]{60,300}",
        2 => any::<String>(),
        3 => vec(nasty(), 0..6).prop_map(|v| v.concat()),
        2 => vec(prop_oneof![nasty(), any::<String>()], 0..4).prop_map(|v| v.concat()),
    ]
    .boxed()
}

/// `text`, rarely very long (huge fields).
pub fn long_text() -> BoxedStrategy<String> {
    prop_oneof![
        100 => text(),
        1 => (1usize..70_000, select(&["x", "é", "\n", "- ", "---\n"][..]))
            .prop_map(|(n, s)| s.repeat(n)),
    ]
    .boxed()
}

/// The full ID in `slot` of `kind`.
pub fn id_of(kind: Kind, slot: u128) -> String {
    format!("{}-{}", kind.prefix(), uuid::Uuid::from_u128(slot + 1))
}

/// Not a full ID: empty, a short prefix, a batch reference or any text.
fn junk_id() -> BoxedStrategy<String> {
    prop_oneof![
        Just(String::new()),
        Just("H-0000".to_string()),
        Just("@ref".to_string()),
        Just("../../config".to_string()),
        text(),
    ]
    .boxed()
}

/// A reference to one of `kinds`, sometimes to another kind or no ID.
pub fn reference(kinds: &'static [Kind]) -> BoxedStrategy<String> {
    prop_oneof![
        12 => (select(kinds), 0..SLOTS).prop_map(|(k, i)| id_of(k, i)),
        3 => (select(KINDS), 0..SLOTS).prop_map(|(k, i)| id_of(k, i)),
        1 => junk_id(),
    ]
    .boxed()
}

const HYPOTHESIS: &[Kind] = &[Kind::Hypothesis];
const EVIDENCE: &[Kind] = &[Kind::Evidence];
const CLAIMS: &[Kind] = &[Kind::Hypothesis, Kind::Criterion, Kind::Prediction];

/// A SHA-256 of one of `BLOBS`, of no stored bytes, or not one at all.
pub fn sha() -> BoxedStrategy<String> {
    prop_oneof![
        4 => select(BLOBS).prop_map(hash),
        1 => "[0-9a-f]{64}",
        1 => text(),
    ]
    .boxed()
}

fn frozen() -> impl Strategy<Value = FrozenRef> {
    (reference(KINDS), text(), text(), text()).prop_map(|(id, revision, title, body)| FrozenRef {
        id,
        revision,
        title,
        body,
    })
}

fn confidence() -> impl Strategy<Value = Option<f64>> {
    prop_oneof![
        Just(None),
        (0.0..=1.0f64).prop_map(Some),
        any::<f64>().prop_map(Some),
    ]
}

/// The kind-specific fields of a `kind` record.
pub fn data(kind: Kind) -> BoxedStrategy<Data> {
    match kind {
        Kind::Hypothesis => (
            text(),
            text(),
            select(
                &[
                    Lifecycle::Draft,
                    Lifecycle::Investigating,
                    Lifecycle::Paused,
                    Lifecycle::Closed,
                ][..],
            ),
            prop_oneof![Just(String::new()), text()],
        )
            .prop_map(
                |(scope, assumptions, lifecycle, untestable_reason)| Data::Hypothesis {
                    scope,
                    assumptions,
                    lifecycle,
                    untestable_reason,
                },
            )
            .boxed(),
        Kind::Prediction => (reference(HYPOTHESIS), text())
            .prop_map(|(hypothesis, conditions)| Data::Prediction {
                hypothesis,
                conditions,
            })
            .boxed(),
        Kind::Criterion => reference(HYPOTHESIS)
            .prop_map(|hypothesis| Data::Criterion { hypothesis })
            .boxed(),
        Kind::Evidence => (
            text(),
            text(),
            text(),
            vec(
                (prop_oneof![Just("note.txt".to_string()), text()], sha())
                    .prop_map(|(path, sha256)| Attachment { path, sha256 }),
                0..2,
            ),
        )
            .prop_map(
                |(source, locator, observed_at, attachments)| Data::Evidence {
                    source,
                    locator,
                    observed_at,
                    attachments,
                },
            )
            .boxed(),
        Kind::Link => (reference(EVIDENCE), reference(CLAIMS), relation())
            .prop_map(|(from, to, relation)| Data::Link { from, to, relation })
            .boxed(),
        Kind::Experiment => (
            reference(HYPOTHESIS),
            vec(frozen(), 0..2),
            select(
                &[
                    ExperimentStatus::Planned,
                    ExperimentStatus::Running,
                    ExperimentStatus::Completed,
                    ExperimentStatus::Cancelled,
                ][..],
            ),
        )
            .prop_map(|(hypothesis, targets, status)| Data::Experiment {
                hypothesis,
                targets,
                status,
            })
            .boxed(),
        Kind::Run => (
            reference(&[Kind::Experiment]),
            frozen(),
            select(&[Outcome::Observed, Outcome::Inconclusive, Outcome::Failed][..]),
            vec(reference(EVIDENCE), 0..3),
        )
            .prop_map(|(experiment, plan, outcome, evidence)| Data::Run {
                experiment,
                plan,
                outcome,
                evidence,
            })
            .boxed(),
        Kind::Assessment => (
            reference(HYPOTHESIS),
            judgment(),
            confidence(),
            vec(reference(EVIDENCE), 0..3),
            option::of(reference(&[Kind::Criterion])),
            prop_oneof![Just(String::new()), sha(), text()],
            vec(reference(&[Kind::Assessment]), 0..2),
        )
            .prop_map(
                |(hypothesis, judgment, confidence, evidence, criterion, based_on, supersedes)| {
                    Data::Assessment {
                        hypothesis,
                        judgment,
                        confidence,
                        evidence,
                        criterion,
                        based_on,
                        supersedes,
                    }
                },
            )
            .boxed(),
        Kind::Gap => (
            reference(HYPOTHESIS),
            any::<bool>(),
            vec(reference(EVIDENCE), 0..2),
        )
            .prop_map(|(hypothesis, resolved, resolved_by)| Data::Gap {
                hypothesis,
                resolved,
                resolved_by,
            })
            .boxed(),
        Kind::Data => (
            text(),
            text(),
            prop_oneof![
                Just("text/plain".to_string()),
                Just("application/octet-stream".to_string()),
                Just(String::new()),
                text(),
            ],
            prop_oneof![(0u64..16), any::<u64>()],
            sha(),
        )
            .prop_map(
                |(origin, captured_at, media_type, size, sha256)| Data::Captured {
                    origin,
                    captured_at,
                    media_type,
                    size,
                    sha256,
                },
            )
            .boxed(),
    }
}

pub fn relation() -> impl Strategy<Value = Relation> {
    select(
        &[
            Relation::Supports,
            Relation::Contradicts,
            Relation::Qualifies,
            Relation::DependsOn,
            Relation::CompetesWith,
            Relation::Supersedes,
        ][..],
    )
}

pub fn judgment() -> impl Strategy<Value = Judgment> {
    select(
        &[
            Judgment::Untested,
            Judgment::Inconclusive,
            Judgment::Supported,
            Judgment::Weakened,
            Judgment::Falsified,
        ][..],
    )
}

/// A `kind` record: mostly with an ID of its kind from the pool,
/// sometimes with one of another kind or none.
pub fn record_of(kind: Kind) -> BoxedStrategy<Record> {
    let id = prop_oneof![
        8 => (0..SLOTS).prop_map(move |i| id_of(kind, i)),
        1 => reference(KINDS),
        1 => junk_id(),
    ];
    (
        id,
        text(),
        long_text(),
        vec(text(), 0..3),
        vec(reference(&[Kind::Data]), 0..2),
        any::<bool>(),
        text(),
        text(),
        data(kind),
    )
        .prop_map(
            |(id, title, body, tags, data_refs, archived, created_at, updated_at, data)| Record {
                id,
                title,
                body,
                tags,
                data_refs,
                archived,
                created_at,
                updated_at,
                data,
            },
        )
        .boxed()
}

/// A record of any kind.
pub fn record() -> BoxedStrategy<Record> {
    select(KINDS).prop_flat_map(record_of).boxed()
}

/// A set of records: what a notebook may hold after merges and hand edits.
pub fn notebook() -> BoxedStrategy<Vec<Record>> {
    vec(record(), 0..24).boxed()
}

/// `records` with each ID once (the first record of an ID kept), as a
/// notebook read from disk holds them.
pub fn distinct(records: Vec<Record>) -> Vec<Record> {
    let mut seen = std::collections::BTreeSet::new();
    records
        .into_iter()
        .filter(|r| seen.insert(r.id.clone()))
        .collect()
}

/// A snapshot of `records` as read, derived, without diagnostics.
pub fn snapshot_of(records: Vec<Record>) -> Snapshot {
    let objects = records
        .into_iter()
        .map(|record| Entry {
            revision: hash(format!("{record:?}")),
            record,
        })
        .collect();
    let mut snap = Snapshot {
        objects,
        ..Snapshot::default()
    };
    snap.derive();
    snap
}

/// Whether a front-matter string of `r` holds a line feed and ends with
/// U+2028 or U+2029: the input `encode` refuses, pinned by the ignored
/// `codec::a_last_field_ending_in_a_unicode_separator_is_readable`.
/// Broader than the bug, which needs the field to be the last of the front
/// matter.
pub fn unicode_separator_bug(r: &Record) -> bool {
    fn any(v: &serde_json::Value) -> bool {
        match v {
            serde_json::Value::String(s) => {
                s.contains('\n') && s.ends_with(['\u{2028}', '\u{2029}'])
            }
            serde_json::Value::Array(a) => a.iter().any(any),
            serde_json::Value::Object(o) => o.values().any(any),
            _ => false,
        }
    }
    let mut header = r.clone();
    header.body.clear();
    any(&serde_json::to_value(header).expect("records serialize"))
}
