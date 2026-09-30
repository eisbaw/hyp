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

/// Everything a hypothesis's review token covers is visible in `show`: when
/// the token changes, so does the rest of the output.
#[test]
fn show_reveals_every_change_the_review_token_covers() {
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
        let mut shown = json(&["--json", "show", &h]);
        let token = shown["state"]["review_token"].as_str().unwrap().to_string();
        shown.as_object_mut().unwrap().remove("state");
        (shown, token)
    };
    let ids = |v: &serde_json::Value| -> Vec<String> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|e| e["record"]["id"].as_str().unwrap().to_string())
            .collect()
    };

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
    let (before, token) = show();
    let run = ok(p, &["run", &x, "Run 2", "--evidence", &e2])
        .trim()
        .to_string();
    let (after, token2) = show();
    assert_ne!(token, token2);
    assert!(
        before != after,
        "the run changed the token but not what show reveals"
    );
    assert!(ids(&after["runs"]).contains(&run), "{}", after["runs"]);
    assert!(
        ids(&after["evidence"]).contains(&e2),
        "{}",
        after["evidence"]
    );
    assert!(after["basis"][&run].is_string() && after["basis"][&e2].is_string());

    // A record merely linked to h (the competing hypothesis) is covered too.
    assert!(after["basis"][&other].is_string());
    ok(p, &["set", &other, "--title", "Bus contention, renamed"]);
    let (renamed, token3) = show();
    assert_ne!(token2, token3);
    assert_ne!(after["basis"][&other], renamed["basis"][&other]);
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
