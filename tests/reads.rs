//! Reads and their locks (HYPO-0004): a read never waits for a whole write,
//! only for a transaction being applied; it never sees a transaction half
//! applied; and a notebook on read-only media can be read.
use fs2::FileExt;
use hyp::{
    model::*,
    store::{Capture, Change, Store, Verify, encode},
};
use std::{
    fs::{File, OpenOptions},
    path::Path,
    process::{Command, Output, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use tempfile::TempDir;

fn project() -> (TempDir, Store) {
    let dir = TempDir::new().unwrap();
    let store = Store::init(dir.path()).unwrap();
    (dir, store)
}
fn hypothesis(title: &str) -> Record {
    Record::new(
        title,
        Data::Hypothesis {
            scope: String::new(),
            assumptions: String::new(),
            lifecycle: Lifecycle::Draft,
            untestable_reason: String::new(),
        },
    )
}
/// Lock file `name` under `.hyp/`, held exclusively until dropped.
fn hold(store: &Store, name: &str) -> File {
    let f = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(store.root.join(".hyp").join(name))
        .unwrap();
    f.lock_exclusive().unwrap();
    f
}
/// Starts a snapshot on another thread; its result arrives on the receiver.
fn read_in_background(store: &Store) -> mpsc::Receiver<anyhow::Result<Snapshot>> {
    let (tx, rx) = mpsc::channel();
    let store = store.clone();
    std::thread::spawn(move || tx.send(store.snapshot()).unwrap());
    rx
}
fn hyp(project: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_hyp"))
        .arg("--project")
        .arg(project)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}
fn ok(project: &Path, args: &[&str]) -> String {
    let out = hyp(project, args);
    assert!(
        out.status.success(),
        "hyp {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

/// A writer holds the write lock for its whole commit (planning, checks,
/// hashing cited bytes). Reads do not take it, so the WebUI's polls and
/// other commands go on meanwhile.
#[test]
fn a_read_does_not_wait_for_a_write_in_progress() {
    let (_d, store) = project();
    let _writer = hold(&store, "write.lock");
    let read = read_in_background(&store);
    let snap = read
        .recv_timeout(Duration::from_secs(10))
        .expect("the read waited for the write lock")
        .unwrap();
    assert!(snap.objects.is_empty());
}

/// While a transaction is applied (its journal written and its files
/// renamed into place, under the apply lock), a read waits, and then sees
/// the whole transaction: never some of its files without the others.
#[test]
fn a_read_waits_while_a_transaction_is_applied_and_sees_all_of_it() {
    let (_d, store) = project();
    let (a, b) = (hypothesis("First"), hypothesis("Second"));
    let path = |r: &Record| format!("hypotheses/{}.md", r.id);
    let applying = hold(&store, "apply.lock");
    // A writer mid-transaction: the journal names both files, one is in place.
    let journal =
        serde_json::json!({ path(&a): encode(&a).unwrap(), path(&b): encode(&b).unwrap() });
    std::fs::write(
        store.root.join(".hyp/transaction.json"),
        journal.to_string(),
    )
    .unwrap();
    std::fs::write(store.root.join("hyp").join(path(&a)), encode(&a).unwrap()).unwrap();
    let read = read_in_background(&store);
    assert!(
        read.recv_timeout(Duration::from_millis(300)).is_err(),
        "a read went ahead while a transaction was applied"
    );
    // The writer finishes: the other file, then the journal goes.
    std::fs::write(store.root.join("hyp").join(path(&b)), encode(&b).unwrap()).unwrap();
    std::fs::remove_file(store.root.join(".hyp/transaction.json")).unwrap();
    drop(applying);
    let snap = read.recv_timeout(Duration::from_secs(10)).unwrap().unwrap();
    let ids: Vec<&str> = snap.objects.iter().map(|e| e.record.id.as_str()).collect();
    assert_eq!(ids.len(), 2, "{ids:?}");
}

/// A write that crashed while applying its journal left one file of two in
/// place. The next read rolls the journal forward first (it may write for
/// that), so it sees both records, and the journal is gone.
#[test]
fn a_read_after_a_crash_mid_transaction_sees_the_whole_transaction() {
    let (_d, store) = project();
    let (a, b) = (hypothesis("First"), hypothesis("Second"));
    let path = |r: &Record| format!("hypotheses/{}.md", r.id);
    let journal =
        serde_json::json!({ path(&a): encode(&a).unwrap(), path(&b): encode(&b).unwrap() });
    std::fs::write(
        store.root.join(".hyp/transaction.json"),
        journal.to_string(),
    )
    .unwrap();
    std::fs::write(store.root.join("hyp").join(path(&a)), encode(&a).unwrap()).unwrap();
    // The read waits for no writer: none holds the write lock.
    let snap = store.snapshot().unwrap();
    assert_eq!(snap.objects.len(), 2);
    assert!(snap.get(&b.id).is_some());
    assert!(!store.root.join(".hyp/transaction.json").exists());
}

fn create(r: Record) -> Change {
    Change::Create {
        record: r,
        expected: None,
    }
}
/// Starts a commit on another thread; its result arrives on the receiver.
fn write_in_background(store: &Store, changes: Vec<Change>) -> mpsc::Receiver<anyhow::Result<()>> {
    let (tx, rx) = mpsc::channel();
    let store = store.clone();
    std::thread::spawn(move || tx.send(store.commit(changes, None).map(drop)).unwrap());
    rx
}

/// Linux file locks do not favour writers: with reads overlapping all the
/// time (the WebUI polling, several tabs and commands), a writer waiting
/// for the apply lock would wait for a moment when none holds it, which
/// may never come. Readers queue behind a waiting writer (the gate lock).
#[test]
fn a_write_is_not_starved_by_reads_that_never_stop() {
    let (_d, store) = project();
    let many = (0..300)
        .map(|i| create(hypothesis(&format!("H{i}"))))
        .collect();
    store.commit(many, None).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let readers: Vec<_> = (0..6)
        .map(|_| {
            let (store, stop) = (store.clone(), stop.clone());
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    store.snapshot().unwrap();
                }
            })
        })
        .collect();
    std::thread::sleep(Duration::from_millis(300));
    let started = Instant::now();
    let write = write_in_background(&store, vec![create(hypothesis("Late"))]);
    let done = write.recv_timeout(Duration::from_secs(10));
    let took = started.elapsed();
    stop.store(true, Ordering::Relaxed);
    readers.into_iter().for_each(|r| r.join().unwrap());
    done.expect("the write was starved by reads").unwrap();
    assert!(took < Duration::from_secs(5), "the write took {took:?}");
}

/// A write takes the apply lock exclusively before applying its journal, so
/// it waits for a read in progress (here: the shared lock held by the test).
#[test]
fn a_write_waits_for_a_read_in_progress_before_applying() {
    let (_d, store) = project();
    let reading = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(store.root.join(".hyp/apply.lock"))
        .unwrap();
    FileExt::lock_shared(&reading).unwrap();
    let write = write_in_background(&store, vec![create(hypothesis("Waits"))]);
    assert!(
        write.recv_timeout(Duration::from_millis(300)).is_err(),
        "the write applied its journal during a read"
    );
    assert!(
        store
            .root
            .join("hyp/hypotheses")
            .read_dir()
            .unwrap()
            .next()
            .is_none()
    );
    drop(reading);
    write
        .recv_timeout(Duration::from_secs(10))
        .unwrap()
        .unwrap();
    assert_eq!(store.snapshot().unwrap().objects.len(), 1);
}

/// A read that finds a journal rolls it forward only under the write lock,
/// as writers do: a journal next to a held write lock may be one a writer
/// (one that does not take the apply lock: hyp before commit 9a62a67) is applying, and
/// replaying it would undo what that writer writes after it.
#[test]
fn a_read_that_finds_a_journal_waits_for_the_write_lock() {
    let (_d, store) = project();
    let h = hypothesis("Journaled");
    let journal = serde_json::json!({ format!("hypotheses/{}.md", h.id): encode(&h).unwrap() });
    let writer = hold(&store, "write.lock");
    std::fs::write(
        store.root.join(".hyp/transaction.json"),
        journal.to_string(),
    )
    .unwrap();
    let read = read_in_background(&store);
    assert!(
        read.recv_timeout(Duration::from_millis(300)).is_err(),
        "the read rolled a journal forward without the write lock"
    );
    // The writer finishes and removes its journal; the read then finds none.
    std::fs::remove_file(store.root.join(".hyp/transaction.json")).unwrap();
    drop(writer);
    let snap = read.recv_timeout(Duration::from_secs(10)).unwrap().unwrap();
    assert!(
        snap.objects.is_empty(),
        "the finished journal was not replayed"
    );
}

/// Writers create a hypothesis and a criterion of it in one transaction
/// while readers read: no read ever sees the criterion without its
/// hypothesis. A journal applies `criteria/` before `hypotheses/` (sorted
/// paths) and a read lists `hypotheses/` first, so a read between the two
/// renames, without the apply lock, would see exactly that.
#[test]
fn reads_never_see_a_criterion_without_the_hypothesis_created_with_it() {
    let (_d, store) = project();
    // Enough records that a read spends a while between the two directories.
    let many = (0..200)
        .map(|i| create(hypothesis(&format!("H{i}"))))
        .collect();
    store.commit(many, None).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let readers: Vec<_> = (0..3)
        .map(|_| {
            let (store, stop) = (store.clone(), stop.clone());
            std::thread::spawn(move || {
                let mut reads = 0;
                while !stop.load(Ordering::Relaxed) {
                    let s = store.snapshot().unwrap();
                    let dangling: Vec<_> = s
                        .diagnostics
                        .iter()
                        .filter(|d| d.code == Code::DanglingReference)
                        .collect();
                    assert!(dangling.is_empty(), "{dangling:?}");
                    reads += 1;
                }
                reads
            })
        })
        .collect();
    let writers: Vec<_> = (0..3)
        .map(|w| {
            let store = store.clone();
            std::thread::spawn(move || {
                for i in 0..10 {
                    let h = hypothesis(&format!("Claim {w}-{i}"));
                    let f = Record::new(
                        "Refuted if no timeout",
                        Data::Criterion {
                            hypothesis: h.id.clone(),
                        },
                    );
                    store.commit(vec![create(h), create(f)], None).unwrap();
                }
            })
        })
        .collect();
    writers.into_iter().for_each(|w| w.join().unwrap());
    stop.store(true, Ordering::Relaxed);
    let reads: usize = readers.into_iter().map(|r| r.join().unwrap()).sum();
    assert!(reads > 0);
    assert_eq!(store.snapshot().unwrap().objects.len(), 200 + 3 * 10 * 2);
}

/// `hyp check` hashes every stored file, which takes long on a large
/// notebook. It does that after letting go of its shared lock, so a writer
/// that queues meanwhile is not held up, nor are the reads queued behind
/// that writer (`hyp list`, the WebUI).
#[test]
fn a_check_hashing_stored_bytes_does_not_hold_up_reads_behind_a_writer() {
    let (_d, store) = project();
    // A real capture raises the notebook to the data schema.
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
    // Large stored files cost nothing on disk when sparse. Their content
    // does not have the hash they are named by, which check reports.
    let size: u64 = 32 << 20;
    let add_blob = |i: usize| {
        let sha256 = format!("{i:064x}");
        File::create(store.root.join("hyp/assets").join(&sha256))
            .unwrap()
            .set_len(size)
            .unwrap();
        let mut r = Record::new(
            format!("Large {i}"),
            Data::Captured {
                origin: "test".into(),
                captured_at: String::new(),
                media_type: "application/octet-stream".into(),
                size,
                sha256,
            },
        );
        r.created_at = chrono::Utc::now().to_rfc3339();
        r.updated_at = r.created_at.clone();
        if let Data::Captured { captured_at, .. } = &mut r.data {
            captured_at.clone_from(&r.created_at);
        }
        std::fs::write(
            store.root.join(format!("hyp/data/{}.md", r.id)),
            encode(&r).unwrap(),
        )
        .unwrap();
    };
    let timed = |f: &dyn Fn()| {
        let started = Instant::now();
        f();
        started.elapsed()
    };
    // Enough bytes that hashing them clearly takes longer than a read.
    let mut blobs = 0;
    let check = loop {
        blobs += 1;
        add_blob(blobs);
        let took = timed(&|| drop(store.read(Verify::Content).unwrap()));
        if took >= Duration::from_millis(400) || blobs == 48 {
            break took;
        }
    };
    let checked = {
        let store = store.clone();
        std::thread::spawn(move || store.read(Verify::Content).unwrap())
    };
    std::thread::sleep(check / 10);
    let write = write_in_background(&store, vec![create(hypothesis("Queued"))]);
    std::thread::sleep(check / 10);
    let list = timed(&|| drop(store.snapshot().unwrap()));
    assert!(
        list < check / 2,
        "a read waited {list:?} behind a writer queued behind a check of {check:?}"
    );
    write
        .recv_timeout(Duration::from_secs(30))
        .unwrap()
        .unwrap();
    let changed = checked
        .join()
        .unwrap()
        .diagnostics
        .iter()
        .filter(|d| d.code == Code::ChangedBytes)
        .count();
    assert_eq!(changed, blobs, "every sparse blob is reported");
}

/// Lock files are regular files. Anything else in their place is refused
/// with the fix, not followed (a symlink) or waited on (a FIFO, which a
/// read-only open would wait on forever).
#[test]
fn lock_files_that_are_not_regular_files_are_refused() {
    let (_d, store) = project();
    let gate = store.root.join(".hyp/gate.lock");
    std::fs::remove_file(&gate).unwrap();
    let fifo = std::ffi::CString::new(gate.to_str().unwrap()).unwrap();
    // Read-only, so that hyp opens it read-only: that open is the one a
    // FIFO makes wait for a writer.
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o444) }, 0);
    let read = read_in_background(&store);
    let err = read
        .recv_timeout(Duration::from_secs(10))
        .expect("a read waited on a FIFO")
        .unwrap_err();
    let message = format!("{err:#}");
    assert!(
        message.contains("gate.lock is not a regular file") && message.contains("remove it"),
        "{message}"
    );
    std::fs::remove_file(&gate).unwrap();
    std::os::unix::fs::symlink(store.root.join("elsewhere"), &gate).unwrap();
    let message = format!("{:#}", store.snapshot().unwrap_err());
    assert!(
        message.contains("gate.lock is not a regular file"),
        "{message}"
    );
    assert!(!store.root.join("elsewhere").exists(), "not followed");
    std::fs::remove_file(&gate).unwrap();
    store.snapshot().unwrap();
}

/// A read that must roll a journal forward but cannot, for a reason other
/// than write access, reports that reason as it is.
#[test]
fn a_read_reports_a_broken_journal_as_it_is() {
    let (_d, store) = project();
    let target = store.root.join("journal.json");
    std::fs::write(&target, "{}").unwrap();
    std::os::unix::fs::symlink(&target, store.root.join(".hyp/transaction.json")).unwrap();
    let message = format!("{:#}", store.snapshot().unwrap_err());
    assert!(message.contains("journal is a symlink"), "{message}");
    assert!(!message.contains("needs write access"), "{message}");
}

/// A notebook from before `gate.lock` with a `.hyp/` hyp cannot write: reads
/// work, and a write says what it needs, whichever lock file is missing.
#[test]
fn a_write_without_access_to_hyp_dir_says_so_whichever_lock_is_missing() {
    for missing in ["gate.lock", "write.lock", "apply.lock"] {
        let dir = TempDir::new().unwrap();
        let p = dir.path();
        ok(p, &["init"]);
        std::fs::remove_file(p.join(".hyp").join(missing)).unwrap();
        let hyp_dir = p.join(".hyp");
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hyp_dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        let blocked = permissions_apply(&hyp_dir);
        let list = hyp(p, &["list"]);
        let add = hyp(p, &["--json", "add", "Blocked"]);
        std::fs::set_permissions(&hyp_dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        if !blocked {
            return;
        }
        assert!(
            list.status.success(),
            "{missing}: {}",
            String::from_utf8_lossy(&list.stderr)
        );
        assert_eq!(add.status.code(), Some(1), "{missing}");
        let err: serde_json::Value = serde_json::from_slice(&add.stderr).unwrap();
        let message = err["error"].as_str().unwrap();
        assert!(
            // hyp names the canonical path, which differs when the temporary
            // directory is behind a symlink (on macOS, /var is /private/var).
            message.contains(&format!(
                "needs write access to {}",
                hyp_dir.canonicalize().unwrap().display()
            )) && message.contains("nothing was written"),
            "{missing}: {message}"
        );
        assert_eq!(err["kind"], "io", "{missing}");
    }
}

/// Makes `dir` and everything below it read-only until dropped, so that
/// the temporary directory can be removed even after a failed assertion.
struct ReadOnly<'a>(&'a Path);
impl<'a> ReadOnly<'a> {
    fn new(dir: &'a Path) -> Self {
        set_read_only(dir, true);
        Self(dir)
    }
}
impl Drop for ReadOnly<'_> {
    fn drop(&mut self) {
        set_read_only(self.0, false);
    }
}
/// Makes `dir` and everything below it read-only (or writable again).
fn set_read_only(dir: &Path, read_only: bool) {
    use std::os::unix::fs::PermissionsExt;
    for entry in walk(dir) {
        let meta = std::fs::symlink_metadata(&entry).unwrap();
        let mode = match (meta.is_dir(), read_only) {
            (true, true) => 0o555,
            (true, false) => 0o755,
            (false, true) => 0o444,
            (false, false) => 0o644,
        };
        std::fs::set_permissions(&entry, std::fs::Permissions::from_mode(mode)).unwrap();
    }
}
fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = vec![dir.to_path_buf()];
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else {
            out.push(path);
        }
    }
    out
}
/// Whether permissions stop this process from writing `dir` (not as root).
fn permissions_apply(dir: &Path) -> bool {
    let probe = dir.join("probe");
    let blocked = std::fs::write(&probe, "x").is_err();
    let _ = std::fs::remove_file(&probe);
    if !blocked {
        eprintln!("skipped: permissions do not stop this user from writing");
    }
    blocked
}
/// A notebook whose files cannot be written (a read-only mount, an archive
/// unpacked read-only, another user's files) is read like any other: with
/// the lock files present, under a shared lock opened read-only; without
/// `.hyp/` and the empty directories a copy drops, with no lock at all, as
/// nothing can write there. Writes fail with the cause, and nothing is
/// created.
#[test]
fn a_read_only_notebook_can_be_read() {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    ok(p, &["init", "--demo"]);
    ok(p, &["list"]); // creates the lock files
    let h = ok(p, &["--json", "list"]);
    let h: serde_json::Value = serde_json::from_str(&h).unwrap();
    let h = h[0]["record"]["id"].as_str().unwrap().to_string();
    let read_only = ReadOnly::new(p);
    if !permissions_apply(p) {
        return;
    }
    for args in [
        &["list"][..],
        &["status"],
        &["check"],
        &["show", &h],
        &["export", "--format", "json"],
    ] {
        ok(p, args);
    }
    let out = hyp(p, &["add", "Cannot be written"]);
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("ermission denied"), "{err}");
    drop(read_only);

    // As copied without .hyp/ and empty directories, then made read-only.
    std::fs::remove_dir_all(p.join(".hyp")).unwrap();
    std::fs::remove_dir(p.join("hyp/assets")).unwrap();
    std::fs::remove_dir(p.join("hyp/data")).unwrap();
    let _read_only = ReadOnly::new(p);
    for args in [&["list"][..], &["status"], &["check"], &["show", &h]] {
        ok(p, args);
    }
    assert!(!p.join(".hyp").exists(), "a read created nothing");
}

/// A write that crashed leaves its journal; where hyp cannot write, a read
/// cannot roll it forward and says so rather than reading half of it.
#[test]
fn a_read_only_notebook_with_an_unfinished_write_is_refused() {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    ok(p, &["init"]);
    let h = hypothesis("Pending");
    let journal = serde_json::json!({ format!("hypotheses/{}.md", h.id): encode(&h).unwrap() });
    std::fs::write(p.join(".hyp/transaction.json"), journal.to_string()).unwrap();
    let read_only = ReadOnly::new(p);
    if !permissions_apply(p) {
        return;
    }
    let out = hyp(p, &["list"]);
    drop(read_only);
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("did not finish"), "{err}");
    // With write access, any command rolls it forward.
    ok(p, &["list"]);
    assert!(!p.join(".hyp/transaction.json").exists());
}
