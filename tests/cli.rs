//! Tests that run the real `hyp` binary: exit codes, stdout handling, and
//! projects that arrive by copy or clone rather than `hyp init`.
use std::{
    io::{BufRead, BufReader},
    path::Path,
    process::{Command, Output, Stdio},
    sync::mpsc,
    time::Duration,
};
use tempfile::TempDir;

fn hyp(project: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_hyp"));
    command.arg("--project").arg(project).stdin(Stdio::null());
    command
}
fn run(project: &Path, args: &[&str]) -> Output {
    hyp(project).args(args).output().unwrap()
}
fn ok(project: &Path, args: &[&str]) -> String {
    let out = run(project, args);
    assert!(
        out.status.success(),
        "hyp {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}
fn demo() -> TempDir {
    let dir = TempDir::new().unwrap();
    ok(dir.path(), &["init", "--demo"]);
    dir
}
/// Copy regular files only, as sync and archive tools with file filters do:
/// empty directories and `.hyp/` do not arrive.
fn copy_files(from: &Path, to: &Path) {
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_files(&entry.path(), &target);
        } else {
            std::fs::create_dir_all(to).unwrap();
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}
/// The operations an agent runs first in a project it did not create.
fn list_check_add(project: &Path) {
    assert!(ok(project, &["list"]).contains("DMA timeout"));
    ok(project, &["check"]);
    let id = ok(project, &["add", "Copied project accepts writes"]);
    assert!(ok(project, &["list"]).contains(id.trim()));
}

#[test]
fn project_copied_without_empty_directories_opens_and_accepts_writes() {
    let source = demo();
    let copy = TempDir::new().unwrap();
    copy_files(&source.path().join("hyp"), &copy.path().join("hyp"));
    assert!(!copy.path().join("hyp/assets").exists());
    assert!(!copy.path().join(".hyp").exists());
    list_check_add(copy.path());
    for dir in hyp::store::DIRECTORIES.iter().copied().chain(["assets"]) {
        assert!(
            copy.path().join("hyp").join(dir).is_dir(),
            "{dir} recreated"
        );
    }
    assert_eq!(
        std::fs::read_to_string(copy.path().join(".hyp/.gitignore")).unwrap(),
        "*\n"
    );
}

fn git(dir: &Path, args: &[&str]) -> String {
    // Isolate from the user's and system's Git configuration (signing, hooks, identity).
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "hyp test")
        .env("GIT_AUTHOR_EMAIL", "hyp-test@example.invalid")
        .env("GIT_COMMITTER_NAME", "hyp test")
        .env("GIT_COMMITTER_EMAIL", "hyp-test@example.invalid")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

/// Optional: Git is one way to move a project, not a requirement (decision-0001).
#[test]
fn git_clone_of_project_opens_and_status_stays_clean() {
    if Command::new("git").arg("--version").output().is_err() {
        eprintln!("skipping git_clone_of_project_opens_and_status_stays_clean: git not found");
        return;
    }
    let source = demo();
    git(source.path(), &["init", "-q"]);
    // Only hyp/ is shared; the clone gets no .hyp/ and hyp must recreate it ignored.
    git(source.path(), &["add", "hyp"]);
    git(source.path(), &["commit", "-q", "-m", "demo"]);
    let parent = TempDir::new().unwrap();
    let clone = parent.path().join("clone");
    git(
        parent.path(),
        &["clone", "-q", source.path().to_str().unwrap(), "clone"],
    );
    assert!(!clone.join("hyp/assets").exists());
    assert!(ok(&clone, &["list"]).contains("DMA timeout"));
    ok(&clone, &["check"]);
    assert_eq!(git(&clone, &["status", "--porcelain"]), "");
    let id = ok(&clone, &["add", "Cloned project accepts writes"]);
    assert_eq!(
        git(&clone, &["status", "--porcelain"]),
        format!("?? hyp/hypotheses/{}.md\n", id.trim())
    );
}

#[test]
fn stale_cli_write_exits_with_code_3_and_a_conflict_message() {
    let project = demo();
    let change = r#"[{"op":"create","record":{"kind":"hypothesis","title":"Stale","scope":"","assumptions":"","lifecycle":"draft","untestable_reason":""}}]"#;
    for json in [false, true] {
        let mut command = hyp(project.path());
        if json {
            command.arg("--json");
        }
        let mut child = command
            .args(["apply", "--expected-revision", "stale-revision"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        std::io::Write::write_all(&mut child.stdin.take().unwrap(), change.as_bytes()).unwrap();
        let out = child.wait_with_output().unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(3), "{stderr}");
        if json {
            let error: serde_json::Value = serde_json::from_str(&stderr).unwrap();
            assert!(error["error"].as_str().unwrap().starts_with("conflict:"));
        } else {
            assert!(stderr.starts_with("hyp: conflict:"), "{stderr}");
        }
    }
    let out = run(project.path(), &["show", "no-such-id"]);
    assert_eq!(out.status.code(), Some(1), "ordinary errors exit 1");
}

#[test]
fn closed_stdout_ends_quietly_without_panic() {
    use std::os::unix::{net::UnixStream, process::ExitStatusExt};
    let project = demo();
    // A socket whose peer is already closed: the first write fails with EPIPE,
    // deterministically, like `hyp list | head -c0`.
    let (stdout, reader) = UnixStream::pair().unwrap();
    drop(reader);
    for args in [&["list"][..], &["export", "--format", "json"]] {
        let out = hyp(project.path())
            .args(args)
            .stdout(std::os::fd::OwnedFd::from(stdout.try_clone().unwrap()))
            .stderr(Stdio::piped())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(!stderr.contains("panicked"), "{args:?}: {stderr}");
        assert_ne!(out.status.code(), Some(101), "{args:?}: {stderr}");
        assert_eq!(out.status.signal(), Some(13), "{args:?}: SIGPIPE");
    }
}

/// Kills the server when the test ends, pass or fail.
struct Server(std::process::Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn wait_for_line(lines: &mpsc::Receiver<String>, what: &str, pred: impl Fn(&str) -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    let mut seen = Vec::new();
    while let Some(left) = deadline.checked_duration_since(std::time::Instant::now()) {
        match lines.recv_timeout(left) {
            Ok(line) if pred(&line) => return,
            Ok(line) => seen.push(line),
            Err(_) => break,
        }
    }
    panic!("timed out waiting for {what}; saw {seen:?}");
}
fn lines(stream: impl std::io::Read + Send + 'static) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    rx
}

#[test]
fn web_logs_why_the_project_is_unavailable_and_when_it_recovers() {
    let project = demo();
    let mut child = hyp(project.path())
        .args(["web", "--port", "0"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = lines(child.stdout.take().unwrap());
    let stderr = lines(child.stderr.take().unwrap());
    let _server = Server(child);
    wait_for_line(&stdout, "startup banner", |l| l.contains("hyp WebUI:"));
    let gaps = project.path().join("hyp/gaps");
    std::fs::remove_dir_all(&gaps).unwrap();
    std::fs::write(&gaps, "not a directory").unwrap();
    wait_for_line(&stderr, "logged cause", |l| {
        l.contains("cannot read project") && l.contains("hyp/gaps")
    });
    // The server recreates the missing directory itself.
    std::fs::remove_file(&gaps).unwrap();
    wait_for_line(&stderr, "logged recovery", |l| l.contains("readable again"));
    assert!(gaps.is_dir());
}
