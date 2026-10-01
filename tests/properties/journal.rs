//! Rolling a crashed write forward. A commit writes its journal
//! (`.hyp/transaction.json`: path under `hyp/` -> new content, or null to
//! delete), applies it file by file, then removes it; the next read or
//! write that finds a journal applies it again from the start. So a crash
//! after any number of applied files must end, after recovery, in the
//! same notebook as no crash, and recovering twice must change nothing.
//!
//! The journal is written here directly, in the format hyp writes it: an
//! on-disk format that every later hyp must still roll forward.
use crate::{
    files,
    generate::{record, text},
};
use hyp::store::{Change, DIRECTORIES, Store, encode};
use proptest::{collection::vec, option, prelude::*, sample::Index};
use std::{collections::BTreeMap, fs, path::Path};

/// A file of the journal, by path relative to `hyp/`.
#[derive(Debug, Clone)]
enum Target {
    /// One of the files the notebook holds before the write.
    Existing(Index),
    /// A record file the notebook does not hold.
    New(Index, String),
    /// `config.toml`, which a write that raises the schema rewrites.
    Config,
}

fn content() -> impl Strategy<Value = String> {
    prop_oneof![
        3 => record().prop_filter_map("encode refuses it", |r| encode(&r).ok()),
        1 => text(),
    ]
}

fn relative(dir: &Index, name: &str) -> String {
    format!("{}/{name}.md", dir.get(DIRECTORIES))
}

/// What the journal does to a file: write this content, or delete it.
fn journal() -> impl Strategy<Value = Vec<(Target, Option<String>)>> {
    let target = prop_oneof![
        4 => any::<Index>().prop_map(Target::Existing),
        4 => (any::<Index>(), "[a-zA-Z0-9-]{1,12}").prop_map(|(d, n)| Target::New(d, n)),
        1 => Just(Target::Config),
    ];
    vec((target, option::weighted(0.7, content())), 1..8)
}

/// How recovery is set off: by a read, or by a write (which may then fail
/// on what the journal wrote, but recovers first).
#[derive(Debug, Clone, Copy)]
enum Trigger {
    Read,
    Write,
}

fn recover(store: &Store, trigger: Trigger) {
    match trigger {
        Trigger::Read => drop(store.snapshot()),
        Trigger::Write => drop(store.commit(Vec::<Change>::new(), None)),
    }
}

/// The schema configs a journal may write; all readable.
fn config(n: usize) -> String {
    match n % 3 {
        0 => "schema_version = 1\nname = \"p\"\n".into(),
        1 => "schema_version = 2\nname = \"p\"\n".into(),
        _ => "schema_version = 3\nname = \"p\"\nmin_hyp_version = \"0.3.0\"\n".into(),
    }
}

/// Applies `writes` to the files of `hyp/` at `notebook`, in the order
/// `recover` does, as a crashed write would have left the first of them.
fn apply(notebook: &Path, writes: &BTreeMap<String, Option<String>>, first: usize) {
    for (relative, content) in writes.iter().take(first) {
        let path = notebook.join(relative);
        match content {
            Some(c) => {
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(&path, c).unwrap();
            }
            None => {
                let _ = fs::remove_file(&path);
            }
        }
    }
}

proptest! {
    #![proptest_config(crate::cases(24))]

    /// Whatever prefix of the journal a crash applied, recovery ends in the
    /// notebook the whole journal makes, removes the journal, and a second
    /// recovery of the same journal changes nothing.
    #[test]
    fn recovery_is_idempotent(
        initial in vec((any::<Index>(), "[a-zA-Z0-9-]{1,12}", content()), 0..6),
        entries in journal(),
        crash_after in any::<Index>(),
        config_choice in any::<usize>(),
        trigger in prop_oneof![Just(Trigger::Read), Just(Trigger::Write)],
    ) {
        let dir = tempfile::TempDir::new().unwrap();
        let store = Store::init(dir.path()).unwrap();
        let notebook = dir.path().join("hyp");
        let journal = dir.path().join(".hyp/transaction.json");
        let initial: BTreeMap<String, Option<String>> = initial
            .iter()
            .map(|(d, n, c)| (relative(d, n), Some(c.clone())))
            .collect();
        apply(&notebook, &initial, usize::MAX);
        let existing: Vec<&String> = initial.keys().collect();
        let writes: BTreeMap<String, Option<String>> = entries
            .iter()
            .map(|(target, content)| {
                let path = match target {
                    Target::Existing(i) if !existing.is_empty() => i.get(&existing).to_string(),
                    Target::Existing(i) => relative(i, "absent"),
                    Target::New(d, n) => relative(d, n),
                    Target::Config => "config.toml".into(),
                };
                let content = match target {
                    // A config written is one this hyp reads; deleting it is
                    // not something a write does.
                    Target::Config => Some(config(config_choice)),
                    _ => content.clone(),
                };
                (path, content)
            })
            .collect();
        // The notebook the whole journal makes.
        let mut expected = files(&notebook);
        for (relative, content) in &writes {
            let path = notebook.join(relative);
            match content {
                Some(c) => expected.insert(path, c.clone().into_bytes()),
                None => expected.remove(&path),
            };
        }
        let first = crash_after.index(writes.len() + 1);
        apply(&notebook, &writes, first);
        let journal_bytes = serde_json::to_vec(&writes).unwrap();
        fs::write(&journal, &journal_bytes).unwrap();

        recover(&store, trigger);
        prop_assert!(!journal.exists(), "recovery left the journal");
        prop_assert_eq!(files(&notebook), expected.clone(), "crash after {} of {} files", first, writes.len());

        // A crash after applying every file, before removing the journal.
        fs::write(&journal, &journal_bytes).unwrap();
        recover(&store, trigger);
        prop_assert!(!journal.exists(), "the second recovery left the journal");
        prop_assert_eq!(files(&notebook), expected);
    }
}
