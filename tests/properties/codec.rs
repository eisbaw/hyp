//! The record file format: YAML front matter, then the body.
use crate::generate::{BLOBS, record, unicode_separator_bug};
use hyp::store::{decode, encode};
use proptest::{collection::vec, prelude::*};

/// Bytes with a meaning in front matter, to insert.
const SYNTAX: &[&[u8]] = &[b"\n---\n", b"---", b": ", b"\n  ", b"\"", b"kind: "];

/// One edit of a file's bytes, at a position taken modulo its length.
#[derive(Debug, Clone)]
enum Edit {
    Delete(usize, usize),
    Insert(usize, Vec<u8>),
    Replace(usize, u8),
    Duplicate(usize, usize),
}

fn edit() -> impl Strategy<Value = Edit> {
    prop_oneof![
        (any::<usize>(), 1usize..16).prop_map(|(at, n)| Edit::Delete(at, n)),
        (
            any::<usize>(),
            prop_oneof![
                vec(any::<u8>(), 1..8),
                prop::sample::select(SYNTAX).prop_map(<[u8]>::to_vec),
            ]
        )
            .prop_map(|(at, bytes)| Edit::Insert(at, bytes)),
        (any::<usize>(), any::<u8>()).prop_map(|(at, b)| Edit::Replace(at, b)),
        (any::<usize>(), 1usize..32).prop_map(|(at, n)| Edit::Duplicate(at, n)),
    ]
}

fn apply(bytes: &mut Vec<u8>, e: &Edit) {
    let at = |i: usize, len: usize| if len == 0 { 0 } else { i % (len + 1) };
    match e {
        Edit::Delete(i, n) => {
            let start = at(*i, bytes.len());
            let end = (start + n).min(bytes.len());
            bytes.drain(start..end);
        }
        Edit::Insert(i, b) => {
            let start = at(*i, bytes.len());
            bytes.splice(start..start, b.iter().copied());
        }
        Edit::Replace(i, b) => {
            if !bytes.is_empty() {
                let i = i % bytes.len();
                bytes[i] = *b;
            }
        }
        Edit::Duplicate(i, n) => {
            let start = at(*i, bytes.len());
            let end = (start + n).min(bytes.len());
            let copy = bytes[start..end].to_vec();
            bytes.splice(end..end, copy);
        }
    }
}

/// A record file damaged by `edits`, or arbitrary bytes, as text the way
/// a reader that replaces invalid UTF-8 would see it.
fn damaged() -> impl Strategy<Value = String> {
    prop_oneof![
        3 => (record(), vec(edit(), 1..6)).prop_map(|(r, edits)| {
            let mut bytes = encode(&r).unwrap_or_default().into_bytes();
            edits.iter().for_each(|e| apply(&mut bytes, e));
            String::from_utf8_lossy(&bytes).into_owned()
        }),
        1 => vec(any::<u8>(), 0..512).prop_map(|b| String::from_utf8_lossy(&b).into_owned()),
        1 => prop::sample::select(BLOBS).prop_map(|b| String::from_utf8_lossy(b).into_owned()),
    ]
}

proptest! {
    #![proptest_config(crate::cases(256))]

    /// Every record survives a write and a read unchanged, `Debug` being the
    /// strictest comparison available (it tells -0.0 from 0.0 and shows
    /// NaN), and its file is a fixed point of decode-then-encode. `encode`
    /// refuses only the input of the known bug (`unicode_separator_bug`).
    #[test]
    fn decode_inverts_encode(r in record()) {
        let file = match encode(&r) {
            Ok(file) => file,
            Err(e) => {
                prop_assert!(unicode_separator_bug(&r), "encode refused {:?}: {:#}", r, e);
                return Ok(());
            }
        };
        let back = decode(&file).map_err(|e| TestCaseError::fail(format!("{e:#}\nfile: {file:?}")))?;
        prop_assert_eq!(format!("{back:?}"), format!("{r:?}"), "file: {:?}", file);
        prop_assert_eq!(encode(&back).expect("encodes"), file);
    }

    /// `decode` returns an error rather than panicking on damaged files,
    /// and whatever it accepts round-trips like a written record.
    #[test]
    fn decode_never_panics_and_accepts_only_what_round_trips(file in damaged()) {
        if let Ok(r) = decode(&file) {
            let again = match encode(&r) {
                Ok(again) => again,
                Err(e) => {
                    prop_assert!(unicode_separator_bug(&r), "encode refused {:?}: {:#}", r, e);
                    return Ok(());
                }
            };
            let back = decode(&again)
                .map_err(|e| TestCaseError::fail(format!("{e:#}\nfile: {again:?}")))?;
            prop_assert_eq!(format!("{back:?}"), format!("{r:?}"));
        }
    }
}

/// KNOWN BUG, found by `decode_inverts_encode`; ignored until fixed.
///
/// A multi-line value that ends with U+2028 or U+2029 (Unicode line and
/// paragraph separators, as in text pasted from web pages) in the last
/// front-matter field (a prediction's `conditions`, a hypothesis's
/// `untestable_reason`, an evidence's `observed_at`) is written by
/// serde_yaml as a block scalar that ends with that separator and no line
/// feed, so the closing `---` would not start a line and `decode` could
/// not read the file back. `encode` now refuses such a record (fail fast,
/// nothing is written); storing the value needs a format decision: escape
/// it in `encode`, or let `decode` accept the delimiter after any YAML
/// line break.
#[test]
#[ignore = "known bug: hyp cannot store such a value yet (see doc comment)"]
fn a_last_field_ending_in_a_unicode_separator_is_readable() {
    use hyp::model::{Data, Kind, Record};
    for separator in ['\u{2028}', '\u{2029}'] {
        let r = Record::new(
            "Faster when warm",
            Data::Prediction {
                hypothesis: crate::generate::id_of(Kind::Hypothesis, 0),
                conditions: format!("after a warm-up run\nwith caches on{separator}"),
            },
        );
        let file = encode(&r).unwrap();
        let back = decode(&file).unwrap_or_else(|e| panic!("{e:#}: {file:?}"));
        assert_eq!(format!("{back:?}"), format!("{r:?}"));
    }
}

/// Until the bug above is fixed, a write of such a value fails and
/// leaves the notebook as it was, instead of writing a file hyp cannot read.
#[test]
fn a_value_hyp_cannot_store_is_refused_not_written() {
    use hyp::{
        model::{Data, Lifecycle, Record, Snapshot},
        store::{Change, Store, Verify},
    };
    let dir = tempfile::TempDir::new().unwrap();
    let store = Store::init(dir.path()).unwrap();
    let mut h = Record::new(
        "Faster when warm",
        Data::Hypothesis {
            scope: String::new(),
            assumptions: String::new(),
            lifecycle: Lifecycle::Draft,
            untestable_reason: "pasted\nfrom a web page\u{2028}".into(),
        },
    );
    h.id = "@h".into();
    let before = crate::files(&dir.path().join("hyp"));
    let err = store
        .commit(vec![Change::create_seen(h, &Snapshot::default())], None)
        .unwrap_err();
    // Error kinds are contract (decision-0002): input to fix.
    assert_eq!(
        hyp::error::kind_of(&err),
        hyp::error::ErrorKind::InvalidInput,
        "{err:#}"
    );
    let message = format!("{err:#}");
    assert!(message.contains("U+2028"), "{message}");
    assert!(message.contains("untestable_reason"), "{message}");
    assert_eq!(crate::files(&dir.path().join("hyp")), before);
    assert!(store.read(Verify::Content).unwrap().diagnostics.is_empty());
}

/// Found by `decode_inverts_encode`: the last field of the front matter,
/// written as a YAML block scalar, lost its final newline on read, because
/// `decode` passed the front matter without its last line break. For a
/// prediction that field is `conditions`, part of the review basis.
#[test]
fn a_trailing_newline_in_the_last_front_matter_field_survives() {
    use hyp::model::{Data, Kind, Record};
    let r = Record::new(
        "Faster when warm",
        Data::Prediction {
            hypothesis: crate::generate::id_of(Kind::Hypothesis, 0),
            conditions: "after a warm-up run\nwith caches on\n".into(),
        },
    );
    let back = decode(&encode(&r).unwrap()).unwrap();
    let Data::Prediction { conditions, .. } = back.data else {
        panic!("a prediction decodes as one")
    };
    assert_eq!(conditions, "after a warm-up run\nwith caches on\n");
}
