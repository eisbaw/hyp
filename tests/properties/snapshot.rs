//! Validation and derivation over any set of records: dangling and
//! wrong-kind references, duplicate IDs, cycles, out-of-range values, huge
//! fields. Merges and hand edits can produce all of these, so every read
//! must report them as diagnostics, never panic.
use crate::generate::{BLOBS, distinct, notebook, snapshot_of};
use hyp::{
    model::*,
    store::{Change, DIRECTORIES, Store, Verify, encode},
};
use proptest::{collection::vec, prelude::*};
use std::{fs, path::Path};

/// A file that is not a record a writer produced: what a merge conflict,
/// a binary file or a truncated sync leaves.
#[derive(Debug, Clone)]
enum Stray {
    /// Bytes, often not UTF-8, as `<dir>/<name>.md`.
    Bytes(usize, String, Vec<u8>),
    /// A copy of record `n` under another name or directory.
    Copy(usize, usize, String),
}

fn stray() -> impl Strategy<Value = Stray> {
    let name = "[a-zA-Z0-9-]{1,12}";
    prop_oneof![
        (any::<usize>(), name, vec(any::<u8>(), 0..64))
            .prop_map(|(dir, name, bytes)| Stray::Bytes(dir, name, bytes)),
        (any::<usize>(), any::<usize>(), name)
            .prop_map(|(record, dir, name)| Stray::Copy(record, dir, name)),
    ]
}

/// A file name for the `n`th record, of ID `id`: the ID where it can be
/// one, else a stand-in, so the "filename must match object ID" rule gets
/// exercised too.
fn file_name(n: usize, id: &str) -> String {
    let usable = !id.is_empty()
        && id.len() < 200
        && !id.contains(['/', '\\', '\0'])
        && id != "."
        && id != "..";
    if usable {
        format!("{id}.md")
    } else {
        format!("unnamed-{n}.md")
    }
}

/// Writes `records` and `strays` into the notebook at `root` the way a
/// merge would leave them, and stores `BLOBS` for data records to name.
fn write_notebook(root: &Path, records: &[Record], strays: &[Stray]) {
    let hyp = root.join("hyp");
    for (n, r) in records.iter().enumerate() {
        // What `encode` refuses (`unicode_separator_bug`) no writer stores.
        let Ok(text) = encode(r) else { continue };
        let dir = hyp.join(r.data.directory());
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(file_name(n, &r.id)), text).unwrap();
    }
    for s in strays {
        let (dir, name, bytes) = match s {
            Stray::Bytes(dir, name, bytes) => (dir, name.clone(), bytes.clone()),
            Stray::Copy(_, _, _) if records.is_empty() => continue,
            Stray::Copy(n, dir, name) => (
                dir,
                format!("{name}.md"),
                encode(&records[n % records.len()])
                    .unwrap_or_default()
                    .into_bytes(),
            ),
        };
        let dir = hyp.join(DIRECTORIES[dir % DIRECTORIES.len()]);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(name), bytes).unwrap();
    }
    let assets = hyp.join("assets");
    fs::create_dir_all(&assets).unwrap();
    // The last blob stays missing, so some data records name absent bytes.
    for b in &BLOBS[..BLOBS.len() - 1] {
        fs::write(assets.join(hash(b)), b).unwrap();
    }
}

/// Every query the CLI and WebUI make of a snapshot, on every record.
fn query_everything(snap: &Snapshot) {
    for e in &snap.objects {
        let id = e.record.id.as_str();
        let _ = snap.validate(&e.record);
        let _ = snap.validate_new(&e.record);
        let _ = snap.find(id);
        let _ = snap.find(&id.chars().take(4).collect::<String>());
        let _ = snap.bears_on(id);
        let _ = Change::create_seen(e.record.clone(), snap);
        if matches!(e.record.data, Data::Hypothesis { .. }) {
            let _ = snap.basis(id);
            let _ = snap.fingerprint(id);
            let _ = snap.evidence_bearings(id);
            let _ = snap.assessment_heads(id);
            let _ = snap.is_live(id);
            let _ = snap.has_active_criterion(id);
        }
    }
    let _ = snap.unexplained();
    let _ = snap.assert_writable();
}

proptest! {
    #![proptest_config(crate::cases(32))]

    /// In memory, duplicates included: no query panics.
    #[test]
    fn queries_never_panic(records in notebook()) {
        query_everything(&snapshot_of(records));
    }

    /// From disk, with stray and damaged files: the read succeeds and
    /// reports every problem as a diagnostic, deterministically; a write
    /// to the damaged notebook fails or succeeds without panicking.
    #[test]
    fn reads_report_rather_than_fail(
        records in notebook(),
        strays in vec(stray(), 0..4),
        schema in 1u32..=3,
    ) {
        let dir = tempfile::TempDir::new().unwrap();
        let store = Store::init(dir.path()).unwrap();
        let config = match schema {
            3 => "schema_version = 3\nname = \"p\"\nmin_hyp_version = \"0.3.0\"\n".to_string(),
            n => format!("schema_version = {n}\nname = \"p\"\n"),
        };
        fs::write(dir.path().join("hyp/config.toml"), config).unwrap();
        write_notebook(dir.path(), &records, &strays);

        let first = store
            .read(Verify::Content)
            .map_err(|e| TestCaseError::fail(format!("read failed: {e:#}")))?;
        let again = store.read(Verify::Content).unwrap();
        prop_assert_eq!(&first.revision, &again.revision, "two reads of one notebook differ");
        query_everything(&first);

        let h = Record::new(
            "Added to a damaged notebook",
            Data::Hypothesis {
                scope: String::new(),
                assumptions: String::new(),
                lifecycle: Lifecycle::Draft,
                untestable_reason: "a probe".into(),
            },
        );
        let _ = store.commit(vec![Change::create_seen(h, &first)], None);
    }

    /// Read from disk, a notebook of distinct records holds each of them
    /// exactly as written, and nothing else; only a file not named after
    /// its record's ID (`file_name`'s stand-in) is reported instead.
    #[test]
    fn every_record_written_is_read_or_reported(records in notebook()) {
        let records: Vec<Record> = distinct(records)
            .into_iter()
            .filter(|r| encode(r).is_ok())
            .collect();
        let dir = tempfile::TempDir::new().unwrap();
        let store = Store::init(dir.path()).unwrap();
        write_notebook(dir.path(), &records, &[]);
        let snap = store.read(Verify::Metadata).unwrap();
        for e in &snap.objects {
            let written = records.iter().find(|r| r.id == e.record.id);
            prop_assert!(written.is_some(), "read a record nobody wrote: {}", e.record.id);
            prop_assert_eq!(format!("{:?}", e.record), format!("{:?}", written.unwrap()));
        }
        let malformed: Vec<&Diagnostic> =
            snap.diagnostics.iter().filter(|d| d.code == Code::Malformed).collect();
        for d in &malformed {
            prop_assert!(d.path.contains("/unnamed-"), "a written record is malformed: {d:?}");
        }
        prop_assert_eq!(snap.objects.len() + malformed.len(), records.len());
    }
}
