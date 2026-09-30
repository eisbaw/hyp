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
        .env("TMPDIR", project)
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
    // A write to the same record in that window is still a conflict. (A new
    // title first, so that the editor's edit differs from the record.)
    ok(project.path(), &["set", &h, "--title", "Before"]);
    let out =
        edit_during_concurrent_write(project.path(), &h, &format!("set {h} --title Concurrent"));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(3), "{stderr}");
    // The edit is not lost: the conflict names a copy of it.
    assert!(stderr.starts_with("hyp: conflict:"), "{stderr}");
    let kept = stderr
        .trim_end()
        .split("your text is kept in ")
        .nth(1)
        .unwrap_or_else(|| panic!("no kept path in {stderr}"));
    assert!(read(kept).contains("title: Edited in editor"));
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
    for bad in [
        "abc",
        &token()[..11],
        &"z".repeat(64),
        &format!("{}0", token()),
    ] {
        let out = assess(bad, "supported");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(1), "{bad}: {stderr}");
        assert!(stderr.contains("first 12 or more hex digits"), "{stderr}");
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

/// Drift guard for the method the skill teaches: tests/fixtures/skill_flow.sh
/// runs against this binary, in a fresh project, with `hyp` on PATH. Its
/// shell variables ($H1, $E1, $TOKEN, ...) capture the IDs and the review
/// token that earlier commands print; the skill itself teaches reading them
/// (agent sandboxes block `$VAR`), so the script lives outside it. Under
/// `set -euo pipefail`, a renamed flag or value (exit 2), a changed output
/// (an empty capture a later command rejects) or a rejected write fails it.
#[test]
fn skill_flow_runs_against_this_binary_and_ends_assessed_and_closed() {
    let project = TempDir::new().unwrap();
    ok(project.path(), &["init"]);
    let bin = TempDir::new().unwrap();
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_hyp"), bin.path().join("hyp")).unwrap();
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let path = std::env::join_paths(
        std::iter::once(bin.path().to_path_buf()).chain(std::env::split_paths(&inherited)),
    )
    .unwrap();
    let script = include_str!("fixtures/skill_flow.sh");
    let out = Command::new("bash")
        .args(["-euo", "pipefail", "-c", script])
        .current_dir(project.path())
        .env("PATH", path)
        .stdin(Stdio::null())
        .output()
        .expect("bash is needed to run the skill flow");
    assert!(
        out.status.success(),
        "the skill flow failed ({}):\n--- stderr\n{}\n--- stdout\n{}\n--- script\n{script}",
        out.status,
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout),
    );
    let list = |kind: &str| -> Vec<serde_json::Value> {
        let text = ok(project.path(), &["--json", "list", "--kind", kind]);
        serde_json::from_str(&text).unwrap()
    };
    // H1 closed and supported, its competitor H2 still a draft but weakened;
    // each assessed once, neither needing review again.
    let mut hypotheses: Vec<_> = list("hypothesis")
        .iter()
        .map(|r| {
            assert_eq!(r["state"]["needs_review"], false, "{r}");
            let assessments = r["state"]["assessment_ids"].as_array().unwrap().len();
            (
                r["record"]["lifecycle"].as_str().unwrap().to_string(),
                r["state"]["judgment"].as_str().unwrap().to_string(),
                assessments,
            )
        })
        .collect();
    hypotheses.sort();
    assert_eq!(
        hypotheses,
        [
            ("closed".into(), "supported".into(), 1),
            ("draft".into(), "weakened".into(), 1)
        ]
    );
    assert_eq!(ok(project.path(), &["list", "--needs-review"]), "");
    let experiments = list("experiment");
    assert_eq!(experiments.len(), 1);
    assert_eq!(experiments[0]["record"]["status"], "completed");
    let gaps = list("gap");
    assert_eq!(
        gaps[0]["record"]["resolved_by"],
        list("evidence")[0]["record"]["id"]
            .as_str()
            .map(|e| serde_json::json!([e]))
            .unwrap()
    );
    ok(project.path(), &["check"]);
}
/// The skill teaches commands an agent sandbox accepts: no shell expansion
/// (`$VAR`, `$(...)`) in any of its command blocks (HYPO-0077).
#[test]
fn skill_commands_use_no_shell_expansion() {
    for block in SKILL_SOURCE.split("```bash\n").skip(1) {
        let block = block.split("\n```").next().unwrap();
        assert!(
            !block.contains('$'),
            "shell expansion in the skill:\n{block}"
        );
    }
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
/// HYPO-0087: an ID in `hyp apply` that names no record is kind not_found
/// (exit 1), not a conflict (exit 3) that an agent would re-read and retry
/// forever.
#[test]
fn apply_with_a_mistyped_id_is_not_found_not_a_conflict() {
    let project = demo();
    let p = project.path();
    let h = first(p, "hypothesis");
    let revision = serde_json::from_str::<serde_json::Value>(&ok(p, &["--json", "show", &h]))
        .unwrap()["entry"]["revision"]
        .as_str()
        .unwrap()
        .to_string();
    // The last hex digit changed: the shape of a real ID, but no record.
    let last = h.chars().last().unwrap();
    let typo = format!(
        "{}{}",
        &h[..h.len() - 1],
        if last == '0' { '1' } else { '0' }
    );
    let batch = serde_json::json!([{"op": "patch", "id": typo, "expected_revision": revision,
                                    "set": {"title": "Retitled"}}]);
    let out = apply(p, true, &[], &batch.to_string());
    let stderr = stderr_of(&out);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    let error: serde_json::Value = serde_json::from_str(&stderr).unwrap();
    assert_eq!(error["kind"], "not_found", "{stderr}");
    assert!(
        error["error"]
            .as_str()
            .unwrap()
            .contains(&format!("{typo} does not exist")),
        "{stderr}"
    );
    assert_eq!(error["ids"], serde_json::json!([typo]), "{stderr}");
}
/// The demo hypothesis with a criterion, with its criterion, its linked
/// evidence and its review token, as `hyp --json status` and `show` give them.
fn demo_claim(p: &Path) -> (String, String, String, String) {
    let status: serde_json::Value = serde_json::from_str(&ok(p, &["--json", "status"])).unwrap();
    let row = status["hypotheses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| !h["criteria"].as_array().unwrap().is_empty())
        .unwrap();
    let text = |v: &serde_json::Value| v.as_str().unwrap().to_string();
    let h = text(&row["id"]);
    let shown: serde_json::Value = serde_json::from_str(&ok(p, &["--json", "show", &h])).unwrap();
    (
        h,
        text(&row["criteria"][0]),
        text(&row["linked_evidence"][0]),
        text(&shown["state"]["review_token"]),
    )
}
/// A falsified assessment of `h` by `criterion`, citing `evidence`, stating `token`.
fn falsified(h: &str, criterion: &str, evidence: &[&str], token: &str) -> String {
    serde_json::json!([{"op": "create",
        "record": {"kind": "assessment", "title": "Falsified", "body": "Why", "hypothesis": h,
                   "judgment": "falsified", "evidence": evidence, "criterion": criterion},
        "expected": {"hypotheses": {h: {"review_token": token}}}}])
    .to_string()
}
/// `ID` with its last hex digit changed: the shape of a real ID, but no record.
fn mistyped(id: &str) -> String {
    let last = if id.ends_with('0') { '1' } else { '0' };
    format!("{}{last}", &id[..id.len() - 1])
}
/// HYPO-0087: a create whose reference fields name an ID that matches no
/// record, before or within the batch, is not_found with that ID in `ids`,
/// as the CLI answers for the same ID; a falsified assessment citing it is
/// told so, not that its evidence does not meet the criterion.
#[test]
fn a_create_naming_an_id_that_matches_nothing_is_not_found_with_its_ids() {
    let project = demo();
    let p = project.path();
    let (h, f, e, token) = demo_claim(p);
    let typo = mistyped(&e);
    let not_found = |out: Output, id: &str| {
        let stderr = stderr_of(&out);
        assert_eq!(out.status.code(), Some(1), "{stderr}");
        let error: serde_json::Value = serde_json::from_str(&stderr).unwrap();
        assert_eq!(error["kind"], "not_found", "{stderr}");
        assert_eq!(error["ids"], serde_json::json!([id]), "{stderr}");
        let message = error["error"].as_str().unwrap();
        assert!(message.contains(id), "{message}");
        assert!(!message.contains("meets its criterion"), "{message}");
    };
    not_found(
        apply(p, true, &[], &falsified(&h, &f, &[&e, &typo], &token)),
        &typo,
    );
    let link = serde_json::json!([{"op": "create",
        "record": {"kind": "link", "title": "Meets", "body": "Why", "from": e,
                   "to": "F-nope", "relation": "supports"}}]);
    not_found(apply(p, true, &[], &link.to_string()), "F-nope");
    // Named within the batch, by a batch-local reference, it exists.
    let batch = serde_json::json!([
        {"op": "create", "record": {"kind": "evidence", "id": "@seen", "title": "Seen",
                                    "body": "Observed", "source": "log"}},
        {"op": "create", "record": {"kind": "link", "title": "Meets", "body": "Why",
                                    "from": "@seen", "to": f, "relation": "supports"}}]);
    let out = apply(p, true, &[], &batch.to_string());
    assert!(out.status.success(), "{}", stderr_of(&out));
    // The CLI answers the same.
    not_found(run(p, &["--json", "show", &typo]), &typo);
}
/// HYPO-0082: whether cited evidence meets the criterion is asked only of
/// evidence that exists; a prefix gets the rule it breaks.
#[test]
fn a_falsified_citing_evidence_by_prefix_is_told_to_use_full_ids() {
    let project = demo();
    let p = project.path();
    let (h, f, e, token) = demo_claim(p);
    let out = apply(p, true, &[], &falsified(&h, &f, &[&e[..6]], &token));
    let stderr = stderr_of(&out);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("must use full IDs"), "{stderr}");
    assert!(!stderr.contains("meets its criterion"), "{stderr}");
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
    assert!(
        text.starts_with(&format!("{h}  hypothesis  review {}\n", &token[..12])),
        "{text}"
    );
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

fn stderr_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}
fn stdout_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn assess_accepts_a_review_token_prefix_of_12_or_more_hex_digits() {
    let project = demo();
    let p = project.path();
    let h = first(p, "hypothesis");
    let e = first(p, "evidence");
    let token = || -> String {
        let shown: serde_json::Value =
            serde_json::from_str(&ok(p, &["--json", "show", &h])).unwrap();
        shown["state"]["review_token"].as_str().unwrap().to_string()
    };
    // Plain show prints the first 12 hex digits for people to copy.
    assert!(ok(p, &["show", &h]).contains(&format!("review {}", &token()[..12])));
    let assess = |reviewed: &str| {
        run(
            p,
            &[
                "assess",
                &h,
                "--reviewed",
                reviewed,
                "--status",
                "weakened",
                "--evidence",
                &e,
                "--reason",
                "Reviewed",
            ],
        )
    };
    let out = assess(&token()[..11]);
    assert_eq!(out.status.code(), Some(1), "{}", stderr_of(&out));
    assert!(stderr_of(&out).contains("`hyp show"), "{}", stderr_of(&out));
    let stale = token();
    let out = assess(&stale[..12].to_uppercase());
    assert!(out.status.success(), "{}", stderr_of(&out));
    // The assessment changed the token: the old prefix is now a conflict
    // that says how to compare.
    let out = assess(&stale[..12]);
    let stderr = stderr_of(&out);
    assert_eq!(out.status.code(), Some(3), "{stderr}");
    assert!(
        stderr.contains("nothing was written") && stderr.contains(&format!("`hyp show {h}`")),
        "{stderr}"
    );
    let out = assess(&token()[..20]);
    assert!(out.status.success(), "{}", stderr_of(&out));
    // Plain show of the assessment names what based_on is.
    let a = stdout_of(&out).trim().to_string();
    let shown = ok(p, &["show", &a]);
    assert!(shown.contains("based on fingerprint: "), "{shown}");
    assert!(!shown.contains("based_on"), "{shown}");
}

#[test]
fn a_percent_looking_confidence_gets_a_hint() {
    let project = demo();
    let p = project.path();
    let h = first(p, "hypothesis");
    let shown: serde_json::Value = serde_json::from_str(&ok(p, &["--json", "show", &h])).unwrap();
    let token = shown["state"]["review_token"].as_str().unwrap().to_string();
    let assess = |confidence: &str| {
        run(
            p,
            &[
                "assess",
                &h,
                "--reviewed",
                &token,
                "--status",
                "untested",
                "--confidence",
                confidence,
                "--reason",
                "x",
            ],
        )
    };
    let out = assess("80");
    assert_eq!(out.status.code(), Some(1), "{}", stderr_of(&out));
    assert!(
        stderr_of(&out).contains("from 0.0 to 1.0, got 80 (did you mean 0.8?)"),
        "{}",
        stderr_of(&out)
    );
    for unlikely_percent in ["150", "1.5"] {
        let out = assess(unlikely_percent);
        assert_eq!(out.status.code(), Some(1));
        assert!(
            !stderr_of(&out).contains("did you mean"),
            "{}",
            stderr_of(&out)
        );
    }
    let out = assess("high");
    assert_eq!(out.status.code(), Some(2), "not a number: a usage error");
    assert!(stderr_of(&out).contains("a number from 0.0 to 1.0"));
    let help = ok(p, &["assess", "--help"]);
    assert!(help.contains("from 0.0 to 1.0"), "{help}");
    assert!(
        help.contains("after \"review\" on the first line"),
        "{help}"
    );
}

#[test]
fn stdin_can_be_given_to_one_argument_only() {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    ok(p, &["init"]);
    let out = with_stdin(p, &["add", "-", "--body", "-"], "Title\nBody\n");
    assert_eq!(out.status.code(), Some(1), "{}", stderr_of(&out));
    assert!(
        stderr_of(&out).contains("TITLE and --body are each '-'"),
        "{}",
        stderr_of(&out)
    );
    assert_eq!(ok(p, &["list", "--all"]), "", "nothing written");
    let h = ok(p, &["add", "Claim"]).trim().to_string();
    let out = with_stdin(p, &["set", &h, "--title", "-", "--body", "-"], "T\n");
    assert_eq!(out.status.code(), Some(1), "{}", stderr_of(&out));
    assert!(stderr_of(&out).contains("--title and --body"));
    // One '-' still reads stdin, and says nothing when stdin is not a terminal.
    let out = with_stdin(p, &["set", &h, "--body", "-"], "From stdin\n");
    assert!(out.status.success(), "{}", stderr_of(&out));
    assert_eq!(stderr_of(&out), "");
    assert_eq!(record(p, &h)["body"], "From stdin");
}

#[test]
fn a_write_that_changes_nothing_says_so() {
    let project = demo();
    let p = project.path();
    let h = first(p, "hypothesis");
    let revision = || record_revision(p, &h);
    let before = revision();
    let no_op = |args: &[&str], notice: &str| {
        let out = run(p, args);
        assert!(out.status.success(), "{args:?}: {}", stderr_of(&out));
        // The ID contract on stdout is unchanged.
        assert_eq!(stdout_of(&out), format!("{h}\n"), "{args:?}");
        assert_eq!(stderr_of(&out), format!("{notice}\n"), "{args:?}");
    };
    no_op(&["set", &h], "no changes");
    no_op(
        &["restore", &h],
        &format!("no changes: {} is not archived", &h[..10]),
    );
    let out = hyp(p)
        .args(["edit", &h])
        .env("VISUAL", "true")
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr_of(&out));
    assert_eq!(stderr_of(&out), "no changes\n");
    assert_eq!(revision(), before, "nothing was written");
    // A real change prints only the IDs when stderr is not a terminal.
    let out = run(p, &["archive", &h]);
    assert!(out.status.success());
    assert_eq!(stderr_of(&out), "");
    assert_ne!(revision(), before);
    no_op(
        &["archive", &h],
        &format!("no changes: {} is already archived", &h[..10]),
    );
    // --json keeps stderr for errors; an unchanged revision says it there.
    let out = run(p, &["--json", "restore", &h]);
    assert_eq!(stderr_of(&out), "");
    let out = run(p, &["--json", "restore", &h]);
    assert_eq!(stderr_of(&out), "");
}
fn record_revision(p: &Path, id: &str) -> String {
    let shown: serde_json::Value = serde_json::from_str(&ok(p, &["--json", "show", id])).unwrap();
    shown["entry"]["revision"].as_str().unwrap().to_string()
}

/// A pseudo-terminal pair (master, slave): what a process writes to the
/// slave is read from the master, `\n` arriving as `\r\n`. Both are opened
/// close-on-exec (std's default), so only the stdio given to a child leaks.
fn pty() -> (std::fs::File, std::fs::File) {
    use std::os::{fd::AsRawFd, unix::fs::OpenOptionsExt};
    let open = |path: &str| {
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOCTTY)
            .open(path)
            .unwrap_or_else(|e| panic!("open {path}: {e}"))
    };
    let master = open("/dev/ptmx");
    let fd = master.as_raw_fd();
    let mut name = [0 as libc::c_char; 128];
    // SAFETY: libc calls on a descriptor `master` owns; ptsname_r writes a
    // NUL-terminated name into the buffer on success.
    let path = unsafe {
        assert_eq!(libc::grantpt(fd), 0, "grantpt");
        assert_eq!(libc::unlockpt(fd), 0, "unlockpt");
        assert_eq!(libc::ptsname_r(fd, name.as_mut_ptr(), name.len()), 0);
        std::ffi::CStr::from_ptr(name.as_ptr())
            .to_string_lossy()
            .into_owned()
    };
    (master, open(&path))
}
/// Waits for `child`, killing it after `secs` seconds: a hang fails the test
/// instead of stalling it.
fn wait_or_kill(mut child: std::process::Child, secs: u64) -> Output {
    let deadline = std::time::Instant::now() + Duration::from_secs(secs);
    while child.try_wait().unwrap().is_none() {
        if std::time::Instant::now() > deadline {
            child.kill().unwrap();
            let out = child.wait_with_output().unwrap();
            panic!("still running after {secs} s; stderr: {}", stderr_of(&out));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    child.wait_with_output().unwrap()
}
/// Runs `command` with stdin and stderr on a terminal, typing `input` when
/// given: after the command printed a line (or 10 s passed), then Ctrl-D.
/// Returns stdout and what the terminal showed, `\r\n` as `\n`.
fn on_terminal(mut command: Command, input: Option<&str>) -> (Output, String) {
    use std::io::{Read, Write};
    let (mut master, slave) = pty();
    let child = command
        .stdout(Stdio::piped())
        .stdin(slave.try_clone().unwrap())
        .stderr(slave.try_clone().unwrap())
        .spawn()
        .unwrap();
    drop(command);
    drop(slave);
    // Once no process holds the slave, reading the master drains what is
    // buffered and then fails (EIO on Linux), which ends the reader.
    let (sender, chunks) = mpsc::channel();
    let mut reader = master.try_clone().unwrap();
    std::thread::spawn(move || {
        let mut buf = [0; 4096];
        while let Ok(n @ 1..) = reader.read(&mut buf) {
            if sender.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });
    let mut shown = Vec::new();
    if let Some(text) = input {
        while !shown.contains(&b'\n') {
            match chunks.recv_timeout(Duration::from_secs(10)) {
                Ok(chunk) => shown.extend(chunk),
                Err(_) => break,
            }
        }
        master.write_all(text.as_bytes()).unwrap();
        master.write_all(b"\x04").unwrap();
    }
    let out = wait_or_kill(child, 20);
    shown.extend(chunks.iter().flatten());
    (out, String::from_utf8_lossy(&shown).replace("\r\n", "\n"))
}
fn hyp_args(project: &Path, args: &[&str]) -> Command {
    let mut command = hyp(project);
    command.args(args);
    command
}

#[test]
fn writes_summarise_on_a_terminal_and_hint_at_stdin() {
    let project = demo();
    let p = project.path();
    let h = first(p, "hypothesis");
    let (out, shown) = on_terminal(
        hyp_args(p, &["evidence", "add", &h, "Seen twice", "--source", "log"]),
        None,
    );
    assert!(out.status.success(), "{shown}");
    let ids: Vec<String> = stdout_of(&out).lines().map(str::to_string).collect();
    assert_eq!(ids.len(), 2, "stdout keeps one ID per line");
    let (e, l) = (&ids[0][..10], &ids[1][..10]);
    assert_eq!(
        shown,
        format!(
            "created evidence {e} (+ link {l}: {e} supports {})\n",
            &h[..10]
        )
    );
    let p_id = first(p, "prediction");
    let (out, shown) = on_terminal(hyp_args(p, &["archive", &p_id]), None);
    assert!(out.status.success(), "{shown}");
    assert_eq!(shown, format!("archived prediction {}\n", &p_id[..10]));
    // Typing a text argument: a hint first, then the echo of the typing.
    let (out, shown) = on_terminal(hyp_args(p, &["add", "-"]), Some("Typed claim\n"));
    assert!(out.status.success(), "{shown}");
    assert!(
        shown.starts_with("reading TITLE from stdin; end with Ctrl-D\n"),
        "{shown:?}"
    );
    let new = stdout_of(&out).trim().to_string();
    assert!(shown.ends_with(&format!("created hypothesis {}\n", &new[..10])));
    assert_eq!(record(p, &new)["title"], "Typed claim");
}

/// An editor for `hyp edit` tests: `$DIR/round-N.sh FILE` edits the file the
/// N-th time it opens (a missing script saves it unchanged); it logs a copy
/// of what it was shown as `$DIR/seen-N.md`.
fn scripted_editor(dir: &Path, rounds: &[&str]) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    for (i, script) in rounds.iter().enumerate() {
        std::fs::write(dir.join(format!("round-{}.sh", i + 1)), script).unwrap();
    }
    let editor = dir.join("editor.sh");
    std::fs::write(
        &editor,
        format!(
            "#!/bin/sh\nset -e\nd='{}'\nn=$(($(cat \"$d/opened\" 2>/dev/null || echo 0) + 1))\n\
             echo $n > \"$d/opened\"\ncp \"$1\" \"$d/seen-$n.md\"\n\
             if [ -f \"$d/round-$n.sh\" ]; then sh \"$d/round-$n.sh\" \"$1\"; fi\n",
            dir.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&editor, std::fs::Permissions::from_mode(0o755)).unwrap();
    editor
}
/// `hyp edit id` with `editor`, kept copies going to `tmp`: on a terminal
/// (where a rejected edit reopens the editor) or not. Returns the output and
/// stderr (with `terminal`, what the terminal showed).
fn edit_with(p: &Path, editor: &Path, id: &str, tmp: &Path, terminal: bool) -> (Output, String) {
    let mut command = hyp(p);
    command
        .args(["edit", id])
        .env_remove("VISUAL")
        .env("EDITOR", editor)
        .env("TMPDIR", tmp);
    if terminal {
        return on_terminal(command, None);
    }
    let child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let out = wait_or_kill(child, 20);
    let stderr = stderr_of(&out);
    (out, stderr)
}
fn kept_path(stderr: &str) -> &str {
    stderr
        .trim_end()
        .split("your text is kept in ")
        .nth(1)
        .unwrap_or_else(|| panic!("no kept path in {stderr}"))
}

#[test]
fn edit_reopens_with_the_error_and_keeps_the_users_text() {
    let project = demo();
    let p = project.path();
    let h = first(p, "hypothesis");
    let scratch = TempDir::new().unwrap();
    let dir = scratch.path();
    // A YAML error in the first round, fixed in the second.
    let editor = scripted_editor(
        dir,
        &[
            "sed -i 's/^title: .*/title: Kept edit/; s/^scope: .*/scope: [unclosed/' \"$1\"",
            "sed -i 's/^scope: .*/scope: Fixed/' \"$1\"",
        ],
    );
    let (out, stderr) = edit_with(p, &editor, &h, dir, true);
    assert!(out.status.success(), "{stderr}");
    assert_eq!(stdout_of(&out), format!("{h}\n"));
    let r = record(p, &h);
    assert_eq!(
        (&r["title"], &r["scope"]),
        (&"Kept edit".into(), &"Fixed".into())
    );
    let reopened = read(dir.join("seen-2.md"));
    let lines: Vec<&str> = reopened.lines().collect();
    assert_eq!(lines[0], "---");
    assert!(lines[1].starts_with("# hyp: error: "), "{reopened}");
    assert!(lines.contains(&"title: Kept edit"), "{reopened}");
    // The error's line numbers count lines of the file as reopened.
    let at = lines[1]
        .split("at line ")
        .nth(1)
        .and_then(|s| s.split(' ').next())
        .and_then(|n| n.parse::<usize>().ok())
        .unwrap_or_else(|| panic!("no line number in {:?}", lines[1]));
    let flow = lines[1].rsplit("at line ").next().unwrap();
    let flow: usize = flow.split(' ').next().unwrap().parse().unwrap();
    assert_eq!(lines[flow - 1], "scope: [unclosed", "{reopened}");
    assert!(at >= flow, "{reopened}");
    // The notes are not part of the record.
    assert!(!read(p.join(format!("hyp/hypotheses/{h}.md"))).contains("# hyp:"));
}

#[test]
fn edit_aborts_keeping_the_text_when_the_reopened_file_is_saved_unchanged() {
    let project = demo();
    let p = project.path();
    let h = first(p, "hypothesis");
    let before = record_revision(p, &h);
    let scratch = TempDir::new().unwrap();
    let dir = scratch.path();
    let editor = scripted_editor(
        dir,
        &["sed -i 's/^title: .*/title: Precious/; s/^kind: .*/kind: prediction/' \"$1\""],
    );
    let (out, stderr) = edit_with(p, &editor, &h, dir, true);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    assert!(
        read(dir.join("seen-2.md")).contains(
            "# hyp: error: cannot change the kind of a record (hypothesis to prediction)"
        ),
        "{}",
        read(dir.join("seen-2.md"))
    );
    let kept = kept_path(&stderr);
    assert!(kept.starts_with(dir.to_str().unwrap()), "{kept}");
    assert!(read(kept).contains("title: Precious"));
    // The message says why: the abort and the error the user did not fix.
    assert!(
        stderr.contains("saved unchanged")
            && stderr.contains("Last error: cannot change the kind of a record"),
        "{stderr}"
    );
    assert_eq!(record_revision(p, &h), before, "nothing was written");
    // Emptying the file aborts too; with no edit in it there is nothing to keep.
    let empty = TempDir::new().unwrap();
    let editor = scripted_editor(empty.path(), &[": > \"$1\""]);
    let (out, stderr) = edit_with(p, &editor, &h, empty.path(), true);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.contains("emptied") && !stderr.contains("kept"),
        "{stderr}"
    );
}

#[test]
fn edit_refuses_assessments_and_runs_before_opening_the_editor() {
    let project = demo();
    let p = project.path();
    for kind in ["assessment", "run"] {
        let id = first(p, kind);
        let scratch = TempDir::new().unwrap();
        let editor = scripted_editor(scratch.path(), &[]);
        let (out, stderr) = edit_with(p, &editor, &id, scratch.path(), false);
        assert_eq!(out.status.code(), Some(1), "{stderr}");
        assert!(stderr.contains("immutable"), "{stderr}");
        assert!(
            !scratch.path().join("opened").exists(),
            "the editor started for the {kind}"
        );
    }
}

/// An editor that is a script, not a person: each time it copies the same
/// invalid file over the one it is given. Without a terminal hyp must fail
/// on the first error; on one, the second copy is an unchanged save.
#[test]
fn edit_with_a_copying_editor_stops_instead_of_reopening_forever() {
    use std::os::unix::fs::PermissionsExt;
    let project = demo();
    let p = project.path();
    let h = first(p, "hypothesis");
    let before = record_revision(p, &h);
    let invalid = read(p.join(format!("hyp/hypotheses/{h}.md")))
        .replace("kind: hypothesis", "kind: prediction")
        .replace("title: DMA timeout", "title: Copied DMA timeout");
    for terminal in [false, true] {
        let scratch = TempDir::new().unwrap();
        let dir = scratch.path();
        std::fs::write(dir.join("new.md"), &invalid).unwrap();
        let editor = dir.join("copy.sh");
        std::fs::write(
            &editor,
            format!(
                "#!/bin/sh\nd='{}'\necho >> \"$d/opened\"\ncp \"$d/new.md\" \"$1\"\n",
                dir.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&editor, std::fs::Permissions::from_mode(0o755)).unwrap();
        let (out, stderr) = edit_with(p, &editor, &h, dir, terminal);
        assert_eq!(out.status.code(), Some(1), "terminal {terminal}: {stderr}");
        let opened = read(dir.join("opened")).lines().count();
        assert_eq!(opened, if terminal { 2 } else { 1 }, "{stderr}");
        assert!(
            stderr.contains("cannot change the kind of a record (hypothesis to prediction)")
                && stderr.contains("nothing was written"),
            "{stderr}"
        );
        assert!(read(kept_path(&stderr)).contains("title: Copied DMA timeout"));
        assert_eq!(record_revision(p, &h), before, "nothing was written");
    }
    // With --json the error, kept path included, is the {"error"} object.
    let scratch = TempDir::new().unwrap();
    let dir = scratch.path();
    std::fs::write(dir.join("new.md"), &invalid).unwrap();
    let out = hyp(p)
        .args(["--json", "edit", &h])
        .env_remove("VISUAL")
        .env("EDITOR", format!("cp '{}'", dir.join("new.md").display()))
        .env("TMPDIR", dir)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let error: serde_json::Value = serde_json::from_str(&stderr_of(&out)).unwrap();
    let message = error["error"].as_str().unwrap();
    assert!(
        message.contains("cannot change the kind of a record") && message.contains("kept in"),
        "{message}"
    );
}

/// The ID of the hypothesis whose title contains `needle`.
fn hypothesis_titled(p: &Path, needle: &str) -> String {
    let rows: serde_json::Value = serde_json::from_str(&ok(p, &["--json", "list"])).unwrap();
    rows.as_array()
        .unwrap()
        .iter()
        .find(|r| r["record"]["title"].as_str().unwrap().contains(needle))
        .unwrap_or_else(|| panic!("no hypothesis titled *{needle}*"))["record"]["id"]
        .as_str()
        .unwrap()
        .to_string()
}
/// The first line a write printed: the ID of the record it created.
fn first_line(out: String) -> String {
    out.lines().next().unwrap().to_string()
}
/// The groups of plain show's Evidence section, by heading (`against H`,
/// `for H`, ...), each with the lines under it.
fn evidence_groups(text: &str) -> std::collections::BTreeMap<String, String> {
    let section = text
        .split("\n\n")
        .find(|s| s.starts_with("Evidence"))
        .unwrap_or_else(|| panic!("no Evidence section in\n{text}"));
    let mut groups = std::collections::BTreeMap::new();
    let mut current = String::new();
    for line in section.lines().skip(1) {
        if line.starts_with("  ") && !line.starts_with("   ") {
            current = line.trim().to_string();
            groups.insert(current.clone(), String::new());
        } else {
            let group: &mut String = groups.get_mut(&current).expect("a group heading first");
            group.push_str(line);
            group.push('\n');
        }
    }
    groups
}
fn shown_evidence(p: &Path, h: &str, e: &str) -> serde_json::Value {
    let shown: serde_json::Value = serde_json::from_str(&ok(p, &["--json", "show", h])).unwrap();
    let matching: Vec<_> = shown["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|x| x["record"]["id"] == e)
        .cloned()
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "{e} once in .evidence: {}",
        shown["evidence"]
    );
    matching[0].clone()
}

/// HYPO-0071: evidence linked to a criterion with `supports` means the
/// falsifying observation was made, so it counts against the hypothesis.
/// Plain show, and `.evidence[].stance` and `.bearings` of --json show, say
/// what each link means for the hypothesis (HYPO-0073: the path is visible).
#[test]
fn show_presents_evidence_by_what_it_means_for_the_hypothesis() {
    let project = demo();
    let p = project.path();
    let h = hypothesis_titled(p, "cache");
    let (f, pr) = (first(p, "criterion"), first(p, "prediction"));
    let demo_evidence = first(p, "evidence");
    let add = |target: &str, title: &str, extra: &[&str]| {
        let mut args = vec!["evidence", "add", target, title, "--source", "run.log"];
        args.extend(extra);
        first_line(ok(p, &args))
    };
    let meets = add(&f, "Timeout again with D-cache off", &[]);
    let misses = add(&f, "No timeout in 10,000 with D-cache off", &["--against"]);
    let matches = add(&pr, "Clean + invalidate: 0 failures", &[]);
    let text = ok(p, &["show", &h]);
    let groups = evidence_groups(&text);
    let against = &groups["against H"];
    assert!(against.contains("Timeout again with D-cache off"), "{text}");
    assert!(
        against.contains(&format!("meets criterion {} (counts against H)", &f[..10])),
        "{text}"
    );
    // The demo's direct contradiction of H.
    assert!(against.contains("contradicts H · link L-"), "{text}");
    let for_h = &groups["for H"];
    assert!(for_h.contains("No timeout in 10,000"), "{text}");
    assert!(for_h.contains(&format!("does not meet criterion {}", &f[..10])));
    assert!(for_h.contains(&format!("matches prediction {}", &pr[..10])));
    assert!(!for_h.contains("Timeout again"), "{text}");
    assert!(
        !groups.contains_key("supports"),
        "grouped by stance, not link relation:\n{text}"
    );

    let expect = |e: &str, stance: &str, via: &str, relation: &str, meaning: &str| {
        let entry = shown_evidence(p, &h, e);
        assert_eq!(entry["stance"], stance, "{entry}");
        let bearings = entry["bearings"].as_array().unwrap();
        assert_eq!(bearings.len(), 1, "{entry}");
        let b = &bearings[0];
        assert_eq!(b["via"], via, "{entry}");
        assert_eq!(b["relation"], relation, "{entry}");
        assert_eq!(b["stance"], stance, "{entry}");
        assert_eq!(b["meaning"], meaning, "{entry}");
        assert!(b["link"].as_str().unwrap().starts_with("L-"), "{entry}");
    };
    let meets_f = format!("meets criterion {} (counts against H)", &f[..10]);
    expect(&meets, "against", &f, "supports", &meets_f);
    let misses_f = format!("does not meet criterion {} (counts for H)", &f[..10]);
    expect(&misses, "for", &f, "contradicts", &misses_f);
    let matches_p = format!("matches prediction {}", &pr[..10]);
    expect(&matches, "for", &pr, "supports", &matches_p);
    expect(
        &demo_evidence,
        "against",
        &h,
        "contradicts",
        "contradicts H",
    );
}

/// HYPO-0072: one observation linked both to a prediction and to the
/// hypothesis is one record: shown once, with both links under it.
#[test]
fn show_lists_each_observation_once_with_all_its_links() {
    let project = demo();
    let p = project.path();
    let h = hypothesis_titled(p, "cache");
    let pr = first(p, "prediction");
    let out = ok(
        p,
        &[
            "evidence",
            "add",
            &pr,
            "Clean + invalidate: 0 failures",
            "--source",
            "run.log",
            "--body",
            "Unique observation body 7f3a",
        ],
    );
    let (e, via_prediction) = (
        first_line(out.clone()),
        out.lines().nth(1).unwrap().to_string(),
    );
    let direct = ok(
        p,
        &[
            "link",
            &e,
            &h,
            "--relation",
            "supports",
            "--reason",
            "Direct",
        ],
    )
    .trim()
    .to_string();
    let text = ok(p, &["show", &h]);
    assert_eq!(
        text.matches("Unique observation body 7f3a").count(),
        1,
        "{text}"
    );
    assert_eq!(text.matches(&e).count(), 1, "{text}");
    let at = |needle: &str| {
        text.find(needle)
            .unwrap_or_else(|| panic!("{needle} in\n{text}"))
    };
    assert!(
        at(&e) < at(&via_prediction) && at(&e) < at(&direct),
        "{text}"
    );
    let entry = shown_evidence(p, &h, &e);
    let links: Vec<&str> = entry["bearings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["link"].as_str().unwrap())
        .collect();
    assert_eq!(links.len(), 2, "{entry}");
    assert!(links.contains(&via_prediction.as_str()) && links.contains(&direct.as_str()));
}

/// HYPO-0074: plain show's first line carries the 12-hex review token next
/// to the hypothesis ID, and assess accepts it; --json keeps the full token.
#[test]
fn show_header_prints_the_short_review_token_that_assess_accepts() {
    let project = demo();
    let p = project.path();
    let h = hypothesis_titled(p, "cache");
    let e = first(p, "evidence");
    let shown: serde_json::Value = serde_json::from_str(&ok(p, &["--json", "show", &h])).unwrap();
    let token = shown["state"]["review_token"].as_str().unwrap();
    assert_eq!(token.len(), 64);
    let text = ok(p, &["show", &h]);
    let header = text.lines().next().unwrap();
    assert_eq!(header, format!("{h}  hypothesis  review {}", &token[..12]));
    assert!(!text.contains(token), "one token to copy, not two:\n{text}");
    let short = header.rsplit(' ').next().unwrap();
    let out = run(
        p,
        &[
            "assess",
            &h,
            "--reviewed",
            short,
            "--status",
            "weakened",
            "--evidence",
            &e,
            "--reason",
            "Reviewed",
        ],
    );
    assert!(out.status.success(), "{}", stderr_of(&out));
    // Records other than hypotheses have no review token.
    assert!(
        !ok(p, &["show", &e])
            .lines()
            .next()
            .unwrap()
            .contains("review")
    );
}

/// HYPO-0075: assess --help says what each judgment needs, and the error for
/// a missing requirement names the rule.
#[test]
fn assess_help_lists_each_judgment_with_what_it_requires() {
    let project = demo();
    let p = project.path();
    let out = hyp(p)
        .args(["assess", "--help"])
        .env_remove("COLUMNS")
        .output()
        .unwrap();
    let help = stdout_of(&out);
    let line = |judgment: &str| -> String {
        help.lines()
            .find(|l| l.trim_start().starts_with(&format!("- {judgment}:")))
            .unwrap_or_else(|| panic!("no line for {judgment} in\n{help}"))
            .to_string()
    };
    assert!(line("untested").contains("no evidence"), "{help}");
    for judgment in ["inconclusive", "supported", "weakened"] {
        assert!(line(judgment).contains("linked"), "{help}");
    }
    assert!(line("falsified").contains("criterion"), "{help}");
    let h = hypothesis_titled(p, "cache");
    let e = first(p, "evidence");
    let token = serde_json::from_str::<serde_json::Value>(&ok(p, &["--json", "show", &h])).unwrap()
        ["state"]["review_token"]
        .as_str()
        .unwrap()
        .to_string();
    let out = run(
        p,
        &[
            "assess",
            &h,
            "--reviewed",
            &token,
            "--status",
            "falsified",
            "--evidence",
            &e,
            "--reason",
            "Criterion met",
        ],
    );
    let stderr = stderr_of(&out);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    for part in ["falsified", "--criterion", "untested: no evidence"] {
        assert!(stderr.contains(part), "{part:?} in {stderr}");
    }
}

/// HYPO-0079: `hyp status` answers where the investigation stands, per
/// open hypothesis and for the project.
#[test]
fn status_shows_where_each_active_hypothesis_stands() {
    let project = demo();
    let p = project.path();
    let (cache, bus) = (hypothesis_titled(p, "cache"), hypothesis_titled(p, "bus"));
    let gap = first(p, "gap");
    let text = ok(p, &["status"]);
    let block = |h: &str| -> String {
        let lines: Vec<&str> = text.lines().collect();
        let i = lines
            .iter()
            .position(|l| l.starts_with(&h[..10]))
            .unwrap_or_else(|| panic!("{h} in\n{text}"));
        format!("{}\n{}", lines[i], lines[i + 1])
    };
    let c = block(&cache);
    for part in [
        "weakened",
        "DMA timeout is caused by cache coherency",
        "criteria 1",
        "linked evidence 1",
        &format!("open gaps 1: {}", &gap[..10]),
        "experiments without runs 0",
    ] {
        assert!(c.contains(part), "{part:?} in\n{c}");
    }
    assert!(!c.contains("needs review"), "{c}");
    let b = block(&bus);
    for part in [
        "untested",
        "no criterion",
        "linked evidence 1",
        "open gaps 0",
    ] {
        assert!(b.contains(part), "{part:?} in\n{b}");
    }
    assert!(
        text.lines().next().unwrap().contains("writes not blocked"),
        "{text}"
    );
    // Not a place to take a review token from: no run of 12 hex digits.
    assert!(
        !text
            .split(|c: char| !c.is_ascii_hexdigit())
            .any(|w| w.len() >= 12),
        "{text}"
    );

    let x = ok(
        p,
        &["experiment", "add", &bus, "Load the bus with USB traffic"],
    )
    .trim()
    .to_string();
    ok(
        p,
        &[
            "set",
            &cache,
            "--title",
            "DMA timeout is caused by stale cache lines",
        ],
    );
    ok(p, &["set", &gap, "--resolved", "true"]);
    let json: serde_json::Value = serde_json::from_str(&ok(p, &["--json", "status"])).unwrap();
    let rows = json["hypotheses"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["id"], cache, "needs review first: {json}");
    assert_eq!(rows[0]["needs_review"], true);
    assert_eq!(rows[0]["judgment"], "weakened");
    // HYPO-0082: what a row has is listed by ID, never counted.
    let ids = |v: &serde_json::Value, prefix: &str| -> Vec<String> {
        let list = v.as_array().unwrap_or_else(|| panic!("a list: {v}"));
        list.iter()
            .map(|id| id.as_str().unwrap().to_string())
            .inspect(|id| assert!(id.starts_with(prefix) && id.len() == 38, "{id}"))
            .collect()
    };
    assert_eq!(ids(&rows[0]["criteria"], "F-").len(), 1);
    assert_eq!(rows[0]["missing_criterion"], false);
    assert_eq!(ids(&rows[0]["linked_evidence"], "E-").len(), 1);
    assert_eq!(rows[1]["criteria"], serde_json::json!([]));
    assert_eq!(rows[0]["open_gaps"], serde_json::json!([]));
    assert_eq!(rows[1]["id"], bus);
    assert_eq!(rows[1]["missing_criterion"], true);
    assert_eq!(rows[1]["experiments_without_runs"], serde_json::json!([x]));
    assert_eq!(json["writes_blocked"], false);
    assert_eq!(json["blocking"], serde_json::json!([]));
    assert_eq!(json["warnings"], 1, "the bus hypothesis has no criterion");
    assert!(json["hypotheses"][0].get("review_token").is_none());

    // Closed and archived hypotheses are counted, not listed.
    ok(p, &["set", &bus, "--lifecycle", "closed"]);
    let text = ok(p, &["status"]);
    assert!(!text.contains(&bus[..10]), "{text}");
    assert!(text.lines().next().unwrap().contains("1 closed"), "{text}");
    assert!(text.contains("needs review"), "{text}");
}

/// A project an agent cannot write to says so first, with the files to fix.
#[test]
fn status_reports_diagnostics_that_block_writes() {
    let project = demo();
    let p = project.path();
    let h = hypothesis_titled(p, "bus");
    let path = format!("hyp/hypotheses/{h}.md");
    std::fs::write(p.join(&path), "broken").unwrap();
    let out = run(p, &["status"]);
    assert!(out.status.success(), "a read: {}", stderr_of(&out));
    let text = stdout_of(&out);
    assert!(
        text.lines().next().unwrap().contains("writes blocked"),
        "{text}"
    );
    assert!(
        text.lines()
            .any(|l| l.contains("blocks writes") && l.contains(&path)),
        "{text}"
    );
    let json: serde_json::Value = serde_json::from_str(&ok(p, &["--json", "status"])).unwrap();
    assert_eq!(json["writes_blocked"], true);
    let blocking = json["blocking"].as_array().unwrap();
    assert_eq!(blocking.len(), 1, "{json}");
    assert_eq!(blocking[0]["path"], path);
    assert_eq!(blocking[0]["code"], "malformed");
}

/// About four lines per hypothesis at most: ten fit in ~40 lines.
#[test]
fn status_of_ten_hypotheses_fits_in_forty_lines() {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    ok(p, &["init"]);
    for i in 0..10 {
        let h = ok(p, &["add", &format!("Cause number {i}")])
            .trim()
            .to_string();
        ok(p, &["falsify-if", &h, "It reproduces without it"]);
        ok(p, &["gap", &h, "Open question"]);
        ok(p, &["gap", &h, "Another open question"]);
        ok(p, &["experiment", "add", &h, "Try it"]);
        ok(p, &["evidence", "add", &h, "Seen once", "--source", "log"]);
    }
    let text = ok(p, &["status"]);
    assert!(
        text.lines().count() <= 40,
        "{} lines:\n{text}",
        text.lines().count()
    );
}

/// HYPO-0073: agents added a second link to the hypothesis "just in case";
/// the help and the skill say that evidence on a criterion or prediction
/// already counts.
#[test]
fn help_and_skill_say_evidence_on_a_criterion_or_prediction_counts() {
    let dir = TempDir::new().unwrap();
    let words = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
    for args in [["evidence", "add", "--help"], ["link", "--help", ""]] {
        let args: Vec<&str> = args.into_iter().filter(|a| !a.is_empty()).collect();
        let help = words(&ok(dir.path(), &args));
        assert!(
            help.contains(
                "Evidence linked to an active criterion or prediction counts toward its hypothesis"
            ) && help.contains("part of the hypothesis's basis"),
            "{args:?}: {help}"
        );
        assert!(
            help.contains("counts against the hypothesis"),
            "{args:?}: {help}"
        );
    }
    let skill = words(SKILL_SOURCE);
    assert!(
        skill.contains("Evidence linked to an active criterion or prediction is part of its hypothesis's basis and citable"),
        "{skill}"
    );
}

/// An observation is `mixed` only when its links point both ways; a
/// qualifying link does not change the direction of the others, and it is
/// `qualifies` only when every link qualifies.
#[test]
fn qualifying_links_do_not_make_an_observation_mixed() {
    let project = demo();
    let p = project.path();
    let h = hypothesis_titled(p, "cache");
    let (f, pr) = (first(p, "criterion"), first(p, "prediction"));
    let add = |target: &str, title: &str, extra: &[&str]| {
        let mut args = vec!["evidence", "add", target, title, "--source", "run.log"];
        args.extend(extra);
        first_line(ok(p, &args))
    };
    let link = |e: &str, to: &str, relation: &str| {
        ok(p, &["link", e, to, "--relation", relation, "--reason", "r"]);
    };
    let for_q = add(&pr, "Matches, with a caveat", &[]);
    link(&for_q, &h, "qualifies");
    let against_q = add(&f, "Meets the criterion, with a caveat", &[]);
    link(&against_q, &h, "qualifies");
    let only_q = add(&h, "Only a caveat", &["--qualifies"]);
    link(&only_q, &pr, "qualifies");
    let mixed = add(&pr, "Matches the prediction", &[]);
    link(&mixed, &f, "supports");
    for (e, stance) in [
        (&for_q, "for"),
        (&against_q, "against"),
        (&only_q, "qualifies"),
        (&mixed, "mixed"),
    ] {
        assert_eq!(shown_evidence(p, &h, e)["stance"], stance, "{e}");
    }
    let groups = evidence_groups(&ok(p, &["show", &h]));
    assert!(
        groups["for H"].contains("Matches, with a caveat"),
        "{groups:#?}"
    );
    assert!(groups["against H"].contains("Meets the criterion, with a caveat"));
    assert!(groups["qualifies H"].contains("Only a caveat"));
    assert!(groups["mixed: its links disagree"].contains("Matches the prediction"));
}

/// HYPO-0082: an investigation is not "finished" while an archived
/// hypothesis still needs review, and linked evidence counts only records
/// that loaded (a malformed evidence file is `hyp check`'s to report).
#[test]
fn status_is_not_finished_with_an_archived_review_and_skips_unloaded_evidence() {
    let project = demo();
    let p = project.path();
    let (cache, bus) = (hypothesis_titled(p, "cache"), hypothesis_titled(p, "bus"));
    let json: serde_json::Value = serde_json::from_str(&ok(p, &["--json", "status"])).unwrap();
    let row = |json: &serde_json::Value, id: &str| -> serde_json::Value {
        json["hypotheses"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .unwrap_or_else(|| panic!("{id} in {json}"))
            .clone()
    };
    let e = row(&json, &bus)["linked_evidence"][0]
        .as_str()
        .unwrap()
        .to_string();
    let file = p.join(format!("hyp/evidence/{e}.md"));
    let original = read(&file);
    std::fs::write(&file, "broken").unwrap();
    let json: serde_json::Value = serde_json::from_str(&ok(p, &["--json", "status"])).unwrap();
    assert_eq!(row(&json, &bus)["linked_evidence"], serde_json::json!([]));
    let text = ok(p, &["status"]);
    let line = text
        .lines()
        .skip_while(|l| !l.starts_with(&bus[..10]))
        .nth(1);
    assert!(line.unwrap().contains("linked evidence 0"), "{text}");
    std::fs::write(&file, original).unwrap();

    // Archiving the assessed hypothesis changes its basis: it needs review.
    ok(p, &["archive", &cache]);
    ok(p, &["set", &bus, "--lifecycle", "closed"]);
    let json: serde_json::Value = serde_json::from_str(&ok(p, &["--json", "status"])).unwrap();
    assert_eq!(json["hypotheses"], serde_json::json!([]));
    assert_eq!(
        json["not_shown"],
        serde_json::json!({"closed": 1, "archived": 1, "needs_review": 1})
    );
    let header = ok(p, &["status"]).lines().next().unwrap().to_string();
    assert!(header.contains("1 archived needs review"), "{header}");
    assert!(!header.contains("finished"), "{header}");
    ok(p, &["restore", &cache]);
    ok(p, &["set", &cache, "--lifecycle", "closed"]);
    let header = ok(p, &["status"]).lines().next().unwrap().to_string();
    assert!(header.contains("investigation finished"), "{header}");
}

/// A closed hypothesis whose basis changed after its assessment still needs
/// review: `hyp status` lists it, as `hyp list --needs-review` does.
#[test]
fn status_lists_a_closed_hypothesis_that_needs_review() {
    let project = demo();
    let p = project.path();
    let (cache, bus) = (hypothesis_titled(p, "cache"), hypothesis_titled(p, "bus"));
    let pr = first(p, "prediction");
    ok(p, &["set", &cache, "--lifecycle", "closed"]);
    ok(p, &["set", &bus, "--lifecycle", "closed"]);
    let text = ok(p, &["status"]);
    assert!(text.starts_with("0 open hypotheses"), "{text}");
    assert!(
        text.contains("hyp list"),
        "a finished investigation: {text}"
    );
    ok(
        p,
        &[
            "evidence",
            "add",
            &pr,
            "Late observation",
            "--source",
            "log",
        ],
    );
    assert!(ok(p, &["list", "--needs-review"]).contains(&cache));
    let text = ok(p, &["status"]);
    let line = text
        .lines()
        .find(|l| l.starts_with(&cache[..10]))
        .unwrap_or_else(|| panic!("{cache} listed:\n{text}"));
    assert!(
        line.contains("needs review") && line.contains("closed"),
        "{line}"
    );
    assert!(!text.contains(&bus[..10]), "{text}");
    let json: serde_json::Value = serde_json::from_str(&ok(p, &["--json", "status"])).unwrap();
    assert_eq!(json["hypotheses"][0]["id"], cache);
    assert_eq!(json["hypotheses"].as_array().unwrap().len(), 1);
    assert_eq!(json["not_shown"]["closed"], 1, "{json}");
    assert_eq!(json["not_shown"]["needs_review"], 0, "{json}");
    // An archived hypothesis is not listed, even needing review; counted.
    ok(p, &["archive", &cache]);
    let json: serde_json::Value = serde_json::from_str(&ok(p, &["--json", "status"])).unwrap();
    assert_eq!(json["hypotheses"], serde_json::json!([]));
    assert_eq!(json["not_shown"]["archived"], 1, "{json}");
    assert_eq!(json["not_shown"]["needs_review"], 1, "{json}");
}

#[test]
fn init_demo_does_not_suggest_starting_from_an_empty_project() {
    let dir = TempDir::new().unwrap();
    let out = ok(dir.path(), &["init", "--demo"]);
    assert!(!out.contains("hyp add"), "{out}");
    assert!(out.contains("hyp status"), "{out}");
}

/// `--json` error output: `{"error", "kind"}` plus `ids` for a conflict whose
/// failing records are known (HYPO-0078).
fn json_error(out: &Output) -> serde_json::Value {
    let stderr = stderr_of(out);
    serde_json::from_str(&stderr).unwrap_or_else(|e| panic!("{e}: {stderr}"))
}
/// The `record` of `hyp --json show id`.
fn record_of(p: &Path, id: &str) -> serde_json::Value {
    let shown: serde_json::Value = serde_json::from_str(&ok(p, &["--json", "show", id])).unwrap();
    shown["entry"]["record"].clone()
}
fn revision_of(p: &Path, id: &str) -> String {
    let shown: serde_json::Value = serde_json::from_str(&ok(p, &["--json", "show", id])).unwrap();
    shown["entry"]["revision"].as_str().unwrap().to_string()
}

/// HYPO-0077: one batch creates a hypothesis, criterion, prediction,
/// experiment, evidence, a link and a run, naming earlier records only by
/// batch-local references, with no shell variable and no ID round-trip.
#[test]
fn apply_chains_a_whole_step_with_batch_local_references() {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    ok(p, &["init"]);
    let batch = serde_json::json!([
        {"op": "create", "record": {"id": "@h", "kind": "hypothesis",
            "title": "Timezone bug is caused by naive datetimes"}},
        {"op": "create", "record": {"id": "@f", "kind": "criterion", "hypothesis": "@h",
            "title": "Still fails with aware datetimes"}},
        {"op": "create", "record": {"id": "@p", "kind": "prediction", "hypothesis": "@h",
            "title": "Aware datetimes pass at 23:30 UTC"}},
        {"op": "create", "record": {"id": "@x", "kind": "experiment", "hypothesis": "@h",
            "title": "Rerun with aware datetimes", "targets": [{"id": "@f"}]}},
        {"op": "create", "record": {"id": "@e", "kind": "evidence",
            "title": "20/20 passes with aware datetimes", "source": "cargo test tz"}},
        {"op": "create", "record": {"kind": "link", "title": "Criterion not met",
            "from": "@e", "to": "@f", "relation": "contradicts",
            "body": "Aware datetimes removed the failure"}},
        {"op": "create", "record": {"id": "@r", "kind": "run", "title": "Run 1",
            "experiment": "@x", "evidence": ["@e"]}}
    ]);
    let out = apply(p, true, &[], &batch.to_string());
    assert!(out.status.success(), "{}", stderr_of(&out));
    let v: serde_json::Value = serde_json::from_str(&stdout_of(&out)).unwrap();
    let written = v["written"].as_array().unwrap();
    let expected = [
        ("hypothesis", Some("@h"), "H-"),
        ("criterion", Some("@f"), "F-"),
        ("prediction", Some("@p"), "P-"),
        ("experiment", Some("@x"), "X-"),
        ("evidence", Some("@e"), "E-"),
        ("link", None, "L-"),
        ("run", Some("@r"), "R-"),
    ];
    assert_eq!(written.len(), expected.len(), "{v}");
    let mut ids = std::collections::BTreeMap::new();
    for (w, (kind, reference, prefix)) in written.iter().zip(expected) {
        assert_eq!(w["kind"], kind, "{w}");
        assert_eq!(w["ref"].as_str(), reference, "{w}");
        let id = w["id"].as_str().unwrap();
        assert!(id.starts_with(prefix) && id.len() == 38, "a full ID: {w}");
        ids.insert(reference.unwrap_or("link"), id.to_string());
    }
    let run = record_of(p, &ids["@r"]);
    assert_eq!(run["experiment"], ids["@x"].as_str());
    assert_eq!(run["evidence"], serde_json::json!([ids["@e"]]));
    let link = record_of(p, &ids["link"]);
    assert_eq!(
        (&link["from"], &link["to"]),
        (&ids["@e"].clone().into(), &ids["@f"].clone().into())
    );
    let experiment = record_of(p, &ids["@x"]);
    let targets: Vec<&str> = experiment["targets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    // The hypothesis is always a target, as with `hyp experiment add`.
    assert_eq!(targets, [ids["@h"].as_str(), ids["@f"].as_str()]);
    ok(p, &["check", "--strict"]);

    // A statement about a record the batch creates needs no revision; one
    // given is accepted, with the reference resolved.
    let stated = serde_json::json!([
        {"op": "create", "record": {"id": "@h", "kind": "hypothesis", "title": "Second"}},
        {"op": "create", "record": {"kind": "experiment", "hypothesis": "@h", "title": "Check"},
         "expected": {"revisions": {"@h": "not knowable yet"}}}
    ]);
    let out = apply(p, false, &[], &stated.to_string());
    assert!(out.status.success(), "{}", stderr_of(&out));
    assert_eq!(
        stdout_of(&out).lines().count(),
        2,
        "plain: one full ID per line"
    );

    // Unknown, forward and duplicate references are ordinary errors, and
    // nothing of the batch is written.
    let before = ok(p, &["list", "--all"]);
    for (batch, message) in [
        (
            serde_json::json!([{"op": "create", "record": {"kind": "criterion",
                "hypothesis": "@nope", "title": "Orphan"}}]),
            "unknown reference @nope",
        ),
        (
            serde_json::json!([
                {"op": "create", "record": {"kind": "criterion", "hypothesis": "@late", "title": "Too early"}},
                {"op": "create", "record": {"id": "@late", "kind": "hypothesis", "title": "Late"}}]),
            "unknown reference @late",
        ),
        (
            serde_json::json!([
                {"op": "create", "record": {"id": "@twice", "kind": "hypothesis", "title": "One"}},
                {"op": "create", "record": {"id": "@twice", "kind": "hypothesis", "title": "Two"}}]),
            "@twice is defined twice",
        ),
        (
            serde_json::json!([{"op": "create", "record": {"kind": "gap", "hypothesis": "@x",
                "title": "Q", "colour": "blue"}}]),
            "every record takes title, body, tags and archived",
        ),
    ] {
        let out = apply(p, true, &[], &batch.to_string());
        assert_eq!(out.status.code(), Some(1), "{}", stderr_of(&out));
        let error = json_error(&out);
        assert!(
            error["error"].as_str().unwrap().contains(message),
            "{error}"
        );
        assert_eq!(error["kind"], "invalid_input", "{error}");
    }
    assert_eq!(ok(p, &["list", "--all"]), before, "nothing written");
}

/// HYPO-0053: a create needs only what the CLI would ask for, and a patch
/// changes only the fields it names.
#[test]
fn apply_creates_with_cli_defaults_and_patches_only_given_fields() {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    ok(p, &["init"]);
    let out = apply(
        p,
        false,
        &[],
        r#"[{"op": "create", "record": {"kind": "hypothesis", "title": "Minimal"}}]"#,
    );
    assert!(out.status.success(), "{}", stderr_of(&out));
    let h = stdout_of(&out).trim().to_string();
    let r = record_of(p, &h);
    assert_eq!(r["lifecycle"], "draft");
    assert_eq!(r["tags"], serde_json::json!([]));
    assert!(!r["created_at"].as_str().unwrap().is_empty(), "{r}");
    let out = apply(
        p,
        false,
        &[],
        &serde_json::json!([
            {"op": "create", "record": {"kind": "gap", "hypothesis": h, "title": "Open?"}},
            {"op": "create", "record": {"kind": "criterion", "hypothesis": h, "title": "Refuted if"}},
            {"op": "create", "record": {"kind": "experiment", "hypothesis": h, "title": "Try"},
             "expected": {"revisions": {h.clone(): revision_of(p, &h)}}}
        ])
        .to_string(),
    );
    assert!(out.status.success(), "{}", stderr_of(&out));
    let ids: Vec<String> = stdout_of(&out).lines().map(str::to_string).collect();
    assert_eq!(record_of(p, &ids[0])["resolved"], false);
    assert_eq!(record_of(p, &ids[2])["status"], "planned");

    // A patch names the fields it changes; the rest stays as stored.
    ok(p, &["set", &h, "--body", "Kept body", "--tags", "keep"]);
    let before = record_of(p, &h);
    let patch = |set: serde_json::Value, revision: &str| {
        serde_json::json!([{"op": "patch", "id": h, "expected_revision": revision, "set": set}])
            .to_string()
    };
    let out = apply(
        p,
        true,
        &[],
        &patch(
            serde_json::json!({"title": "Patched", "scope": "CI only"}),
            &revision_of(p, &h),
        ),
    );
    assert!(out.status.success(), "{}", stderr_of(&out));
    let after = record_of(p, &h);
    assert_eq!(after["title"], "Patched");
    assert_eq!(after["scope"], "CI only");
    for kept in ["body", "tags", "lifecycle", "created_at"] {
        assert_eq!(after[kept], before[kept], "{kept} kept");
    }
    // A stale revision is a conflict, like an update's.
    let stale = before_revision_after_write(p, &h);
    let out = apply(
        p,
        true,
        &[],
        &patch(serde_json::json!({"title": "Stale"}), &stale),
    );
    assert_eq!(out.status.code(), Some(3), "{}", stderr_of(&out));
    // Fields that cannot change, unknown fields and rule violations are
    // ordinary errors; immutable kinds are refused.
    let revision = revision_of(p, &h);
    for (set, message) in [
        (serde_json::json!({"kind": "gap"}), "cannot change kind"),
        (serde_json::json!({"id": "H-x"}), "cannot change id"),
        (
            serde_json::json!({"created_at": "2020-01-01T00:00:00Z"}),
            "cannot change created_at",
        ),
        (
            serde_json::json!({"colour": "blue"}),
            "unknown field `colour`",
        ),
        (
            serde_json::json!({"colour": "blue"}),
            "every record also takes title, body, tags and archived",
        ),
        (
            serde_json::json!({"lifecycle": "sideways"}),
            "unknown variant `sideways`",
        ),
    ] {
        let out = apply(p, true, &[], &patch(set.clone(), &revision));
        assert_eq!(out.status.code(), Some(1), "{set}: {}", stderr_of(&out));
        let error = json_error(&out);
        assert!(
            error["error"].as_str().unwrap().contains(message),
            "{set}: {error}"
        );
    }
    let demo = demo();
    let a = first(demo.path(), "assessment");
    let out = apply(
        demo.path(),
        false,
        &[],
        &serde_json::json!([{"op": "patch", "id": a, "expected_revision": revision_of(demo.path(), &a),
            "set": {"title": "Rewritten"}}])
        .to_string(),
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr_of(&out).contains("immutable"), "{}", stderr_of(&out));
}
/// A revision of `id` that a later write made stale.
fn before_revision_after_write(p: &Path, id: &str) -> String {
    let stale = revision_of(p, id);
    ok(p, &["set", id, "--tags", "moved-on"]);
    stale
}

/// HYPO-0078: with --json, every error names its kind; exit codes stay.
#[test]
fn json_errors_carry_a_machine_readable_kind() {
    let project = demo();
    let p = project.path();
    let kind_of = |args: &[&str], code: i32| -> serde_json::Value {
        let mut all = vec!["--json"];
        all.extend_from_slice(args);
        let out = run(p, &all);
        assert_eq!(
            out.status.code(),
            Some(code),
            "{args:?}: {}",
            stderr_of(&out)
        );
        json_error(&out)
    };
    assert_eq!(kind_of(&["show", "H-00000000"], 1)["kind"], "not_found");
    let out = hyp(&p.join("no-such-dir"))
        .args(["--json", "status"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(json_error(&out)["kind"], "not_found", "a missing --project");
    assert_eq!(kind_of(&["show", "H"], 1)["kind"], "ambiguous_id");
    let e = first(p, "evidence");
    let missing = p.join("no-such-file.log");
    assert_eq!(
        kind_of(&["evidence", "attach", &e, missing.to_str().unwrap()], 1)["kind"],
        "io"
    );
    assert_eq!(kind_of(&["add", "Two\nlines"], 1)["kind"], "invalid_input");
    // A conflict lists the records whose preconditions failed.
    let h = first(p, "hypothesis");
    let stale = before_revision_after_write(p, &h);
    let out = apply(
        p,
        true,
        &[],
        &serde_json::json!([{"op": "archive", "id": h, "archived": true, "expected_revision": stale}])
            .to_string(),
    );
    assert_eq!(out.status.code(), Some(3));
    let error = json_error(&out);
    assert_eq!(error["kind"], "conflict", "{error}");
    assert_eq!(error["ids"], serde_json::json!([h]), "{error}");
    let x = first(p, "experiment");
    let out = apply(
        p,
        true,
        &[],
        &serde_json::json!([{"op": "create",
            "record": {"kind": "run", "title": "Stale", "experiment": x},
            "expected": {"revisions": {x.clone(): "0".repeat(64)}}}])
        .to_string(),
    );
    assert_eq!(out.status.code(), Some(3), "{}", stderr_of(&out));
    let error = json_error(&out);
    assert_eq!(
        (&error["kind"], &error["ids"]),
        (&"conflict".into(), &serde_json::json!([x])),
        "{error}"
    );
    // A malformed file blocks every write.
    std::fs::write(p.join("hyp/hypotheses/H-broken.md"), "not front matter").unwrap();
    assert_eq!(kind_of(&["add", "Blocked"], 1)["kind"], "blocked");
}

/// HYPO-0076: a gap is resolved by the evidence that answered it, shown by
/// `hyp show` and `hyp status`, and outside the review basis.
#[test]
fn a_gap_is_resolved_by_the_evidence_that_answered_it() {
    let project = demo();
    let p = project.path();
    let (h, g, e1) = (
        hypothesis_titled(p, "cache"),
        first(p, "gap"),
        first(p, "evidence"),
    );
    let gap_file = p.join(format!("hyp/gaps/{g}.md"));
    assert!(
        !read(&gap_file).contains("resolved_by"),
        "not written while empty"
    );
    let e2 = ok(
        p,
        &[
            "evidence",
            "add",
            &h,
            "Timing unchanged",
            "--source",
            "scope trace",
        ],
    )
    .lines()
    .next()
    .unwrap()
    .to_string();
    let token = |h: &str| record_state(p, h)["review_token"].clone();
    let reviewed = token(&h);
    let status: serde_json::Value = serde_json::from_str(&ok(p, &["--json", "status"])).unwrap();
    let row = |status: &serde_json::Value| {
        status["hypotheses"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == h.as_str())
            .unwrap()
            .clone()
    };
    assert_eq!(row(&status)["open_gaps"], serde_json::json!([{"id": g}]));
    assert_eq!(row(&status)["resolved_gaps"], serde_json::json!([]));
    // --by applies only when the gap ends up resolved, and names evidence.
    fails(p, &["set", &g, "--by", &e1], "--resolved true");
    fails(
        p,
        &["set", &g, "--resolved", "true", "--by", &h],
        "expected evidence",
    );
    fails(
        p,
        &["set", &h, "--resolved", "true", "--by", &e1],
        "resolved only applies to gaps",
    );
    ok(
        p,
        &[
            "set",
            &g,
            "--resolved",
            "true",
            "--by",
            &e1[..10],
            "--by",
            &e2,
        ],
    );
    assert_eq!(record_of(p, &g)["resolved_by"], serde_json::json!([e1, e2]));
    ok(p, &["set", &g, "--by", &format!("{e2},{e1}")]);
    assert_eq!(record_of(p, &g)["resolved_by"], serde_json::json!([e2, e1]));
    assert_eq!(
        token(&h),
        reviewed,
        "gaps are not part of the basis (decision-0003)"
    );
    let shown = ok(p, &["show", &h]);
    assert!(
        shown.contains(&format!("[resolved by {e2}, {e1}]")),
        "{shown}"
    );
    let status: serde_json::Value = serde_json::from_str(&ok(p, &["--json", "status"])).unwrap();
    let row = row(&status);
    assert_eq!(row["open_gaps"], serde_json::json!([]), "{row}");
    assert_eq!(
        row["resolved_gaps"],
        serde_json::json!([{"id": g, "resolved_by": [e2, e1]}]),
        "{row}"
    );
    let plain = ok(p, &["status"]);
    assert!(
        plain.contains(&format!(
            "resolved gaps 1: {} by {}, {}",
            &g[..10],
            &e2[..10],
            &e1[..10]
        )),
        "{plain}"
    );
    // Evidence that resolved a gap cannot be deleted from under it.
    ok(p, &["archive", &e2]);
    fails(p, &["delete", &e2], &g);
    // Reopening forgets what resolved it.
    ok(p, &["set", &g, "--resolved", "false"]);
    assert_eq!(record_of(p, &g).get("resolved_by"), None);
    assert!(!read(&gap_file).contains("resolved_by"));
    ok(p, &["check"]);
}
fn record_state(p: &Path, id: &str) -> serde_json::Value {
    let shown: serde_json::Value = serde_json::from_str(&ok(p, &["--json", "show", id])).unwrap();
    shown["state"].clone()
}

/// Drift guard for the skill's `hyp apply` batch example (HYPO-0077): its
/// ```json block applies as written to a fresh project, references only.
#[test]
fn skill_batch_example_applies_as_written() {
    let block: String = SKILL_SOURCE
        .split("```json\n")
        .nth(1)
        .and_then(|rest| rest.split("\n```").next())
        .expect("a ```json block in the skill")
        .to_string();
    let dir = TempDir::new().unwrap();
    ok(dir.path(), &["init"]);
    let out = apply(dir.path(), true, &[], &block);
    assert!(out.status.success(), "{}\n{block}", stderr_of(&out));
    let v: serde_json::Value = serde_json::from_str(&stdout_of(&out)).unwrap();
    assert_eq!(v["written"].as_array().unwrap().len(), 6, "{v}");
    ok(dir.path(), &["check", "--strict"]);
}

/// A record the batch creates cannot also be updated, patched, archived or
/// deleted in it: that is input to fix (put its values in the create), not a
/// conflict that a retry could resolve.
#[test]
fn changing_a_record_created_in_the_same_batch_is_invalid_input() {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    ok(p, &["init"]);
    let full = "H-00000000-0000-4000-8000-000000000001";
    for (id, second) in [
        (
            "@h",
            serde_json::json!({"op": "patch", "id": "@h", "expected_revision": "x", "set": {"title": "Later"}}),
        ),
        (
            "@h",
            serde_json::json!({"op": "archive", "id": "@h", "archived": true, "expected_revision": "x"}),
        ),
        (
            "@h",
            serde_json::json!({"op": "delete", "id": "@h", "expected_revision": "x"}),
        ),
        (
            full,
            serde_json::json!({"op": "update", "expected_revision": "x",
            "record": {"id": full, "kind": "hypothesis", "title": "Later"}}),
        ),
        (
            full,
            serde_json::json!({"op": "archive", "id": &full[..10], "archived": true, "expected_revision": "x"}),
        ),
    ] {
        let batch = serde_json::json!([
            {"op": "create", "record": {"id": id, "kind": "hypothesis", "title": "Now"}},
            second
        ]);
        let out = apply(p, true, &[], &batch.to_string());
        assert_eq!(out.status.code(), Some(1), "{batch}: {}", stderr_of(&out));
        let error = json_error(&out);
        assert_eq!(error["kind"], "invalid_input", "{error}");
        let message = error["error"].as_str().unwrap();
        assert!(
            message.contains("is created by this batch; put its values in the create"),
            "{message}"
        );
        if id == "@h" {
            assert!(message.contains("@h"), "names the reference: {message}");
        }
    }
    assert_eq!(ok(p, &["list", "--all"]), "", "nothing written");
}

/// decision-0004 / HYPO-0085: the notebook's schema, from `hyp/config.toml`.
fn schema_of(p: &Path) -> i64 {
    let config: toml::Table = toml::from_str(&read(p.join("hyp/config.toml"))).unwrap();
    config["schema_version"].as_integer().unwrap()
}
/// A schema-1 notebook stays byte-for-byte schema 1 through reads and
/// writes, until a write first stores something only schema 2 holds (a
/// gap's `resolved_by`); then config.toml says 2, and it never goes back.
#[test]
fn a_schema_1_notebook_is_raised_to_2_only_by_the_first_resolved_by() {
    let project = demo();
    let p = project.path();
    let config = || read(p.join("hyp/config.toml"));
    let original = config();
    assert_eq!(schema_of(p), 1, "{original}");
    let h = first(p, "hypothesis");
    let gap = ok(p, &["gap", &h, "Is the clock involved?"]);
    let gap = gap.trim();
    ok(p, &["set", gap, "--resolved", "true"]);
    ok(p, &["status"]);
    ok(p, &["--json", "export", "--format", "json"]);
    ok(p, &["check"]);
    assert_eq!(config(), original, "reads and schema-1 writes keep it");
    let e = first(p, "evidence");
    ok(p, &["set", gap, "--resolved", "true", "--by", &e]);
    assert_eq!(schema_of(p), 2, "{}", config());
    assert_eq!(
        config().replace("schema_version = 2", "schema_version = 1"),
        original,
        "only the schema changes"
    );
    ok(p, &["set", gap, "--resolved", "false"]);
    assert_eq!(schema_of(p), 2, "never lowered");
    ok(p, &["check"]);
}
/// The raise is part of the write's journal: a write interrupted after the
/// journal was saved is rolled forward, config.toml included, by the next
/// hyp that takes the lock.
#[test]
fn an_interrupted_schema_raise_is_rolled_forward_with_the_records() {
    let project = demo();
    let p = project.path();
    let raised =
        read(p.join("hyp/config.toml")).replace("schema_version = 1", "schema_version = 2");
    let journal = serde_json::json!({ "config.toml": raised });
    std::fs::write(p.join(".hyp/transaction.json"), journal.to_string()).unwrap();
    ok(p, &["list"]);
    assert_eq!(schema_of(p), 2);
    assert!(!p.join(".hyp/transaction.json").exists());
}
/// A notebook of a schema this hyp does not know is refused up front, with
/// one error naming the version to upgrade to, not one "unknown field" per
/// file; new config keys of that schema do not get in the way.
#[test]
fn a_notebook_of_a_newer_schema_is_refused_with_the_version_to_upgrade_to() {
    let project = demo();
    let p = project.path();
    let before = ok(p, &["--json", "export", "--format", "json"]);
    let refused = |config: &str, upgrade: &str| {
        std::fs::write(p.join("hyp/config.toml"), config).unwrap();
        for args in [
            &["--json", "list"][..],
            &["--json", "status"],
            &["--json", "add", "Written by an older hyp"],
        ] {
            let out = run(p, args);
            let stderr = stderr_of(&out);
            assert_eq!(out.status.code(), Some(1), "{args:?}: {stderr}");
            let error: serde_json::Value = serde_json::from_str(&stderr).unwrap();
            assert_eq!(error["kind"], "unsupported_schema", "{stderr}");
            let message = error["error"].as_str().unwrap();
            for part in ["uses schema 3", "reads schemas 1 to 2", upgrade] {
                assert!(message.contains(part), "{part:?} in {message}");
            }
        }
    };
    refused(
        "schema_version = 3\nname = \"demo\"\nmin_hyp_version = \"0.3.0\"\nnew_setting = true\n",
        "upgrade hyp to >= 0.3.0",
    );
    refused(
        "schema_version = 3\nname = \"demo\"\n",
        &format!("newer than {}", env!("CARGO_PKG_VERSION")),
    );
    std::fs::write(
        p.join("hyp/config.toml"),
        "schema_version = 2\nname = \"demo\"\n",
    )
    .unwrap();
    assert_eq!(
        ok(p, &["--json", "export", "--format", "json"]),
        before,
        "nothing written"
    );
}
/// Strict as before for the schemas this hyp reads: an unknown key, a
/// schema that never existed, or text that is not TOML is invalid input.
#[test]
fn a_config_with_an_unknown_key_or_garbage_is_invalid() {
    let project = demo();
    let p = project.path();
    for (config, part) in [
        (
            "schema_version = 2\nname = \"demo\"\ncolour = \"red\"\n",
            "unknown field `colour`",
        ),
        (
            "schema_version = 0\nname = \"demo\"\n",
            "schema_version 0 does not exist",
        ),
        ("schema_version = [\n", "invalid"),
        ("name = \"demo\"\n", "missing field `schema_version`"),
    ] {
        std::fs::write(p.join("hyp/config.toml"), config).unwrap();
        let out = run(p, &["--json", "list"]);
        let stderr = stderr_of(&out);
        assert_eq!(out.status.code(), Some(1), "{config}: {stderr}");
        let error: serde_json::Value = serde_json::from_str(&stderr).unwrap();
        assert_eq!(error["kind"], "invalid_input", "{config}: {stderr}");
        let message = error["error"].as_str().unwrap();
        assert!(message.contains("hyp/config.toml"), "{message}");
        assert!(message.contains(part), "{part:?} in {message}");
    }
}
/// The schema table in code names released hyp versions no newer than
/// this one, and the README lists the same rows.
#[test]
fn the_schema_table_matches_this_version_and_the_readme() {
    let parse = |v: &str| -> Vec<u64> { v.split('.').map(|n| n.parse().unwrap()).collect() };
    let schemas = hyp::store::SCHEMAS;
    let (_, newest) = schemas[schemas.len() - 1];
    assert!(parse(newest) <= parse(env!("CARGO_PKG_VERSION")));
    let readme = include_str!("../README.md");
    let rows: Vec<Vec<&str>> = readme
        .lines()
        .filter(|l| l.starts_with('|'))
        .map(|l| l.split('|').map(str::trim).collect())
        .collect();
    for (i, (schema, version)) in schemas.iter().enumerate() {
        assert_eq!(*schema, i as u32 + 1, "consecutive from 1");
        let (schema, version) = (schema.to_string(), format!("`{version}`"));
        assert!(
            rows.iter()
                .any(|r| r.len() > 3 && r[1] == schema && r[2] == version),
            "README schema table lacks | {schema} | {version} |"
        );
    }
}
