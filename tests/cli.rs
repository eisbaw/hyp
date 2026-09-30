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

/// Runs `hyp edit id` with an editor that first makes another hyp write
/// (`concurrent`), deterministically between the command's read and its commit.
fn edit_during_concurrent_write(project: &Path, id: &str, concurrent: &str) -> Output {
    use std::os::unix::fs::PermissionsExt;
    let editor = project.join("editor.sh");
    std::fs::write(
        &editor,
        "#!/bin/sh\nset -e\n\"$HYP_BIN\" --project \"$HYP_PROJECT\" $HYP_CONCURRENT >/dev/null\n\
         sed -i 's/^title: .*/title: Edited in editor/' \"$1\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&editor, std::fs::Permissions::from_mode(0o755)).unwrap();
    hyp(project)
        .args(["edit", id])
        .env("VISUAL", &editor)
        .env("HYP_BIN", env!("CARGO_BIN_EXE_hyp"))
        .env("HYP_PROJECT", project)
        .env("HYP_CONCURRENT", concurrent)
        .output()
        .unwrap()
}
fn first(project: &Path, kind: &str) -> String {
    let rows: serde_json::Value =
        serde_json::from_str(&ok(project, &["--json", "list", "--kind", kind])).unwrap();
    rows[0]["record"]["id"].as_str().unwrap().to_string()
}

#[test]
fn unrelated_write_between_read_and_commit_does_not_fail_a_cli_write() {
    let project = demo();
    let h = first(project.path(), "hypothesis");
    let out = edit_during_concurrent_write(project.path(), &h, "add Unrelated");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let shown: serde_json::Value =
        serde_json::from_str(&ok(project.path(), &["--json", "show", &h])).unwrap();
    assert_eq!(shown["entry"]["record"]["title"], "Edited in editor");
    assert!(ok(project.path(), &["list"]).contains("Unrelated"));
    // A write to the same record in that window is still a conflict.
    let out =
        edit_during_concurrent_write(project.path(), &h, &format!("set {h} --title Concurrent"));
    assert_eq!(
        out.status.code(),
        Some(3),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn apply_rejects_an_assessment_that_does_not_state_what_it_saw() {
    let project = demo();
    let h = first(project.path(), "hypothesis");
    let change = serde_json::json!([{"op":"create","record":{"kind":"assessment","title":"Unstated","body":"Why","hypothesis":h,"judgment":"inconclusive"}}]);
    let mut child = hyp(project.path())
        .args(["--json", "apply"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    std::io::Write::write_all(
        &mut child.stdin.take().unwrap(),
        change.to_string().as_bytes(),
    )
    .unwrap();
    let out = child.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    let error: serde_json::Value = serde_json::from_str(&stderr).unwrap();
    let message = error["error"].as_str().unwrap();
    assert!(message.contains("requires `expected`"), "{message}");
}

/// `hyp assess` records what the agent reviewed, not what the command read a
/// moment before writing: counter-evidence or another agent's assessment in
/// between is a conflict.
#[test]
fn assess_is_based_on_the_state_the_agent_reviewed() {
    let project = demo();
    let p = project.path();
    let h = first(p, "hypothesis");
    let state = || -> serde_json::Value {
        serde_json::from_str::<serde_json::Value>(&ok(p, &["--json", "show", &h])).unwrap()["state"]
            .clone()
    };
    let token = || state()["review_token"].as_str().unwrap().to_string();
    let e = first(p, "evidence");
    let assess = |token: &str, status: &str| {
        run(
            p,
            &[
                "assess",
                &h,
                "--reviewed",
                token,
                "--status",
                status,
                "--evidence",
                &e,
                "--reason",
                "Reviewed",
            ],
        )
    };
    let listed: serde_json::Value =
        serde_json::from_str(&ok(p, &["--json", "list", "--kind", "hypothesis"])).unwrap();
    let row = listed
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["record"]["id"] == h.as_str())
        .unwrap();
    assert_eq!(row["state"]["review_token"].as_str().unwrap(), token());
    // Counter-evidence after the review.
    let reviewed = token();
    ok(
        p,
        &[
            "evidence",
            "add",
            &h,
            "Counter-evidence",
            "--source",
            "log",
            "--against",
        ],
    );
    let out = assess(&reviewed, "supported");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(3), "{stderr}");
    assert!(stderr.contains("changed since you reviewed it"), "{stderr}");
    // Another agent's assessment after the review: not part of the fingerprint.
    let reviewed = token();
    let fingerprint = state()["fingerprint"].clone();
    let out = assess(&token(), "weakened");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(state()["fingerprint"], fingerprint);
    let out = assess(&reviewed, "supported");
    assert_eq!(
        out.status.code(),
        Some(3),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(state()["judgment"], "weakened");
    // Malformed tokens are usage mistakes to fix, not races to retry.
    for bad in ["abc", &token()[..63], &"z".repeat(64)] {
        let out = assess(bad, "supported");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(1), "{bad}: {stderr}");
        assert!(stderr.contains("64-hex-digit"), "{stderr}");
    }
    let out = run(p, &["assess", &h, "--status", "supported", "--reason", "x"]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "a missing --reviewed is a usage error"
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("--reviewed"));
    let out = assess(&token(), "supported");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(state()["judgment"], "supported");
    assert_eq!(state()["needs_review"], false);
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

const SKILL_SOURCE: &str = include_str!("../agents/hyp/SKILL.md");
const SKILLS: [&str; 2] = [".claude/skills/hyp/SKILL.md", ".agents/skills/hyp/SKILL.md"];

fn read(path: impl AsRef<Path>) -> String {
    std::fs::read_to_string(path).unwrap()
}
/// Name and description from a SKILL.md's YAML front matter.
fn front_matter(text: &str) -> (String, String) {
    let yaml = text
        .strip_prefix("---\n")
        .unwrap()
        .split("\n---\n")
        .next()
        .unwrap();
    let map: std::collections::BTreeMap<String, String> = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(map.len(), 2, "only name and description: {map:?}");
    (map["name"].clone(), map["description"].clone())
}
fn mode(path: impl AsRef<Path>) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}
/// This process's umask, read without changing it.
fn umask() -> u32 {
    let status = read("/proc/self/status");
    let line = status
        .lines()
        .find_map(|l| l.strip_prefix("Umask:"))
        .unwrap();
    u32::from_str_radix(line.trim(), 8).unwrap()
}
fn fails(project: &Path, args: &[&str], message: &str) {
    let out = run(project, args);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "hyp {args:?}: {stderr}");
    assert!(stderr.contains(message), "hyp {args:?}: {stderr}");
}

#[test]
fn agents_print_writes_the_embedded_skill_without_a_project() {
    let dir = TempDir::new().unwrap();
    let out = ok(dir.path(), &["agents", "print"]);
    assert_eq!(out, SKILL_SOURCE);
    let (name, description) = front_matter(&out);
    assert_eq!(name, "hyp");
    assert!(description.len() <= 1024, "skill descriptions are capped");
    for trigger in [
        "debugging",
        "root-cause",
        "why something fails",
        "competing",
    ] {
        assert!(
            description.contains(trigger),
            "description lacks {trigger:?}"
        );
    }
    assert!(
        out.lines().count() < 200,
        "the skill is loaded into context"
    );
    assert!(!dir.path().join("hyp").exists());
}

/// Installs in a plain directory (no Git); repeating install or update
/// changes no byte and no modification time.
#[test]
fn init_with_agents_installs_the_skill_once() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    let out = ok(root, &["init", "--agents", "claude,codex"]);
    assert!(!root.join(".git").exists());
    for path in SKILLS {
        assert!(out.contains(&format!("installed {path}")), "{out}");
        let text = read(root.join(path));
        let marker = format!(
            "# hyp-managed: version={} sha256=",
            env!("CARGO_PKG_VERSION")
        );
        let lines: Vec<&str> = text.lines().filter(|l| !l.starts_with(&marker)).collect();
        assert_eq!(
            lines.len() + 1,
            text.lines().count(),
            "one marker line: {text}"
        );
        assert_eq!(lines.join("\n") + "\n", SKILL_SOURCE);
        assert_eq!(front_matter(&text), front_matter(SKILL_SOURCE));
        assert_eq!(
            mode(root.join(path)),
            0o644 & !umask(),
            "{path} readable by others"
        );
    }
    let before: Vec<_> = SKILLS
        .iter()
        .map(|p| {
            let path = root.join(p);
            (
                read(&path),
                std::fs::metadata(&path).unwrap().modified().unwrap(),
            )
        })
        .collect();
    for command in ["install", "update"] {
        let out = ok(root, &["agents", command]);
        assert_eq!(out.matches("unchanged").count(), 2, "{command}: {out}");
        for (p, (text, mtime)) in SKILLS.iter().zip(&before) {
            let path = root.join(p);
            assert_eq!(&read(&path), text, "{command} {p}");
            assert_eq!(
                &std::fs::metadata(&path).unwrap().modified().unwrap(),
                mtime
            );
        }
    }
    // A sync tool's CRLF conversion is not a local edit.
    let crlf = before[0].0.replace('\n', "\r\n");
    std::fs::write(root.join(SKILLS[0]), &crlf).unwrap();
    assert!(ok(root, &["agents", "install"]).contains("unchanged .claude"));
    // A bad agent list is an argument error, before anything is created.
    let bad = TempDir::new().unwrap();
    let out = run(bad.path(), &["init", "--agents", "none,claude"]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(!bad.path().join("hyp").exists());
    for args in [&["init"][..], &["init", "--agents", "none"]] {
        let dir = TempDir::new().unwrap();
        ok(dir.path(), args);
        assert!(!dir.path().join(".claude").exists(), "{args:?}");
        assert!(!dir.path().join(".agents").exists(), "{args:?}");
    }
}

/// hyp manages only its own skill files: other files are never touched, and
/// a skill file edited locally is overwritten or removed only with --force.
#[test]
fn agents_leave_other_files_alone_and_keep_local_edits_without_force() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    ok(root, &["init"]);
    let others = [
        ("CLAUDE.md", "# My instructions\n"),
        ("AGENTS.md", "# Codex instructions\n"),
        (".claude/settings.json", "{}\n"),
        (
            ".claude/skills/mine/SKILL.md",
            "---\nname: mine\ndescription: x\n---\n",
        ),
    ];
    for (path, text) in others {
        std::fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
        std::fs::write(root.join(path), text).unwrap();
    }
    let [claude, codex] = SKILLS.map(|p| root.join(p));
    ok(root, &["agents", "install", "--agents", "claude"]);
    let installed = read(&claude);
    std::fs::write(&claude, installed.clone() + "My own note.\n").unwrap();

    // A blocked plan writes nothing, not even for the unblocked agent.
    fails(
        root,
        &["agents", "install"],
        "modified after hyp installed it",
    );
    assert!(read(&claude).ends_with("My own note.\n"));
    assert!(!codex.exists());
    fails(root, &["agents", "update"], "--force");
    fails(root, &["agents", "remove"], "--force");
    assert!(read(&claude).ends_with("My own note.\n"));

    let out = ok(root, &["agents", "install", "--force"]);
    assert!(
        out.contains("replaced .claude/skills/hyp/SKILL.md"),
        "{out}"
    );
    assert!(
        out.contains("installed .agents/skills/hyp/SKILL.md"),
        "{out}"
    );
    assert_eq!(read(&claude), installed);

    std::fs::write(&claude, installed.clone() + "Edited again.\n").unwrap();
    ok(root, &["agents", "remove", "--agents", "claude", "--force"]);
    assert!(!root.join(".claude/skills/hyp").exists());
    ok(root, &["agents", "remove"]);
    assert!(
        !root.join(".agents").exists(),
        "empty directories hyp created"
    );

    // A file hyp did not install is never removed, even with --force.
    std::fs::create_dir_all(codex.parent().unwrap()).unwrap();
    std::fs::write(&codex, "not from hyp\n").unwrap();
    fails(
        root,
        &["agents", "remove", "--force"],
        "not installed by hyp",
    );
    let out = run(root, &["agents", "remove"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("--force"),
        "--force cannot help here: {stderr}"
    );
    fails(root, &["agents", "install"], "not installed by hyp");
    assert_eq!(read(&codex), "not from hyp\n");

    for (path, text) in others {
        assert_eq!(read(root.join(path)), text, "{path}");
    }
}

#[test]
fn agents_update_replaces_an_older_skill_but_not_a_newer_one() {
    use hyp::agents::{SKILL, VERSION, managed_file};
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    ok(root, &["init"]);
    let [claude, codex] = SKILLS.map(|p| root.join(p));
    let old = "---\nname: hyp\ndescription: Old wording.\n---\n\nOld body.\n";
    std::fs::create_dir_all(claude.parent().unwrap()).unwrap();
    std::fs::write(&claude, managed_file("0.0.1", old).unwrap()).unwrap();

    let out = ok(root, &["agents", "update"]);
    assert!(out.contains("updated .claude/skills/hyp/SKILL.md"), "{out}");
    assert!(
        out.contains("not_installed .agents/skills/hyp/SKILL.md"),
        "{out}"
    );
    assert_eq!(read(&claude), managed_file(VERSION, SKILL).unwrap());
    assert!(!codex.exists(), "update installs nothing new");

    // An older hyp must not silently downgrade a skill a newer one installed.
    let newer = managed_file("999.0.0", old).unwrap();
    std::fs::write(&claude, &newer).unwrap();
    fails(root, &["agents", "update"], "newer than this hyp");
    assert_eq!(read(&claude), newer);
    ok(root, &["agents", "update", "--force"]);
    assert_eq!(read(&claude), managed_file(VERSION, SKILL).unwrap());
}

/// Dotfile managers often make `.claude` or `.claude/skills` a symlink into
/// another tree. hyp does not write or delete through it unless told to.
#[test]
fn agents_do_not_reach_through_a_symlinked_skills_directory() {
    let dir = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    let root = dir.path();
    ok(root, &["init"]);
    std::fs::create_dir(root.join(".claude")).unwrap();
    std::os::unix::fs::symlink(outside.path(), root.join(".claude/skills")).unwrap();
    let [claude, codex] = SKILLS.map(|p| root.join(p));

    fails(root, &["agents", "install"], ".claude/skills is a symlink");
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
    assert!(!codex.exists(), "a blocked plan writes nothing");

    ok(root, &["agents", "install", "--force"]);
    assert!(outside.path().join("hyp/SKILL.md").is_file());
    fails(root, &["agents", "remove"], ".claude/skills is a symlink");
    assert!(claude.is_file() && codex.is_file());

    let out = ok(root, &["agents", "remove", "--force"]);
    assert!(out.contains("removed .claude/skills/hyp/SKILL.md"), "{out}");
    assert!(out.contains("removed .agents/skills/hyp/SKILL.md"), "{out}");
    assert!(!codex.exists() && !outside.path().join("hyp").exists());
    let link = std::fs::symlink_metadata(root.join(".claude/skills")).unwrap();
    assert!(link.file_type().is_symlink(), "the symlink itself stays");
    assert!(outside.path().is_dir());
}

/// An agent reviewing a hypothesis sees the observations behind it, not only
/// the IDs of the links that point at them.
#[test]
fn show_includes_the_evidence_a_hypothesis_links() {
    let project = demo();
    let json = |args: &[&str]| -> serde_json::Value {
        serde_json::from_str(&ok(project.path(), args)).unwrap()
    };
    let rows = json(&["--json", "list", "--kind", "hypothesis"]);
    let cache = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["record"]["title"].as_str().unwrap().contains("cache"))
        .unwrap();
    let shown = json(&["--json", "show", cache["record"]["id"].as_str().unwrap()]);
    let linked: Vec<&str> = shown["related"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["record"]["kind"] == "link")
        .filter_map(|r| r["record"]["from"].as_str())
        .filter(|from| from.starts_with("E-"))
        .collect();
    assert!(!linked.is_empty());
    let evidence = shown["evidence"].as_array().unwrap();
    for e in &linked {
        let entry = evidence
            .iter()
            .find(|x| x["record"]["id"] == *e)
            .unwrap_or_else(|| panic!("{e} missing from .evidence"));
        assert!(!entry["record"]["source"].as_str().unwrap().is_empty());
    }
    assert!(evidence.iter().all(|x| x["record"]["kind"] == "evidence"));
}

/// `.basis` is what the fingerprint hashes: the SHA-256 of its compact JSON
/// with sorted keys. Everything in it is visible in `show`; what is not in it
/// (another hypothesis's content) does not change the token.
#[test]
fn show_prints_the_basis_the_fingerprint_hashes() {
    let project = demo();
    let p = project.path();
    let json = |args: &[&str]| -> serde_json::Value { serde_json::from_str(&ok(p, args)).unwrap() };
    let rows = json(&["--json", "list", "--kind", "hypothesis"]);
    let id_of = |needle: &str| {
        rows.as_array()
            .unwrap()
            .iter()
            .find(|r| r["record"]["title"].as_str().unwrap().contains(needle))
            .unwrap()["record"]["id"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let (h, other) = (id_of("cache"), id_of("bus"));
    let x = first(p, "experiment");
    let show = || {
        let shown = json(&["--json", "show", &h]);
        let basis = serde_json::to_string(&shown["basis"]).unwrap();
        assert_eq!(
            hyp::model::hash(basis),
            shown["state"]["fingerprint"].as_str().unwrap(),
            "sha256 of .basis is .state.fingerprint"
        );
        shown
    };
    let ids = |v: &serde_json::Value| -> Vec<String> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|e| e["record"]["id"].as_str().unwrap().to_string())
            .collect()
    };
    let before = show();
    assert_eq!(
        before["basis"][&h]["scope"],
        before["entry"]["record"]["scope"]
    );

    // Evidence of another hypothesis, cited only by a run of h's experiment.
    let e2 = ok(
        p,
        &[
            "evidence",
            "add",
            &other,
            "Bus stalls at 8,000",
            "--source",
            "trace.txt",
        ],
    );
    let e2 = e2.lines().next().unwrap().to_string();
    assert_eq!(
        show()["state"],
        before["state"],
        "evidence of another hypothesis"
    );
    let run = ok(p, &["run", &x, "Run 2", "--evidence", &e2])
        .trim()
        .to_string();
    let after = show();
    assert_ne!(
        after["state"]["review_token"],
        before["state"]["review_token"]
    );
    assert!(ids(&after["runs"]).contains(&run), "{}", after["runs"]);
    assert!(
        ids(&after["evidence"]).contains(&e2),
        "{}",
        after["evidence"]
    );
    assert_eq!(after["basis"][&run]["outcome"], "observed");
    assert_eq!(after["basis"][&e2]["title"], "Bus stalls at 8,000");

    // The competing hypothesis counts only through the link.
    assert!(after["basis"][&other].is_null());
    ok(p, &["set", &other, "--title", "Bus contention, renamed"]);
    ok(p, &["set", &other, "--lifecycle", "closed"]);
    assert_eq!(show()["state"], after["state"]);
}
/// The 2026-09-30 dogfood: both agents assessed, then closed, and ended with
/// every hypothesis needing review. Closing is not new information.
#[test]
fn closing_after_assessing_leaves_nothing_needing_review() {
    let project = demo();
    let p = project.path();
    let rows: serde_json::Value =
        serde_json::from_str(&ok(p, &["--json", "list", "--kind", "hypothesis"])).unwrap();
    let hypotheses: Vec<String> = rows
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["record"]["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(hypotheses.len(), 2, "the demo's cache and bus hypotheses");
    let e = first(p, "evidence");
    let state = |h: &str| -> serde_json::Value {
        serde_json::from_str::<serde_json::Value>(&ok(p, &["--json", "show", h])).unwrap()["state"]
            .clone()
    };
    for h in &hypotheses {
        let token = state(h)["review_token"].as_str().unwrap().to_string();
        ok(
            p,
            &[
                "assess",
                h,
                "--reviewed",
                &token,
                "--status",
                "weakened",
                "--evidence",
                &e,
                "--reason",
                "Reviewed",
            ],
        );
    }
    for h in &hypotheses {
        ok(p, &["set", h, "--lifecycle", "closed", "--tags", "done"]);
    }
    for h in &hypotheses {
        assert_eq!(state(h)["needs_review"], false, "{h}");
    }
    assert_eq!(ok(p, &["list", "--needs-review"]).trim(), "");
}
/// Decision-0003 items 2 and 4, as an agent meets them: a judgment needs
/// evidence, and only evidence already linked to the hypothesis.
#[test]
fn assess_requires_linked_evidence_for_a_judgment() {
    let project = demo();
    let p = project.path();
    let h = first(p, "hypothesis");
    let token = || {
        serde_json::from_str::<serde_json::Value>(&ok(p, &["--json", "show", &h])).unwrap()["state"]
            ["review_token"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let assess = |status: &str, evidence: &[&str]| {
        let token = token();
        let mut args = vec!["assess", &h, "--reviewed", &token, "--status", status];
        for e in evidence {
            args.extend(["--evidence", e]);
        }
        args.extend(["--reason", "Reviewed"]);
        run(p, &args)
    };
    let out = assess("supported", &[]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.contains("a supported assessment must cite evidence"),
        "{stderr}"
    );
    let out = assess("untested", &[]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    // Evidence nobody linked to h: exit 1, and the message says how to link it.
    let unlinked = ok(p, &["add", "Unrelated claim"]).trim().to_string();
    let e = ok(
        p,
        &[
            "evidence",
            "add",
            &unlinked,
            "Seen elsewhere",
            "--source",
            "log",
        ],
    );
    let e = e.lines().next().unwrap().to_string();
    let out = assess("supported", &[&e]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains(&format!("hyp link {e} {h}")), "{stderr}");
    ok(
        p,
        &[
            "link",
            &e,
            &h,
            "--relation",
            "supports",
            "--reason",
            "Also here",
        ],
    );
    let out = assess("supported", &[&e]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
/// HYPO-0026: the review token covers the current assessments' content, not
/// only their IDs, so a hand edit (or a merge) of one is a conflict.
#[test]
fn a_hand_edited_current_assessment_invalidates_an_old_token() {
    let project = demo();
    let p = project.path();
    let rows: serde_json::Value =
        serde_json::from_str(&ok(p, &["--json", "list", "--kind", "hypothesis"])).unwrap();
    let assessed = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["state"]["assessment_ids"][0].is_string())
        .expect("the demo assesses one hypothesis");
    let h = assessed["record"]["id"].as_str().unwrap().to_string();
    let e = first(p, "evidence");
    let shown: serde_json::Value = serde_json::from_str(&ok(p, &["--json", "show", &h])).unwrap();
    let token = shown["state"]["review_token"].as_str().unwrap().to_string();
    let current = shown["state"]["assessment_ids"][0].as_str().unwrap();
    let path = p.join(format!("hyp/assessments/{current}.md"));
    let text = read(&path);
    let (header, _) = text.split_once("\n---\n").unwrap();
    std::fs::write(
        &path,
        format!("{header}\n---\nRewritten by hand: supported after all.\n"),
    )
    .unwrap();
    ok(p, &["check"]);
    let out = run(
        p,
        &[
            "assess",
            &h,
            "--reviewed",
            &token,
            "--status",
            "supported",
            "--evidence",
            &e,
            "--reason",
            "Based on the old judgment",
        ],
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(3), "{stderr}");
    assert!(stderr.contains("changed since you reviewed it"), "{stderr}");
}

/// An untestable reason is the stated alternative to a criterion, so
/// `check` does not warn about the missing criterion.
#[test]
fn check_accepts_an_untestable_reason_instead_of_a_criterion() {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    ok(p, &["init"]);
    let h = ok(p, &["add", "Cosmic rays flip the bit"])
        .trim()
        .to_string();
    let warning = format!("{h}: no active falsification criterion");
    assert!(ok(p, &["check"]).contains(&warning));
    ok(
        p,
        &[
            "set",
            &h,
            "--untestable-reason",
            "No radiation source available",
        ],
    );
    assert!(!ok(p, &["check"]).contains(&warning));
    ok(p, &["check", "--strict"]);
}

/// Runs `hyp [--json] apply` with `changes` on stdin.
fn apply(project: &Path, json: bool, extra: &[&str], changes: &str) -> Output {
    let mut command = hyp(project);
    if json {
        command.arg("--json");
    }
    let mut child = command
        .arg("apply")
        .args(extra)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    std::io::Write::write_all(&mut child.stdin.take().unwrap(), changes.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}
/// `{"written": [{"id", "kind", "revision"}], "revision"}` and nothing else
/// (HYPO-0028): agents parse it on every write, so it must not grow with the
/// project.
struct Written {
    id: String,
    kind: String,
    revision: Option<String>,
}
fn written(stdout: &str) -> (Vec<Written>, String) {
    let v: serde_json::Value = serde_json::from_str(stdout).unwrap();
    let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["revision", "written"], "{stdout}");
    let written = v["written"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| {
            let keys: Vec<&String> = x.as_object().unwrap().keys().collect();
            assert_eq!(keys, ["id", "kind", "revision"], "{x}");
            Written {
                id: x["id"].as_str().unwrap().to_string(),
                kind: x["kind"].as_str().unwrap().to_string(),
                revision: x["revision"].as_str().map(str::to_string),
            }
        })
        .collect();
    (written, v["revision"].as_str().unwrap().to_string())
}

#[test]
fn json_writes_print_what_they_wrote_and_the_new_revisions() {
    let project = demo();
    let p = project.path();
    let revision_of = |id: &str| -> String {
        serde_json::from_str::<serde_json::Value>(&ok(p, &["--json", "show", id])).unwrap()["entry"]
            ["revision"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let (w, _) = written(&ok(p, &["--json", "add", "Lean output"]));
    let [h] = &w[..] else { panic!("one record") };
    assert_eq!((h.id.len(), h.kind.as_str()), (38, "hypothesis"));
    assert_eq!(h.revision.as_deref(), Some(revision_of(&h.id).as_str()));
    let h = h.id.clone();
    let (w, _) = written(&ok(
        p,
        &["--json", "evidence", "add", &h, "Seen", "--source", "log"],
    ));
    let kinds: Vec<&str> = w.iter().map(|x| x.kind.as_str()).collect();
    assert_eq!(kinds, ["evidence", "link"]);
    let e = w[0].id.clone();
    let token = serde_json::from_str::<serde_json::Value>(&ok(p, &["--json", "show", &h])).unwrap()
        ["state"]["review_token"]
        .as_str()
        .unwrap()
        .to_string();
    let (w, _) = written(&ok(
        p,
        &[
            "--json",
            "assess",
            &h,
            "--reviewed",
            &token,
            "--status",
            "supported",
            "--evidence",
            &e,
            "--reason",
            "Seen",
        ],
    ));
    assert_eq!(w[0].kind, "assessment");
    // Attach prints the evidence ID, not the file.
    let file = p.join("capture.txt");
    std::fs::write(&file, "timeout").unwrap();
    assert_eq!(
        ok(p, &["evidence", "attach", &e[..10], file.to_str().unwrap()]),
        format!("{e}\n")
    );
    let (w, revision) = written(&ok(
        p,
        &["--json", "evidence", "attach", &e, file.to_str().unwrap()],
    ));
    assert_eq!(
        (w[0].id.as_str(), w[0].kind.as_str()),
        (e.as_str(), "evidence")
    );
    // apply: the ID the server assigned, and revisions the next apply can
    // state without a read in between.
    let create =
        r#"[{"op":"create","record":{"kind":"hypothesis","title":"Applied","lifecycle":"draft"}}]"#;
    let out = apply(p, true, &["--expected-revision", &revision], create);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let (w, _) = written(&String::from_utf8(out.stdout).unwrap());
    let created = &w[0];
    assert_eq!(created.kind, "hypothesis");
    let archive = serde_json::json!([{"op": "archive", "id": created.id, "archived": true,
        "expected_revision": created.revision}]);
    let out = apply(p, true, &[], &archive.to_string());
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let (w, _) = written(&String::from_utf8(out.stdout).unwrap());
    let delete = serde_json::json!([{"op": "delete", "id": created.id,
        "expected_revision": w[0].revision}]);
    let out = apply(p, true, &[], &delete.to_string());
    let (w, _) = written(&String::from_utf8(out.stdout).unwrap());
    assert_eq!(w[0].id, created.id);
    assert_eq!(w[0].revision, None, "a deleted record has no revision");
    let out = apply(p, false, &[], create);
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.starts_with("H-") && stdout.lines().count() == 1,
        "plain apply prints one ID per line: {stdout}"
    );
    // init: everything in the new project.
    let fresh = TempDir::new().unwrap();
    let (w, _) = written(&ok(fresh.path(), &["--json", "init", "--demo"]));
    let listed: serde_json::Value =
        serde_json::from_str(&ok(fresh.path(), &["--json", "list", "--all"])).unwrap();
    assert_eq!(w.len(), listed.as_array().unwrap().len());
}

#[test]
fn list_rejects_an_unknown_kind_or_status_instead_of_listing_nothing() {
    let project = demo();
    let p = project.path();
    for args in [
        &["list", "--kind", "hypotheses"][..],
        &["list", "--status", "open"],
    ] {
        let out = run(p, args);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {stderr}");
        assert!(stderr.contains("possible values"), "{stderr}");
    }
    // Filters the listed kind cannot have are usage errors too.
    for (args, hint) in [
        (&["list", "--status", "planned"][..], "--kind experiment"),
        (&["list", "--kind", "gap", "--needs-review"], "--all"),
    ] {
        let out = run(p, args);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {stderr}");
        assert!(stderr.contains(hint), "{stderr}");
    }
    assert_eq!(
        ok(
            p,
            &["list", "--kind", "experiment", "--status", "completed"]
        )
        .lines()
        .count(),
        1
    );
}

#[test]
fn list_shows_hypotheses_by_default_and_every_kind_with_all() {
    let project = demo();
    let p = project.path();
    let kinds = |text: &str| -> std::collections::BTreeSet<String> {
        text.lines()
            .map(|l| l.split_whitespace().nth(1).unwrap().to_string())
            .collect()
    };
    assert_eq!(kinds(&ok(p, &["list"])), ["hypothesis".to_string()].into());
    let all = kinds(&ok(p, &["list", "--all"]));
    for kind in ["hypothesis", "evidence", "link", "assessment", "run"] {
        assert!(all.contains(kind), "{kind} in {all:?}");
    }
    let rows: serde_json::Value = serde_json::from_str(&ok(p, &["--json", "list"])).unwrap();
    assert!(
        rows.as_array()
            .unwrap()
            .iter()
            .all(|r| r["record"]["kind"] == "hypothesis")
    );
}

#[test]
fn list_rows_show_judgment_lifecycle_and_needs_review() {
    let project = demo();
    let p = project.path();
    let row = |h: &str| -> String {
        ok(p, &["list"])
            .lines()
            .find(|l| l.starts_with(h))
            .unwrap()
            .to_string()
    };
    let assessed = ok(p, &["list"])
        .lines()
        .find(|l| l.contains("cache coherency"))
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_string();
    let columns: Vec<String> = row(&assessed)
        .split_whitespace()
        .take(4)
        .map(str::to_string)
        .collect();
    assert_eq!(columns[1..], ["hypothesis", "weakened", "draft"]);
    assert!(!row(&assessed).contains("needs-review"));
    ok(
        p,
        &[
            "evidence",
            "add",
            &assessed,
            "New",
            "--source",
            "log",
            "--against",
        ],
    );
    assert!(
        row(&assessed).contains("needs-review"),
        "{}",
        row(&assessed)
    );
    assert!(ok(p, &["list", "--needs-review"]).contains(&assessed));
    let rows: serde_json::Value = serde_json::from_str(&ok(p, &["--json", "list"])).unwrap();
    let json_row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["record"]["id"] == assessed.as_str())
        .unwrap();
    assert_eq!(json_row["state"]["judgment"], "weakened");
    assert_eq!(json_row["state"]["needs_review"], true);
    assert_eq!(json_row["record"]["lifecycle"], "draft");
}

#[test]
fn plain_show_summarises_a_hypothesis_for_people() {
    let project = demo();
    let p = project.path();
    let rows: serde_json::Value = serde_json::from_str(&ok(p, &["--json", "list"])).unwrap();
    let cache = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["record"]["title"].as_str().unwrap().contains("cache"))
        .unwrap();
    let h = cache["record"]["id"].as_str().unwrap();
    let text = ok(p, &["show", h]);
    let token = cache["state"]["review_token"].as_str().unwrap();
    assert!(text.contains(&format!("review token: {token}")), "{text}");
    for part in [
        "judgment: weakened · confidence 0.2",
        "Timeout reproduced at transfer 8,142",
        "source: demo:run-142 (illustrative data)",
        "Timeout reproduces with D-cache disabled",
        "Cache-disabled run #142: observed",
        "Does disabling cache change DMA timing? [open]",
        "Current assessment",
        // The observation itself, and relations as `hyp link` spells them.
        "one timeout occurred with cache disabled",
        "competes-with H-",
    ] {
        assert!(text.contains(part), "{part:?} in\n{text}");
    }
    for raw in [
        "untestable_reason",
        "archived: false",
        "tags: []",
        "created_at",
    ] {
        assert!(!text.contains(raw), "{raw:?} in\n{text}");
    }
    // Timestamps to the second.
    let created = text.lines().find(|l| l.starts_with("created ")).unwrap();
    assert!(
        created.ends_with('Z') && !created.contains('.'),
        "{created}"
    );
    // An archived criterion is still in the review basis: listed, marked.
    let f = first(p, "criterion");
    ok(p, &["archive", &f]);
    let text = ok(p, &["show", h]);
    assert!(
        text.contains("Timeout reproduces with D-cache disabled [archived]"),
        "{text}"
    );
}

/// The diagnostics `hyp --json check` reports for `path`.
fn diagnostics_of(p: &Path, path: &str) -> Vec<serde_json::Value> {
    let out = run(p, &["--json", "check"]);
    let all: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    all.as_array()
        .unwrap()
        .iter()
        .filter(|d| d["path"] == path)
        .cloned()
        .collect()
}
/// Runs the repair commands `hyp check` suggests for `path`, as argv arrays
/// in the project directory (no --project), with the binary under test.
fn apply_repair(p: &Path, path: &str) {
    let diagnostics = diagnostics_of(p, path);
    let commands = diagnostics
        .iter()
        .find_map(|d| d["repair"]["commands"].as_array())
        .unwrap_or_else(|| panic!("no repair for {path}: {diagnostics:?}"));
    assert!(!commands.is_empty(), "{diagnostics:?}");
    for command in commands {
        let argv: Vec<&str> = command
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a.as_str().unwrap())
            .collect();
        assert_eq!(argv[0], "hyp", "{command}");
        let out = Command::new(env!("CARGO_BIN_EXE_hyp"))
            .args(&argv[1..])
            .current_dir(p)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{command}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
/// The other half of a `depends_on` cycle, as a merge or sync brings it: a
/// link file written next to hyp's own. Returns the new link's ID.
fn drop_reverse_link(p: &Path, link: &str) -> String {
    let raw = std::fs::read_to_string(p.join(format!("hyp/links/{link}.md"))).unwrap();
    let mut record = hyp::store::decode(&raw).unwrap();
    record.id = format!("L-{}", uuid::Uuid::new_v4());
    if let hyp::model::Data::Link { from, to, .. } = &mut record.data {
        std::mem::swap(from, to);
    }
    std::fs::write(
        p.join(format!("hyp/links/{}.md", record.id)),
        hyp::store::encode(&record).unwrap(),
    )
    .unwrap();
    record.id
}

#[test]
fn merged_dependency_cycle_is_repaired_with_the_suggested_archive() {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    ok(p, &["init"]);
    let a = ok(p, &["add", "A"]).trim().to_string();
    let b = ok(p, &["add", "B"]).trim().to_string();
    let ours = ok(
        p,
        &["link", &a, &b, "--relation", "depends-on", "--reason", "r"],
    )
    .trim()
    .to_string();
    let theirs = drop_reverse_link(p, &ours);

    let out = run(p, &["check"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(1), "{stdout}");
    for id in [&ours, &theirs] {
        assert!(
            stdout.contains(&format!(
                "error hyp/links/{id}.md: depends-on cycle detected\n  \
                 note: Archiving any one link of the cycle breaks it.\n  \
                 repair: hyp archive {id}\n"
            )),
            "{stdout}"
        );
    }
    let cycle = &diagnostics_of(p, &format!("hyp/links/{theirs}.md"))[0];
    assert_eq!(cycle["code"], "cycle", "{cycle}");
    assert_eq!(cycle["blocks_writes"], false, "{cycle}");
    assert_eq!(
        cycle["repair"]["commands"],
        serde_json::json!([["hyp", "archive", theirs]])
    );

    // A write that leaves the errors as they are is not blocked by them.
    ok(p, &["add", "Unrelated work"]);
    // A write that adds an error is rejected: a third link closing the cycle
    // again, or a hypothesis investigating without a criterion.
    fails(
        p,
        &["link", &b, &a, "--relation", "depends-on", "--reason", "r"],
        "depends-on cycle detected",
    );
    fails(
        p,
        &["set", &a, "--lifecycle", "investigating"],
        "investigating requires a falsification criterion",
    );

    apply_repair(p, &format!("hyp/links/{theirs}.md"));
    ok(p, &["check"]);
    assert!(ok(p, &["--json", "list", "--kind", "link", "--archived"]).contains(&theirs));
}

#[test]
fn dangling_reference_after_a_sync_is_repaired_with_the_suggested_commands() {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    ok(p, &["init"]);
    let h = ok(p, &["add", "H"]).trim().to_string();
    let created = ok(p, &["evidence", "add", &h, "Obs", "--source", "log"]);
    let (e, link) = created.trim().split_once('\n').unwrap();
    // The sync deleted the evidence but kept the link to it.
    std::fs::remove_file(p.join(format!("hyp/evidence/{e}.md"))).unwrap();

    let out = run(p, &["check"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(1), "{stdout}");
    assert!(
        stdout.contains(&format!(
            "error hyp/links/{link}.md: references {e}, which does not exist\n  \
             note: Restore {e} from the source of the merge or sync"
        )),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!(
            "  repair: hyp archive {link}\n  repair: hyp delete {link}\n"
        )),
        "{stdout}"
    );
    let repair = &diagnostics_of(p, &format!("hyp/links/{link}.md"))[0]["repair"];
    assert_eq!(
        repair["commands"],
        serde_json::json!([["hyp", "archive", link], ["hyp", "delete", link]])
    );
    assert!(
        repair["note"].as_str().unwrap().contains("loses nothing"),
        "{repair}"
    );
    apply_repair(p, &format!("hyp/links/{link}.md"));
    ok(p, &["check"]);
    assert!(!p.join(format!("hyp/links/{link}.md")).exists());
}

#[test]
fn unreadable_files_block_every_write_including_repairs() {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    ok(p, &["init"]);
    let a = ok(p, &["add", "A"]).trim().to_string();
    let b = ok(p, &["add", "B"]).trim().to_string();
    let ours = ok(
        p,
        &["link", &a, &b, "--relation", "depends-on", "--reason", "r"],
    )
    .trim()
    .to_string();
    let theirs = drop_reverse_link(p, &ours);
    let broken = format!("hyp/hypotheses/H-{}.md", uuid::Uuid::new_v4());
    std::fs::write(p.join(&broken), "not front matter").unwrap();
    let json: serde_json::Value =
        serde_json::from_slice(&run(p, &["--json", "check"]).stdout).unwrap();
    let unreadable = json
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["path"] == broken.as_str())
        .unwrap();
    assert_eq!(unreadable["code"], "malformed");
    assert_eq!(unreadable["blocks_writes"], true);
    let blocked = format!("{broken}: file must start with YAML front matter");
    fails(p, &["add", "Unrelated work"], &blocked);
    fails(p, &["archive", &theirs], &blocked);
    std::fs::remove_file(p.join(&broken)).unwrap();
    ok(p, &["archive", &theirs]);
    ok(p, &["check"]);
}

/// QA repro (HYPO-0003 review): the link already has a known
/// dangling_reference; turning it into a `depends_on` link from evidence adds
/// a second, different violation to the same record, which must not hide
/// behind the first.
#[test]
fn a_new_error_cannot_hide_behind_a_known_one_on_the_same_record() {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    ok(p, &["init"]);
    let h = ok(p, &["add", "H"]).trim().to_string();
    let created = ok(p, &["evidence", "add", &h, "Obs", "--source", "log"]);
    let (e, link) = created.trim().split_once('\n').unwrap();
    std::fs::remove_file(p.join(format!("hyp/evidence/{e}.md"))).unwrap();
    let shown: serde_json::Value = serde_json::from_str(&ok(p, &["--json", "show", link])).unwrap();
    let mut record = shown["entry"]["record"].clone();
    record["relation"] = "depends_on".into();
    let update = serde_json::json!([{"op": "update", "record": record,
        "expected_revision": shown["entry"]["revision"]}]);
    let out = apply(p, false, &[], &update.to_string());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.contains("hypothesis relations require two hypotheses"),
        "{stderr}"
    );
}

fn codes(diagnostics: &[serde_json::Value]) -> Vec<&str> {
    diagnostics
        .iter()
        .map(|d| d["code"].as_str().unwrap())
        .collect()
}

/// A partial sync: the hypothesis file has not arrived yet. Its predictions
/// and criteria are not offered deletion (it cannot be undone without version
/// control); restoring is the repair.
#[test]
fn records_whose_owner_is_missing_are_never_offered_deletion() {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    ok(p, &["init"]);
    let h = ok(p, &["add", "H"]).trim().to_string();
    let prediction = ok(p, &["predict", &h, "P"]).trim().to_string();
    let criterion = ok(p, &["falsify-if", &h, "F"]).trim().to_string();
    std::fs::remove_file(p.join(format!("hyp/hypotheses/{h}.md"))).unwrap();
    let out = run(p, &["check"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(1), "{stdout}");
    assert!(!stdout.contains("delete"), "{stdout}");
    for (dir, id) in [("predictions", &prediction), ("criteria", &criterion)] {
        let d = &diagnostics_of(p, &format!("hyp/{dir}/{id}.md"))[0];
        assert_eq!(d["code"], "dangling_reference", "{d}");
        assert_eq!(d["repair"]["commands"], serde_json::json!([]), "{d}");
        assert!(
            d["repair"]["note"]
                .as_str()
                .unwrap()
                .starts_with(&format!("Restore {h} from the source")),
            "{d}"
        );
    }
}

/// Both halves of a `depends_on` cycle arrived, but one hypothesis did not:
/// each link breaks two rules, and `hyp check` reports both.
#[test]
fn a_record_breaking_two_rules_reports_both() {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    ok(p, &["init"]);
    let a = ok(p, &["add", "A"]).trim().to_string();
    let b = ok(p, &["add", "B"]).trim().to_string();
    let ours = ok(
        p,
        &["link", &a, &b, "--relation", "depends-on", "--reason", "r"],
    )
    .trim()
    .to_string();
    drop_reverse_link(p, &ours);
    std::fs::remove_file(p.join(format!("hyp/hypotheses/{b}.md"))).unwrap();
    assert_eq!(
        codes(&diagnostics_of(p, &format!("hyp/links/{ours}.md"))),
        ["dangling_reference", "cycle"]
    );
}

/// Replacing a record's known violation with one of another kind is a new
/// error: the dangling link, pointed at an existing hypothesis, closes a cycle.
#[test]
fn replacing_a_known_violation_with_another_is_rejected() {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    ok(p, &["init"]);
    let a = ok(p, &["add", "A"]).trim().to_string();
    let b = ok(p, &["add", "B"]).trim().to_string();
    let c = ok(p, &["add", "C"]).trim().to_string();
    ok(
        p,
        &["link", &b, &a, "--relation", "depends-on", "--reason", "r"],
    );
    let link = ok(
        p,
        &["link", &a, &c, "--relation", "depends-on", "--reason", "r"],
    )
    .trim()
    .to_string();
    std::fs::remove_file(p.join(format!("hyp/hypotheses/{c}.md"))).unwrap();
    let path = format!("hyp/links/{link}.md");
    assert_eq!(codes(&diagnostics_of(p, &path)), ["dangling_reference"]);
    let shown: serde_json::Value =
        serde_json::from_str(&ok(p, &["--json", "show", &link])).unwrap();
    let mut record = shown["entry"]["record"].clone();
    record["to"] = b.clone().into();
    let update = serde_json::json!([{"op": "update", "record": record,
        "expected_revision": shown["entry"]["revision"]}]);
    let out = apply(p, false, &[], &update.to_string());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("depends-on cycle detected"), "{stderr}");
    assert_eq!(codes(&diagnostics_of(p, &path)), ["dangling_reference"]);
}

/// Whether a stored short ID is an error does not depend on which records
/// exist (it used to flip between dangling and inconsistent): it is `invalid`,
/// reported once, and blocks writes.
#[test]
fn a_stored_short_reference_is_invalid_regardless_of_other_records() {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    ok(p, &["init"]);
    let h = ok(p, &["add", "H"]).trim().to_string();
    let prediction = ok(p, &["predict", &h, "P"]).trim().to_string();
    let file = p.join(format!("hyp/predictions/{prediction}.md"));
    let raw = std::fs::read_to_string(&file).unwrap();
    // A prefix of an existing ID, and one that matches nothing.
    for short in [&h[..6], "H-zz"] {
        std::fs::write(&file, raw.replace(&h, short)).unwrap();
        let d = diagnostics_of(p, &format!("hyp/predictions/{prediction}.md"));
        assert_eq!(codes(&d), ["invalid"], "{short}: {d:?}");
        assert_eq!(d[0]["message"], "stored references must use full IDs");
        assert_eq!(d[0]["blocks_writes"], true);
    }
    fails(p, &["add", "Other"], "1 error blocks writes; run hyp check");
    // An error that does not block is counted separately.
    std::fs::write(&file, &raw).unwrap();
    let late = ok(p, &["add", "Late"]).trim().to_string();
    ok(p, &["predict", &late, "Q"]);
    std::fs::remove_file(p.join(format!("hyp/hypotheses/{late}.md"))).unwrap();
    std::fs::write(&file, raw.replace(&h, "H-zz")).unwrap();
    fails(
        p,
        &["add", "Other"],
        "1 error blocks writes (1 more does not); run hyp check",
    );
}

/// Runs hyp with `input` on stdin.
fn with_stdin(project: &Path, args: &[&str], input: &str) -> Output {
    use std::io::Write;
    let mut child = hyp(project)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}
fn record(p: &Path, id: &str) -> serde_json::Value {
    let shown: serde_json::Value = serde_json::from_str(&ok(p, &["--json", "show", id])).unwrap();
    shown["entry"]["record"].clone()
}

#[test]
fn a_title_is_one_line_and_stdin_lines_after_it_go_to_the_body() {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    ok(p, &["init"]);
    let out = with_stdin(
        p,
        &["add", "-", "--body", "Given body"],
        "Cron starts two backups\nsecond line\nthird line\n",
    );
    assert!(out.status.success(), "{out:?}");
    let h = String::from_utf8(out.stdout).unwrap().trim().to_string();
    let r = record(p, &h);
    assert_eq!(r["title"], "Cron starts two backups");
    assert_eq!(r["body"], "Given body\n\nsecond line\nthird line");
    // Without a body the rest is the body; the list stays one row per record.
    let out = with_stdin(p, &["predict", &h, "-"], "Two runs in the log\nat 02:00\n");
    assert!(out.status.success(), "{out:?}");
    let prediction = String::from_utf8(out.stdout).unwrap().trim().to_string();
    let r = record(p, &prediction);
    assert_eq!(
        (&r["title"], &r["body"]),
        (&"Two runs in the log".into(), &"at 02:00".into())
    );
    assert_eq!(ok(p, &["list", "--all"]).lines().count(), 2);
    // A given title with a newline is rejected, not stored.
    fails(p, &["add", "a\nb"], "title must be a single line");
    // A hand-edited file with one is reported by check.
    let file = p.join(format!("hyp/hypotheses/{h}.md"));
    let raw = std::fs::read_to_string(&file).unwrap();
    std::fs::write(
        &file,
        raw.replace("title: Cron starts two backups", "title: \"Cron\\nstarts\""),
    )
    .unwrap();
    let d = diagnostics_of(p, &format!("hyp/hypotheses/{h}.md"));
    assert_eq!(codes(&d), ["invalid"], "{d:?}");
    assert_eq!(
        d[0]["message"],
        "title must be a single line; put the rest in the body"
    );
    assert_eq!(d[0]["blocks_writes"], true);
    let note = d[0]["repair"]["note"].as_str().unwrap();
    assert!(
        note.contains("Edit the file by hand") && note.contains("hyp check"),
        "{note}"
    );
}

#[test]
fn id_errors_explain_no_match_list_candidates_and_name_the_wrong_kind() {
    let project = demo();
    let p = project.path();
    // No kind letter: the hint explains them.
    fails(p, &["show", "bd43"], "no record with ID (prefix) \"bd43\"");
    fails(p, &["show", "bd43"], "H- hypothesis, P- prediction");
    let out = run(p, &["show", "H-zz"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("no record with ID (prefix) \"H-zz\""),
        "{stderr}"
    );
    assert!(!stderr.contains("kind letter"), "{stderr}");
    // Ambiguous: every candidate with its kind and title.
    let out = run(p, &["show", "H-"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr.contains("matches 2 records"), "{stderr}");
    let rows: serde_json::Value = serde_json::from_str(&ok(p, &["--json", "list"])).unwrap();
    for row in rows.as_array().unwrap() {
        let line = format!(
            "{}  hypothesis  {}",
            row["record"]["id"].as_str().unwrap(),
            row["record"]["title"].as_str().unwrap()
        );
        assert!(stderr.contains(&line), "{line:?} in {stderr}");
    }
    // Wrong kind: the argument, what it expects and what it got.
    let e = first(p, "evidence");
    fails(
        p,
        &["predict", &e[..8], "x"],
        &format!("argument <HYPOTHESIS>: expected a hypothesis, got evidence {e}"),
    );
    fails(
        p,
        &[
            "run",
            &first(p, "experiment"),
            "r",
            "--evidence",
            &first(p, "gap"),
        ],
        "argument --evidence: expected evidence, got gap G-",
    );
}

#[test]
fn deleting_a_referenced_record_lists_what_refers_to_it() {
    let project = demo();
    let p = project.path();
    let rows: serde_json::Value = serde_json::from_str(&ok(p, &["--json", "list"])).unwrap();
    let bus = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["record"]["title"].as_str().unwrap().contains("bus"))
        .unwrap()["record"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let links: serde_json::Value =
        serde_json::from_str(&ok(p, &["--json", "list", "--kind", "link"])).unwrap();
    let referrers: Vec<String> = links
        .as_array()
        .unwrap()
        .iter()
        .filter(|l| l["record"]["to"] == bus.as_str())
        .map(|l| {
            format!(
                "{}  link        {}",
                l["record"]["id"].as_str().unwrap(),
                l["record"]["title"].as_str().unwrap()
            )
        })
        .collect();
    assert_eq!(referrers.len(), 2);
    for archived in [false, true] {
        let out = run(p, &["delete", &bus]);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(1), "{stderr}");
        assert!(stderr.contains(&format!("cannot delete {bus}: these records refer to it")));
        for line in &referrers {
            assert!(stderr.contains(line), "{line:?} in {stderr}");
        }
        let archive = format!("hyp archive {bus}");
        assert_eq!(stderr.contains(&archive), !archived, "{stderr}");
        assert_eq!(stderr.contains("it is already out of lists"), archived);
        ok(p, &["archive", &bus]);
    }
    // Unreferenced but not archived: the archive command to run first.
    let h = ok(p, &["add", "Unreferenced"]).trim().to_string();
    fails(
        p,
        &["delete", &h],
        &format!("archive {h} before deleting it: hyp archive {h}"),
    );
}

#[test]
fn plain_output_spells_relations_as_the_cli_takes_them() {
    let project = demo();
    let p = project.path();
    let competes: serde_json::Value =
        serde_json::from_str(&ok(p, &["--json", "list", "--kind", "link"])).unwrap();
    let competes = competes
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["record"]["relation"] == "competes_with")
        .unwrap()["record"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let hs: serde_json::Value = serde_json::from_str(&ok(p, &["--json", "list"])).unwrap();
    let (a, b) = (
        hs[0]["record"]["id"].as_str().unwrap(),
        hs[1]["record"]["id"].as_str().unwrap(),
    );
    ok(
        p,
        &[
            "link",
            &a[..8],
            &b[..8],
            "--relation",
            "depends-on",
            "--reason",
            "r",
        ],
    );
    for (args, spelled) in [
        (vec!["show", competes.as_str()], "relation: competes-with"),
        (vec!["graph"], "-->|competes-with|"),
        (vec!["export"], "relation: competes-with"),
        (vec!["list", "--all"], " depends-on "),
    ] {
        let text = ok(p, &args);
        assert!(text.contains(spelled), "{spelled:?} in {args:?}:\n{text}");
        assert!(
            !text.contains("competes_with") && !text.contains("depends_on"),
            "{args:?}:\n{text}"
        );
    }
    // JSON and files keep the serialized form.
    assert_eq!(record(p, &competes)["relation"], "competes_with");
}

/// Agents read --help: every subcommand says in one line what it does, and
/// every argument what it takes.
#[test]
fn every_subcommand_and_argument_has_help() {
    use clap::CommandFactory;
    fn walk(command: &clap::Command, path: &str, missing: &mut Vec<String>) {
        for arg in command.get_arguments() {
            let id = arg.get_id().as_str();
            if !matches!(id, "help" | "version") && arg.get_help().is_none() {
                missing.push(format!("{path} {id}"));
            }
        }
        for sub in command.get_subcommands() {
            let path = format!("{path} {}", sub.get_name());
            if sub.get_name() == "help" {
                continue;
            }
            let about = sub.get_about().map(ToString::to_string).unwrap_or_default();
            if about.is_empty() || about.contains('\n') || about.len() > 70 {
                missing.push(format!("{path}: about {about:?}"));
            }
            walk(sub, &path, missing);
        }
    }
    let mut missing = vec![];
    walk(&hyp::cli::Cli::command(), "hyp", &mut missing);
    assert!(missing.is_empty(), "{missing:#?}");
}

#[test]
fn apply_help_shows_every_change_and_an_assessment_statement() {
    let dir = TempDir::new().unwrap();
    // Hermetic: help must not depend on the caller's terminal width.
    let out = hyp(dir.path())
        .args(["apply", "--help"])
        .env_remove("COLUMNS")
        .output()
        .unwrap();
    assert!(out.status.success());
    let help = String::from_utf8(out.stdout).unwrap();
    for part in [
        r#"{"op": "create""#,
        r#"{"op": "update""#,
        r#"{"op": "archive""#,
        r#""kind": "assessment""#,
        r#""expected": {"hypotheses": {"H-…": {"review_token": "…"}}}"#,
    ] {
        assert!(help.contains(part), "{part:?} in\n{help}");
    }
}

#[test]
fn check_counts_in_the_singular_and_init_says_what_to_do_next() {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    let init = ok(p, &["init"]);
    assert!(
        init.lines().any(|l| l.starts_with("Next: hyp add")),
        "{init}"
    );
    ok(p, &["add", "Only"]);
    assert!(
        ok(p, &["check"]).ends_with("Checked 1 object\n"),
        "singular"
    );
}
