//! Captured data records (HYPO-0090, decision-0005), through the real `hyp`
//! binary: capture, references, the review basis, `hyp check`, retrieval and
//! the schema-3 migration of evidence attachments.
use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
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
fn stderr_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}
fn ok(project: &Path, args: &[&str]) -> String {
    let out = run(project, args);
    assert!(
        out.status.success(),
        "hyp {args:?} failed: {}",
        stderr_of(&out)
    );
    String::from_utf8(out.stdout).unwrap()
}
/// Runs hyp with `input` on stdin.
fn with_stdin(project: &Path, args: &[&str], input: &[u8]) -> Output {
    let mut child = hyp(project)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}
fn demo() -> TempDir {
    let dir = TempDir::new().unwrap();
    ok(dir.path(), &["init", "--demo"]);
    dir
}
fn json(text: &str) -> serde_json::Value {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("{e}: {text}"))
}
fn shown(p: &Path, id: &str) -> serde_json::Value {
    json(&ok(p, &["--json", "show", id]))
}
fn record(p: &Path, id: &str) -> serde_json::Value {
    shown(p, id)["entry"]["record"].clone()
}
fn json_error(out: &Output) -> serde_json::Value {
    json(&stderr_of(out))
}
/// The one ID a write printed.
fn id_of(stdout: String) -> String {
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 1, "{stdout}");
    lines[0].to_string()
}
fn first(p: &Path, kind: &str) -> String {
    let rows = json(&ok(p, &["--json", "list", "--kind", kind]));
    rows[0]["record"]["id"].as_str().unwrap().to_string()
}
fn hypothesis_titled(p: &Path, title: &str) -> String {
    let rows = json(&ok(p, &["--json", "list"]));
    let row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["record"]["title"] == title)
        .unwrap_or_else(|| panic!("no hypothesis {title:?}"));
    row["record"]["id"].as_str().unwrap().to_string()
}
fn read(path: impl AsRef<Path>) -> String {
    std::fs::read_to_string(path.as_ref())
        .unwrap_or_else(|e| panic!("{}: {e}", path.as_ref().display()))
}
fn config(p: &Path) -> toml::Table {
    toml::from_str(&read(p.join("hyp/config.toml"))).unwrap()
}
fn schema_of(p: &Path) -> i64 {
    config(p)["schema_version"].as_integer().unwrap()
}
fn sha256(bytes: &[u8]) -> String {
    hyp::model::hash(bytes)
}
/// Every file under `hyp/`, relative path -> bytes.
fn files(p: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(dir: &Path, root: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, root, out);
            } else {
                let bytes = std::fs::read(&path).unwrap();
                out.insert(path.strip_prefix(root).unwrap().to_path_buf(), bytes);
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(&p.join("hyp"), p, &mut out);
    out
}
fn needs_review(p: &Path) -> BTreeMap<String, bool> {
    let rows = json(&ok(p, &["--json", "list"]));
    rows.as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["record"]["id"].as_str().unwrap().to_string(),
                r["state"]["needs_review"].as_bool().unwrap(),
            )
        })
        .collect()
}
fn fingerprint(p: &Path, h: &str) -> String {
    shown(p, h)["state"]["fingerprint"]
        .as_str()
        .unwrap()
        .to_string()
}

/// A file capture and a stdin capture of the same bytes are two records
/// sharing one stored file; repeating a capture exactly writes nothing.
#[test]
fn capture_from_a_file_and_stdin_stores_the_bytes_once() {
    let project = demo();
    let p = project.path();
    assert_eq!(schema_of(p), 1);
    let bytes = b"23 of 200 runs failed\nfirst at 08:14:02\n";
    let file = p.join("ci-run.log");
    std::fs::write(&file, bytes).unwrap();
    let from_file = id_of(ok(
        p,
        &[
            "capture",
            file.to_str().unwrap(),
            "--origin",
            "CI job 4402 artifact",
        ],
    ));
    assert!(from_file.starts_with("D-"), "{from_file}");
    let r = record(p, &from_file);
    let sha = sha256(bytes);
    assert_eq!(r["kind"], "data");
    assert_eq!(r["title"], "ci-run.log", "default title: the file name");
    assert_eq!(r["origin"], "CI job 4402 artifact");
    assert_eq!(r["media_type"], "text/plain");
    assert_eq!(r["size"], bytes.len());
    assert_eq!(r["sha256"], sha.as_str());
    let captured_at = r["captured_at"].as_str().unwrap();
    assert!(chrono::DateTime::parse_from_rfc3339(captured_at).is_ok());
    // The first data record raises the notebook, naming the hyp that reads it.
    assert_eq!(schema_of(p), 3);
    assert_eq!(config(p)["min_hyp_version"].as_str(), Some("0.3.0"));

    let args = [
        "capture",
        "-",
        "--origin",
        "journalctl -u ci --since today\nsecond line",
        "--body",
        "Right after the failure",
    ];
    let out = with_stdin(p, &args, bytes);
    assert!(out.status.success(), "{}", stderr_of(&out));
    let from_stdin = id_of(String::from_utf8(out.stdout).unwrap());
    assert_ne!(from_stdin, from_file, "new metadata, new record");
    let r = record(p, &from_stdin);
    assert_eq!(r["sha256"], sha.as_str());
    assert_eq!(r["title"], "journalctl -u ci --since today");
    assert_eq!(r["body"], "Right after the failure");
    let stored: Vec<_> = std::fs::read_dir(p.join("hyp/assets"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(
        stored,
        std::slice::from_ref(&sha),
        "one stored file for both"
    );
    assert_eq!(
        std::fs::read(p.join("hyp/assets").join(&sha)).unwrap(),
        bytes
    );

    // A retried capture: the same ID, nothing written.
    let before = files(p);
    let again = with_stdin(p, &args, bytes);
    assert!(again.status.success(), "{}", stderr_of(&again));
    assert!(
        stderr_of(&again).contains("no changes"),
        "{}",
        stderr_of(&again)
    );
    assert_eq!(String::from_utf8(again.stdout).unwrap().trim(), from_stdin);
    assert_eq!(files(p), before);

    // Binary stdin, and the size limit.
    let binary = with_stdin(
        p,
        &["--json", "capture", "-", "--origin", "dd"],
        &[0, 159, 146, 150],
    );
    assert!(binary.status.success(), "{}", stderr_of(&binary));
    let written = json(&String::from_utf8(binary.stdout).unwrap());
    let id = written["written"][0]["id"].as_str().unwrap();
    assert_eq!(written["written"][0]["kind"], "data");
    assert_eq!(record(p, id)["media_type"], "application/octet-stream");
    let big = vec![b'x'; 32 * 1024 * 1024 + 1];
    let out = with_stdin(p, &["--json", "capture", "-", "--origin", "yes"], &big);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        json_error(&out)["error"]
            .as_str()
            .unwrap()
            .contains("32 MiB"),
        "{}",
        stderr_of(&out)
    );
    ok(p, &["check"]);
}

/// Data records cannot change once captured; archiving and restoring them
/// still work.
#[test]
fn data_records_are_immutable_but_can_be_archived() {
    let project = demo();
    let p = project.path();
    let file = p.join("m.csv");
    std::fs::write(&file, "t,v\n1,2\n").unwrap();
    let d = id_of(ok(
        p,
        &["capture", file.to_str().unwrap(), "--origin", "scope"],
    ));
    assert_eq!(record(p, &d)["media_type"], "text/csv");
    let revision = shown(p, &d)["entry"]["revision"]
        .as_str()
        .unwrap()
        .to_string();
    let refused = |out: Output| {
        assert_eq!(out.status.code(), Some(1), "{}", stderr_of(&out));
        assert!(stderr_of(&out).contains("immutable"), "{}", stderr_of(&out));
    };
    refused(run(p, &["set", &d, "--title", "Renamed"]));
    refused(run(p, &["set", &d, "--tags", "x"]));
    let patch = serde_json::json!([{"op": "patch", "id": d, "expected_revision": revision,
        "set": {"origin": "elsewhere"}}]);
    refused(with_stdin(p, &["apply"], patch.to_string().as_bytes()));
    let mut changed = record(p, &d);
    changed["size"] = 1.into();
    let update = serde_json::json!([{"op": "update", "record": changed,
        "expected_revision": revision}]);
    refused(with_stdin(p, &["apply"], update.to_string().as_bytes()));
    let edit = hyp(p)
        .args(["edit", &d])
        .env("VISUAL", "true")
        .output()
        .unwrap();
    refused(edit);
    ok(p, &["archive", &d]);
    assert_eq!(record(p, &d)["archived"], true);
    ok(p, &["restore", &d]);
    assert_eq!(record(p, &d)["archived"], false);
    assert_eq!(record(p, &d)["origin"], "scope");
}

/// `--data` on the commands that create records, and on `hyp set`; one data
/// record referenced from many; deleting it is refused while referenced,
/// naming the referrers, and `hyp show` lists every one.
#[test]
fn records_reference_data_and_a_referenced_data_record_cannot_be_deleted() {
    let project = demo();
    let p = project.path();
    let file = p.join("capture.txt");
    std::fs::write(&file, "timeout at 8,142").unwrap();
    let d = id_of(ok(
        p,
        &["capture", file.to_str().unwrap(), "--origin", "rig 3"],
    ));
    let file2 = p.join("capture2.txt");
    std::fs::write(&file2, "no timeout").unwrap();
    let d2 = id_of(ok(
        p,
        &["capture", file2.to_str().unwrap(), "--origin", "rig 4"],
    ));
    let short = &d[..10];

    let e = id_of(ok(
        p,
        &[
            "observe",
            "Timeout at 8,142",
            "--source",
            "rig 3",
            "--data",
            short,
        ],
    ));
    assert_eq!(record(p, &e)["data"], serde_json::json!([d]));
    let out = ok(
        p,
        &[
            "add",
            "The ring overflows",
            "--explains",
            &e,
            "--data",
            &format!("{short},{short}"),
        ],
    );
    let h = out.lines().next().unwrap().to_string();
    assert_eq!(record(p, &h)["data"], serde_json::json!([d]), "each once");
    let f = id_of(ok(
        p,
        &["falsify-if", &h, "No overflow counter moves", "--data", &d],
    ));
    let pr = id_of(ok(p, &["predict", &h, "Counter moves", "--data", &d]));
    let g = id_of(ok(p, &["gap", &h, "Which ring?", "--data", &d]));
    let out = ok(
        p,
        &[
            "evidence",
            "add",
            &h,
            "Second look",
            "--source",
            "s",
            "--data",
            &d,
        ],
    );
    let e2 = out.lines().next().unwrap().to_string();
    let x = first(p, "experiment");
    let r = id_of(ok(p, &["run", &x, "Run 2", "--data", &d]));
    let token = shown(p, &h)["state"]["review_token"]
        .as_str()
        .unwrap()
        .to_string();
    let a = id_of(ok(
        p,
        &[
            "assess",
            &h,
            "--reviewed",
            &token,
            "--status",
            "untested",
            "--reason",
            "Not yet",
            "--data",
            &d,
        ],
    ));
    for id in [&f, &pr, &g, &e2, &r, &a] {
        assert_eq!(record(p, id)["data"], serde_json::json!([d]), "{id}");
    }
    // `hyp set --data` replaces the list.
    ok(p, &["set", &e, "--data", &format!("{d2},{d}")]);
    assert_eq!(record(p, &e)["data"], serde_json::json!([d2, d]));
    // Only data records.
    let out = run(
        p,
        &["--json", "observe", "x", "--source", "s", "--data", &e],
    );
    assert_eq!(out.status.code(), Some(1));
    let message = json_error(&out)["error"].as_str().unwrap().to_string();
    assert!(
        message.contains("--data: expected data, got evidence"),
        "{message}"
    );

    // Every referrer, in the plain and the JSON form.
    let referrers = [&e, &h, &f, &pr, &g, &e2, &r, &a];
    let plain = ok(p, &["show", &d]);
    assert!(plain.contains("Referenced by"), "{plain}");
    let related = shown(p, &d)["related"].clone();
    for id in referrers {
        assert!(plain.contains(id.as_str()), "{id} in {plain}");
        assert!(
            related
                .as_array()
                .unwrap()
                .iter()
                .any(|x| x["record"]["id"] == **id),
            "{id} in related"
        );
    }
    assert!(ok(p, &["show", &e]).contains(&format!("{d}  text/plain")));

    ok(p, &["archive", &d]);
    let out = run(p, &["delete", &d]);
    assert_eq!(out.status.code(), Some(1));
    let stderr = stderr_of(&out);
    assert!(
        stderr.contains(&format!("cannot delete {d}: these records refer to it")),
        "{stderr}"
    );
    for id in [&e, &h] {
        assert!(stderr.contains(id.as_str()), "{id} in {stderr}");
    }
    assert!(p.join("hyp/data").join(format!("{d}.md")).exists());
}

/// In `hyp apply`, data references accept batch-local references, and a
/// data record can be created for bytes hyp already stores.
#[test]
fn apply_resolves_batch_local_references_in_data() {
    let project = demo();
    let p = project.path();
    let out = with_stdin(
        p,
        &["capture", "-", "--origin", "uname -a"],
        b"Linux rig3 6.1\n",
    );
    assert!(out.status.success(), "{}", stderr_of(&out));
    let captured = id_of(String::from_utf8(out.stdout).unwrap());
    let sha = sha256(b"Linux rig3 6.1\n");
    let batch = serde_json::json!([
        {"op": "create", "record": {"id": "@d", "kind": "data", "title": "Kernel",
            "origin": "uname -a on rig3", "sha256": sha}},
        {"op": "create", "record": {"id": "@e", "kind": "evidence", "title": "Kernel 6.1",
            "source": "rig3", "data": ["@d", captured]}},
        {"op": "create", "record": {"id": "@h", "kind": "hypothesis",
            "title": "The 6.1 driver drops IRQs", "data": ["@d"]}},
        {"op": "create", "record": {"kind": "link", "title": "Fits", "from": "@e", "to": "@h",
            "relation": "supports", "body": "The version with the known bug"}}
    ]);
    let out = with_stdin(p, &["--json", "apply"], batch.to_string().as_bytes());
    assert!(out.status.success(), "{}", stderr_of(&out));
    let written = json(&String::from_utf8(out.stdout).unwrap())["written"].clone();
    let id = |i: usize| written[i]["id"].as_str().unwrap().to_string();
    let (d, e, h) = (id(0), id(1), id(2));
    assert!(d.starts_with("D-"), "{d}");
    let data = record(p, &d);
    assert_eq!(data["size"], 15, "set by the server from the stored bytes");
    assert_eq!(data["media_type"], "text/plain", "guessed when not given");
    assert_eq!(record(p, &e)["data"], serde_json::json!([d, captured]));
    assert_eq!(record(p, &h)["data"], serde_json::json!([d]));

    // Bytes hyp does not store, and server-set fields, are refused.
    for (record, part) in [
        (
            serde_json::json!({"kind": "data", "title": "t", "origin": "o", "sha256": "0".repeat(64)}),
            "capture them with hyp capture",
        ),
        (
            serde_json::json!({"kind": "data", "title": "t", "origin": "o", "sha256": sha, "size": 3}),
            "set by the server",
        ),
        (
            serde_json::json!({"kind": "evidence", "title": "t", "source": "s", "data": ["@nope"]}),
            "unknown reference @nope",
        ),
    ] {
        let batch = serde_json::json!([{"op": "create", "record": record}]);
        let out = with_stdin(p, &["--json", "apply"], batch.to_string().as_bytes());
        assert_eq!(out.status.code(), Some(1), "{}", stderr_of(&out));
        let message = json_error(&out)["error"].as_str().unwrap().to_string();
        assert!(message.contains(part), "{part:?} in {message}");
    }
}

/// The review basis includes the hashes of referenced data: referencing
/// other bytes flags the assessment, going back unflags it. Missing or
/// changed bytes block writes with a repair note, and capturing the original
/// again repairs them.
#[test]
fn referenced_bytes_are_in_the_basis_and_broken_bytes_block_writes() {
    let project = demo();
    let p = project.path();
    let h = hypothesis_titled(p, "DMA timeout is caused by cache coherency");
    let e = first(p, "evidence");
    let (one, two) = (p.join("one.log"), p.join("two.log"));
    std::fs::write(&one, "timeout at 8,142\n").unwrap();
    std::fs::write(&two, "timeout at 9,001\n").unwrap();
    let d1 = id_of(ok(
        p,
        &["capture", one.to_str().unwrap(), "--origin", "rig"],
    ));
    let d2 = id_of(ok(
        p,
        &["capture", two.to_str().unwrap(), "--origin", "rig"],
    ));
    assert!(!needs_review(p)[&h], "capturing alone changes no basis");
    let before = fingerprint(p, &h);

    ok(p, &["set", &e, "--data", &d1]);
    let with_one = fingerprint(p, &h);
    assert_ne!(with_one, before);
    assert!(needs_review(p)[&h]);
    let basis = shown(p, &h)["basis"][&e].clone();
    assert_eq!(
        basis["attachments"],
        serde_json::json!([sha256(b"timeout at 8,142\n")])
    );
    ok(p, &["set", &e, "--data", &d2]);
    assert_ne!(fingerprint(p, &h), with_one, "other bytes, other basis");
    ok(p, &["set", &e, "--data", &d1]);
    assert_eq!(
        fingerprint(p, &h),
        with_one,
        "content only: back to the same"
    );
    // Data the hypothesis references itself counts too.
    ok(p, &["set", &h, "--data", &d2]);
    assert_ne!(fingerprint(p, &h), with_one);

    // Changed bytes: blocked, with the repair.
    let sha = sha256(b"timeout at 8,142\n");
    let blob = p.join("hyp/assets").join(&sha);
    std::fs::write(&blob, "tampered").unwrap();
    let out = run(p, &["--json", "check"]);
    assert_eq!(out.status.code(), Some(1));
    let diagnostics = json(&String::from_utf8(out.stdout).unwrap());
    let d = diagnostics
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "attachment")
        .unwrap_or_else(|| panic!("{diagnostics:#}"))
        .clone();
    assert_eq!(d["path"], format!("hyp/data/{d1}.md"));
    assert_eq!(d["blocks_writes"], true);
    assert!(d["message"].as_str().unwrap().contains("changed"), "{d}");
    let note = d["repair"]["note"].as_str().unwrap();
    assert!(
        note.contains(&format!("Restore hyp/assets/{sha}")),
        "{note}"
    );
    assert!(note.contains("capture the original again"), "{note}");
    let plain = String::from_utf8(run(p, &["check"]).stdout).unwrap();
    assert!(
        plain.contains(&format!("note: Restore hyp/assets/{sha}")),
        "{plain}"
    );
    let out = run(p, &["--json", "add", "Blocked"]);
    assert_eq!(json_error(&out)["kind"], "blocked", "{}", stderr_of(&out));
    let out = run(p, &["data", "get", &d1]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty(), "nothing written");
    // Missing bytes too.
    std::fs::remove_file(&blob).unwrap();
    let plain = String::from_utf8(run(p, &["check"]).stdout).unwrap();
    assert!(plain.contains("missing"), "{plain}");
    // Capturing the original again stores the same bytes at the same path.
    ok(
        p,
        &["capture", one.to_str().unwrap(), "--origin", "rig, again"],
    );
    ok(p, &["check"]);
    assert_eq!(read(&blob), "timeout at 8,142\n");
}

/// HYPO-0004: ordinary reads check stored bytes by metadata only, so bytes
/// changed in place to others of the same length go unnoticed by them (and
/// do not block unrelated writes). `hyp check` hashes every stored file and
/// reports them as `changed_bytes`, which says it does not block every
/// write; a write that newly cites them is refused, and so is an assessment
/// whose basis holds them; `hyp data get` refuses to return them; a write
/// that drops the citation goes through.
#[test]
fn bytes_changed_in_place_are_caught_by_check_and_by_writes_that_cite_them() {
    let project = demo();
    let p = project.path();
    let e = first(p, "evidence");
    let h = hypothesis_titled(p, "DMA timeout is caused by cache coherency");
    let file = p.join("run.log");
    std::fs::write(&file, "timeout at 8,142\n").unwrap();
    let d = id_of(ok(
        p,
        &["capture", file.to_str().unwrap(), "--origin", "rig"],
    ));
    ok(p, &["set", &h, "--data", &d]);
    let blob = p.join("hyp/assets").join(sha256(b"timeout at 8,142\n"));
    std::fs::write(&blob, "timeout at 9,999\n").unwrap();

    // Not seen by an ordinary read; an unrelated write goes through.
    let status = json(&ok(p, &["--json", "status"]));
    assert!(!status.to_string().contains("not intact"), "{status:#}");
    ok(p, &["add", "Unrelated"]);

    // hyp check hashes, and finds it.
    let out = run(p, &["--json", "check"]);
    assert_eq!(out.status.code(), Some(1));
    let diagnostics = json(&String::from_utf8(out.stdout).unwrap());
    let d0 = diagnostics
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["code"] == "changed_bytes")
        .unwrap_or_else(|| panic!("{diagnostics:#}"));
    assert_eq!(d0["path"], format!("hyp/data/{d}.md"));
    assert_eq!(d0["blocks_writes"], false, "{d0}");
    assert!(d0["message"].as_str().unwrap().contains("changed"), "{d0}");
    let note = d0["repair"]["note"].as_str().unwrap();
    assert!(note.contains("refuses writes that newly cite"), "{note}");

    // An assessment of a hypothesis whose basis cites the bytes is refused.
    let token = shown(p, &h)["state"]["review_token"]
        .as_str()
        .unwrap()
        .to_string();
    let out = run(
        p,
        &[
            "--json",
            "assess",
            &h,
            "--reviewed",
            &token,
            "--status",
            "untested",
            "--reason",
            "Not yet",
        ],
    );
    assert_eq!(out.status.code(), Some(1), "{}", stderr_of(&out));
    let message = json_error(&out)["error"].as_str().unwrap().to_string();
    assert!(
        message.contains("cannot assess") && message.contains(&d),
        "{message}"
    );

    // A write that newly cites the bytes is refused, naming them.
    let out = run(p, &["--json", "set", &e, "--data", &d]);
    assert_eq!(out.status.code(), Some(1), "{}", stderr_of(&out));
    let message = json_error(&out)["error"].as_str().unwrap().to_string();
    assert!(
        message.contains(&d) && message.contains("not intact"),
        "{message}"
    );
    let out = run(
        p,
        &["observe", "Seen again", "--source", "rig", "--data", &d],
    );
    assert_eq!(out.status.code(), Some(1), "{}", stderr_of(&out));
    // Nor are they returned.
    let out = run(p, &["data", "get", &d]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    // Dropping the citation is not blocked by the broken bytes.
    let other = p.join("other.log");
    std::fs::write(&other, "other").unwrap();
    let d2 = id_of(ok(
        p,
        &["capture", other.to_str().unwrap(), "--origin", "rig"],
    ));
    ok(p, &["set", &h, "--data", &d2]);
}

/// `hyp data get` writes the bytes back exactly, to stdout or a file.
#[test]
fn data_get_round_trips_the_bytes() {
    let project = demo();
    let p = project.path();
    // Every byte value, invalid UTF-8 and NULs included, over a megabyte.
    let bytes: Vec<u8> = (0..(1 << 20) + 7)
        .map(|i: u32| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect();
    let file = p.join("capture.pcap");
    std::fs::write(&file, &bytes).unwrap();
    let d = id_of(ok(
        p,
        &[
            "capture",
            file.to_str().unwrap(),
            "--origin",
            "tcpdump -i eth0",
        ],
    ));
    assert_eq!(record(p, &d)["media_type"], "application/vnd.tcpdump.pcap");
    let out = run(p, &["data", "get", &d[..10]]);
    assert!(out.status.success(), "{}", stderr_of(&out));
    assert!(out.stdout == bytes, "stdout differs");
    let copy = p.join("copy.pcap");
    ok(p, &["data", "get", &d, "--output", copy.to_str().unwrap()]);
    assert!(std::fs::read(&copy).unwrap() == bytes, "file differs");
    // Only data records have bytes.
    let e = first(p, "evidence");
    let out = run(p, &["data", "get", &e]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr_of(&out).contains("expected data"),
        "{}",
        stderr_of(&out)
    );
    // The derived preview is for text only.
    let out = with_stdin(p, &["capture", "-", "--origin", "echo"], b"<b>hi</b>\n");
    let text = id_of(String::from_utf8(out.stdout).unwrap());
    let export = json(&ok(p, &["export", "--format", "json"]));
    assert!(
        export["previews"].get(&d).is_none(),
        "no preview of binary data"
    );
    assert_eq!(
        export["previews"][&text], "<b>hi</b>\n",
        "text as it is; the UI escapes it"
    );
}

/// The evidence records of a hyp 0.2.0 notebook with attachments: the first
/// has two (one shared with the second), the second one.
struct Legacy {
    dir: TempDir,
    h: String,
    evidence: [String; 2],
    shas: [String; 2],
    /// The evidence's `updated_at` before any migration.
    updated: [serde_json::Value; 2],
}
/// A schema-2 notebook as hyp 0.2.0 leaves it: evidence with attachments
/// (two sharing their bytes), and a current assessment reviewed after them.
fn legacy() -> Legacy {
    use hyp::{model::*, store};
    let dir = demo();
    let p = dir.path();
    let h = hypothesis_titled(p, "DMA timeout is caused by cache coherency");
    let e1 = first(p, "evidence");
    let out = ok(
        p,
        &[
            "evidence",
            "add",
            &h,
            "Second timeout",
            "--source",
            "rig",
            "--against",
        ],
    );
    let e2 = out.lines().next().unwrap().to_string();
    // Schema 2, the way hyp 0.2.0 raised it.
    let g = first(p, "gap");
    ok(p, &["set", &g, "--resolved", "true", "--by", &e1]);
    assert_eq!(schema_of(p), 2);
    let text = b"timeout at transfer 8,142\n".to_vec();
    let binary = vec![0u8, 1, 2, 255, 254];
    let shas = [sha256(&text), sha256(&binary)];
    for (bytes, sha) in [(&text, &shas[0]), (&binary, &shas[1])] {
        std::fs::write(p.join("hyp/assets").join(sha), bytes).unwrap();
    }
    let attach = |e: &str, shas: &[&String]| {
        let path = p.join("hyp/evidence").join(format!("{e}.md"));
        let mut r = store::decode(&read(&path)).unwrap();
        if let Data::Evidence { attachments, .. } = &mut r.data {
            for sha in shas {
                attachments.push(Attachment {
                    path: format!("assets/{sha}"),
                    sha256: (*sha).clone(),
                });
            }
        }
        std::fs::write(&path, store::encode(&r).unwrap()).unwrap();
        assert!(read(&path).contains("attachments:"));
    };
    attach(&e1, &[&shas[0], &shas[1]]);
    attach(&e2, &[&shas[0]]);
    ok(p, &["check"]);
    assert!(needs_review(p)[&h], "the attachments are in the basis");
    let token = shown(p, &h)["state"]["review_token"]
        .as_str()
        .unwrap()
        .to_string();
    ok(
        p,
        &[
            "assess",
            &h,
            "--reviewed",
            &token,
            "--status",
            "weakened",
            "--evidence",
            &e1,
            "--reason",
            "Reviewed with the captures",
        ],
    );
    assert!(
        needs_review(p).values().all(|n| !n),
        "{:?}",
        needs_review(p)
    );
    assert_eq!(schema_of(p), 2);
    let updated = [&e1, &e2].map(|e| record(p, e)["updated_at"].clone());
    Legacy {
        dir,
        h,
        evidence: [e1, e2],
        shas,
        updated,
    }
}
/// HYPO-0067: `(path, code)` identifies a diagnostic, and every path has one
/// form, relative to the project root: a record's file, or for a legacy
/// attachment its stored bytes, under `hyp/` (they were a bare ID and a path
/// relative to `hyp/`). A missing attachment is reported once, not again
/// when `hyp check` hashes stored bytes.
#[test]
fn every_diagnostic_names_a_path_under_hyp() {
    let l = legacy();
    let p = l.dir.path();
    let h = id_of(ok(p, &["add", "Without a criterion"]));
    std::fs::remove_file(p.join("hyp/assets").join(&l.shas[1])).unwrap();
    let out = run(p, &["--json", "check"]);
    assert_eq!(out.status.code(), Some(1));
    let diagnostics = json(&String::from_utf8_lossy(&out.stdout));
    let identities: Vec<(&str, &str)> = diagnostics
        .as_array()
        .unwrap()
        .iter()
        .map(|d| (d["path"].as_str().unwrap(), d["code"].as_str().unwrap()))
        .collect();
    let missing = format!("hyp/assets/{}", l.shas[1]);
    let warning = format!("hyp/hypotheses/{h}.md");
    assert!(
        identities.contains(&(warning.as_str(), "no_criterion")),
        "{identities:?}"
    );
    assert_eq!(
        identities
            .iter()
            .filter(|(path, _)| *path == missing)
            .collect::<Vec<_>>(),
        [&(missing.as_str(), "attachment")],
        "{identities:?}"
    );
    for (path, _) in &identities {
        assert!(path.starts_with("hyp/"), "{identities:?}");
        if !path.starts_with("hyp/assets/") {
            assert!(p.join(path).is_file(), "{path} names the record's file");
        }
    }
}
/// Checks a migrated legacy notebook: schema 3, no attachments left, one
/// data record per distinct stored file with the ID derived from its hash,
/// title, origin and times from the notebook (the earliest `created_at` of
/// the evidence holding it); each evidence referencing them first, in
/// attachment order; the first evidence's `updated_at` as it was; `hyp
/// check` clean; and when given, the fingerprint as before the migration.
fn assert_migrated(l: &Legacy, fingerprint_before: Option<&str>) {
    let p = l.dir.path();
    assert_eq!(schema_of(p), 3);
    assert_eq!(config(p)["min_hyp_version"].as_str(), Some("0.3.0"));
    for (path, bytes) in files(p) {
        let text = String::from_utf8_lossy(&bytes);
        assert!(
            !text.contains("attachments:"),
            "{} still has attachments",
            path.display()
        );
    }
    let data = json(&ok(p, &["--json", "list", "--kind", "data"]));
    let migrated: Vec<&serde_json::Value> = data
        .as_array()
        .unwrap()
        .iter()
        .map(|r| &r["record"])
        .filter(|r| {
            r["origin"]
                .as_str()
                .unwrap()
                .starts_with("migrated from evidence attachment")
        })
        .collect();
    assert_eq!(
        migrated.len(),
        2,
        "one per distinct stored file: {migrated:#?}"
    );
    let e1_created = record(p, &l.evidence[0])["created_at"].clone();
    let by_sha = |sha: &str| -> String {
        let r = migrated.iter().find(|r| r["sha256"] == sha).unwrap();
        assert_eq!(r["id"], hyp::store::migrated_id(sha));
        assert_eq!(r["title"], format!("Migrated attachment {}", &sha[..8]));
        assert_eq!(
            r["origin"],
            format!("migrated from evidence attachment assets/{sha}")
        );
        // Both are held by the first evidence, the earlier one.
        for time in ["created_at", "updated_at", "captured_at"] {
            assert_eq!(r[time], e1_created, "{time}");
        }
        r["id"].as_str().unwrap().to_string()
    };
    let (text, binary) = (by_sha(&l.shas[0]), by_sha(&l.shas[1]));
    let refs =
        |e: &str| -> Vec<String> { serde_json::from_value(record(p, e)["data"].clone()).unwrap() };
    assert!(refs(&l.evidence[0]).starts_with(&[text.clone(), binary.clone()]));
    assert!(refs(&l.evidence[1]).starts_with(std::slice::from_ref(&text)));
    assert_eq!(
        record(p, &l.evidence[0])["updated_at"],
        l.updated[0],
        "the conversion keeps updated_at"
    );
    assert_eq!(record(p, &text)["media_type"], "text/plain");
    assert_eq!(record(p, &binary)["media_type"], "application/octet-stream");
    if let Some(before) = fingerprint_before {
        assert_eq!(fingerprint(p, &l.h), before, "the basis is the same");
        assert!(
            needs_review(p).values().all(|n| !n),
            "{:?}",
            needs_review(p)
        );
    }
    ok(p, &["check"]);
}

/// A schema-2 notebook with attachments reads unchanged; writes that need
/// no data do not migrate; the first data write raises it to 3 and turns
/// every attachment into a data record, reusing the stored files.
#[test]
fn attachments_become_data_records_on_the_first_data_write_only() {
    let l = legacy();
    let p = l.dir.path();
    let fingerprint_before = fingerprint(p, &l.h);
    let before = files(p);
    for args in [
        &["list"][..],
        &["status"],
        &["show", &l.evidence[0]],
        &["--json", "show", &l.h],
        &["export", "--format", "json"],
        &["check"],
    ] {
        ok(p, args);
    }
    assert_eq!(files(p), before, "reads change nothing");
    ok(p, &["add", "Unrelated claim"]);
    assert_eq!(schema_of(p), 2, "a write without data does not migrate");
    let still = files(p);
    let (text, binary) = (&l.shas[0], &l.shas[1]);
    for sha in [text, binary] {
        assert!(still.contains_key(&PathBuf::from(format!("hyp/assets/{sha}"))));
    }

    let file = p.join("new.txt");
    std::fs::write(&file, "unrelated capture").unwrap();
    let out = run(
        p,
        &[
            "--json",
            "capture",
            file.to_str().unwrap(),
            "--origin",
            "here",
        ],
    );
    assert!(out.status.success(), "{}", stderr_of(&out));
    // Agents see what the write converted besides what it named.
    let printed = json(&String::from_utf8(out.stdout).unwrap());
    let mut migrated: Vec<(String, String)> = printed["migrated"]
        .as_array()
        .unwrap_or_else(|| panic!("{printed}"))
        .iter()
        .map(|w| {
            assert!(w["revision"].is_string(), "{w}");
            (
                w["kind"].as_str().unwrap().into(),
                w["id"].as_str().unwrap().into(),
            )
        })
        .collect();
    migrated.sort();
    let mut expected = vec![
        ("data".to_string(), hyp::store::migrated_id(&l.shas[0])),
        ("data".to_string(), hyp::store::migrated_id(&l.shas[1])),
        ("evidence".to_string(), l.evidence[0].clone()),
        ("evidence".to_string(), l.evidence[1].clone()),
    ];
    expected.sort();
    assert_eq!(migrated, expected);
    assert_migrated(&l, Some(&fingerprint_before));
    assert_eq!(record(p, &l.evidence[1])["updated_at"], l.updated[1]);
    let after = files(p);
    for sha in [text, binary] {
        let path = PathBuf::from(format!("hyp/assets/{sha}"));
        assert_eq!(
            after[&path], still[&path],
            "stored files are reused as they are"
        );
    }
}

/// A data reference in a batch and `hyp evidence attach` are data writes as
/// well, and neither changes what the migration makes.
#[test]
fn a_data_reference_or_an_attach_also_migrates() {
    for how in ["apply", "attach new bytes", "attach attached bytes"] {
        let l = legacy();
        let p = l.dir.path();
        let fingerprint_before = fingerprint(p, &l.h);
        let data_count = || {
            json(&ok(p, &["--json", "list", "--kind", "data"]))
                .as_array()
                .unwrap()
                .len()
        };
        match how {
            "apply" => {
                // A new data record with the bytes of an attachment is its
                // own record; the migration still makes the derived one.
                let batch = serde_json::json!([
                    {"op": "create", "record": {"id": "@d", "kind": "data", "title": "Same",
                        "origin": "o", "sha256": l.shas[0]}},
                    {"op": "create", "record": {"kind": "evidence", "title": "Seen",
                        "source": "s", "data": ["@d"]}}]);
                let out = with_stdin(p, &["apply"], batch.to_string().as_bytes());
                assert!(out.status.success(), "{}", stderr_of(&out));
                assert_eq!(data_count(), 3);
                assert_migrated(&l, Some(&fingerprint_before));
            }
            "attach new bytes" => {
                // After its migrated attachment, the evidence references the
                // new data record.
                let file = p.join("more.txt");
                std::fs::write(&file, "timeout at transfer 9,001\n").unwrap();
                let out = ok(
                    p,
                    &["evidence", "attach", &l.evidence[1], file.to_str().unwrap()],
                );
                let lines: Vec<&str> = out.lines().collect();
                assert_eq!(lines[0], l.evidence[1]);
                assert_eq!(record(p, lines[1])["origin"], file.to_str().unwrap());
                let refs = record(p, &l.evidence[1])["data"].clone();
                assert_eq!(refs[1], lines[1], "{refs}");
                assert_eq!(refs.as_array().unwrap().len(), 2, "{refs}");
                assert_ne!(fingerprint(p, &l.h), fingerprint_before);
                assert_migrated(&l, None);
            }
            _ => {
                // The bytes a legacy attachment holds, attached to other
                // evidence: the derived record, and no other.
                let e3 = id_of(ok(p, &["observe", "Third", "--source", "rig"]));
                let file = p.join("same.txt");
                std::fs::write(&file, "timeout at transfer 8,142\n").unwrap();
                let out = ok(p, &["evidence", "attach", &e3, file.to_str().unwrap()]);
                let lines: Vec<&str> = out.lines().collect();
                assert_eq!(lines, [e3.clone(), hyp::store::migrated_id(&l.shas[0])]);
                assert_eq!(record(p, &e3)["data"], serde_json::json!([lines[1]]));
                assert_eq!(data_count(), 2);
                assert_migrated(&l, Some(&fingerprint_before));
            }
        }
    }
}

/// Two copies of one notebook (parallel worktrees, clones) migrate
/// independently, each triggered by its own capture, to the same files:
/// every file both have is byte-for-byte equal, and each has in addition
/// only its own capture. So Git merges them without a conflict.
#[test]
fn two_copies_of_a_notebook_migrate_to_identical_files() {
    let l = legacy();
    let a = l.dir.path();
    let copy = TempDir::new().unwrap();
    let b = copy.path();
    for (path, bytes) in files(a) {
        let target = b.join(&path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, bytes).unwrap();
    }
    let own = |p: &Path, text: &str| -> Vec<PathBuf> {
        let file = p.join("own.txt");
        std::fs::write(&file, text).unwrap();
        let d = id_of(ok(
            p,
            &["capture", file.to_str().unwrap(), "--origin", text],
        ));
        vec![
            PathBuf::from(format!("hyp/data/{d}.md")),
            PathBuf::from(format!("hyp/assets/{}", sha256(text.as_bytes()))),
        ]
    };
    let only_a = own(a, "captured in the first copy");
    // A second later, so a clock-based conversion would differ.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let only_b = own(b, "captured in the second copy");
    let (fa, fb) = (files(a), files(b));
    let mut shared = 0;
    for (path, bytes) in &fa {
        match fb.get(path) {
            Some(other) => {
                assert!(bytes == other, "{} differs", path.display());
                shared += 1;
            }
            None => assert!(
                only_a.contains(path),
                "{} only in the first",
                path.display()
            ),
        }
    }
    for path in fb.keys().filter(|p| !fa.contains_key(*p)) {
        assert!(
            only_b.contains(path),
            "{} only in the second",
            path.display()
        );
    }
    assert!(shared > 10, "{shared}");
    ok(a, &["check"]);
}

/// Identical captures (and attaches of the same new bytes) run at the same
/// time make one data record: the lookup happens under the write lock.
#[test]
fn concurrent_identical_captures_make_one_record() {
    let project = demo();
    let p = project.path();
    let children: Vec<_> = (0..8)
        .map(|_| {
            let mut child = hyp(p)
                .args(["capture", "-", "--origin", "dmesg"])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            child
                .stdin
                .take()
                .unwrap()
                .write_all(b"usb 1-1: reset\n")
                .unwrap();
            child
        })
        .collect();
    let ids: std::collections::BTreeSet<String> = children
        .into_iter()
        .map(|c| {
            let out = c.wait_with_output().unwrap();
            assert!(out.status.success(), "{}", stderr_of(&out));
            id_of(String::from_utf8(out.stdout).unwrap())
        })
        .collect();
    assert_eq!(ids.len(), 1, "{ids:?}");
    let data = json(&ok(p, &["--json", "list", "--kind", "data"]));
    assert_eq!(data.as_array().unwrap().len(), 1, "{data:#}");

    // Attaching the same new bytes to several evidence at once.
    let h = first(p, "hypothesis");
    let evidence: Vec<String> = (0..6)
        .map(|i| {
            let out = ok(
                p,
                &["evidence", "add", &h, &format!("Seen {i}"), "--source", "s"],
            );
            out.lines().next().unwrap().to_string()
        })
        .collect();
    let file = p.join("shared.log");
    std::fs::write(&file, "the same bytes for all").unwrap();
    let children: Vec<_> = evidence
        .iter()
        .map(|e| {
            hyp(p)
                .args(["evidence", "attach", e, file.to_str().unwrap()])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    let ids: std::collections::BTreeSet<String> = children
        .into_iter()
        .map(|c| {
            let out = c.wait_with_output().unwrap();
            assert!(out.status.success(), "{}", stderr_of(&out));
            String::from_utf8(out.stdout)
                .unwrap()
                .lines()
                .nth(1)
                .unwrap()
                .to_string()
        })
        .collect();
    assert_eq!(ids.len(), 1, "{ids:?}");
    let data = json(&ok(p, &["--json", "list", "--kind", "data"]));
    assert_eq!(data.as_array().unwrap().len(), 2, "{data:#}");
    for e in &evidence {
        assert_eq!(
            record(p, e)["data"],
            serde_json::json!([ids.first().unwrap()])
        );
    }
}

/// `hyp check` tells a record changed by hand from bytes changed or lost,
/// reports a symlink among the stored files, and duplicate data references
/// are invalid.
#[test]
fn check_classifies_data_problems_by_what_changed() {
    let project = demo();
    let p = project.path();
    let file = p.join("x.log");
    std::fs::write(&file, "twelve bytes").unwrap();
    let d = id_of(ok(p, &["capture", file.to_str().unwrap(), "--origin", "o"]));
    let path = p.join("hyp/data").join(format!("{d}.md"));
    let original = read(&path);
    std::fs::write(&path, original.replace("size: 12", "size: 13")).unwrap();
    // Only errors: the demo also has a warning.
    let errors = |p: &Path| -> Vec<serde_json::Value> {
        let all = json(&String::from_utf8(run(p, &["--json", "check"]).stdout).unwrap());
        all.as_array()
            .unwrap()
            .iter()
            .filter(|d| d["severity"] == "error")
            .cloned()
            .collect()
    };
    let diagnostics = errors(p);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
    let d0 = &diagnostics[0];
    assert_eq!(d0["code"], "invalid", "{d0}");
    assert_eq!(d0["path"], format!("hyp/data/{d}.md"));
    assert_eq!(d0["blocks_writes"], true);
    let note = d0["repair"]["note"].as_str().unwrap();
    assert!(
        note.contains("record file") && note.contains("not the bytes"),
        "{note}"
    );
    std::fs::write(&path, &original).unwrap();
    ok(p, &["check"]);

    // A symlink among the stored files, which no record names.
    let target = p.join("elsewhere.log");
    std::fs::write(&target, "linked").unwrap();
    let link = p.join("hyp/assets").join(sha256(b"linked"));
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let diagnostics = errors(p);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
    let d0 = &diagnostics[0];
    assert_eq!(d0["code"], "attachment", "{d0}");
    assert_eq!(d0["path"], format!("hyp/assets/{}", sha256(b"linked")));
    std::fs::remove_file(&link).unwrap();
    ok(p, &["check"]);
    // Entries hyp does not name are not its business.
    std::os::unix::fs::symlink(&target, p.join("hyp/assets/README")).unwrap();
    ok(p, &["check"]);
    // A data record whose stored file is a symlink: reported once, for it.
    let sha = sha256(b"twelve bytes");
    let blob = p.join("hyp/assets").join(&sha);
    let moved = p.join("moved.log");
    std::fs::rename(&blob, &moved).unwrap();
    std::os::unix::fs::symlink(&moved, &blob).unwrap();
    let diagnostics = errors(p);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
    assert_eq!(diagnostics[0]["path"], format!("hyp/data/{d}.md"));
    assert_eq!(diagnostics[0]["code"], "attachment");
    std::fs::remove_file(&blob).unwrap();
    std::fs::rename(&moved, &blob).unwrap();
    ok(p, &["check"]);

    // Duplicate data references.
    let batch = serde_json::json!([{"op": "create", "record": {"kind": "evidence",
        "title": "t", "source": "s", "data": [d, d]}}]);
    let out = with_stdin(p, &["--json", "apply"], batch.to_string().as_bytes());
    assert_eq!(out.status.code(), Some(1), "{}", stderr_of(&out));
    let message = json_error(&out)["error"].as_str().unwrap().to_string();
    assert!(
        message.contains(&format!("data names {d} twice")),
        "{message}"
    );
}

/// An empty capture fails unless asked for; `data get --output` refuses the
/// notebook's own directory and replaces a file whole.
#[test]
fn empty_captures_and_output_inside_the_notebook_are_refused() {
    let project = demo();
    let p = project.path();
    let out = with_stdin(
        p,
        &["--json", "capture", "-", "--origin", "failing | cmd"],
        b"",
    );
    assert_eq!(out.status.code(), Some(1));
    let message = json_error(&out)["error"].as_str().unwrap().to_string();
    assert!(
        message.contains("empty") && message.contains("--allow-empty"),
        "{message}"
    );
    assert_eq!(schema_of(p), 1, "nothing written");
    let out = with_stdin(
        p,
        &["capture", "-", "--origin", "true", "--allow-empty"],
        b"",
    );
    assert!(out.status.success(), "{}", stderr_of(&out));
    let empty = id_of(String::from_utf8(out.stdout).unwrap());
    assert_eq!(record(p, &empty)["size"], 0);

    let file = p.join("x.log");
    std::fs::write(&file, "some bytes").unwrap();
    let d = id_of(ok(p, &["capture", file.to_str().unwrap(), "--origin", "o"]));
    let before = files(p);
    let e = first(p, "evidence");
    for (target, own) in [
        (p.join("hyp/evidence").join(format!("{e}.md")), "hyp/"),
        (p.join("hyp/new.bin"), "hyp/"),
        (p.join("hyp/assets/../data/x"), "hyp/"),
        // The journal: hyp would replay the bytes as writes.
        (p.join(".hyp/transaction.json"), ".hyp/"),
        (p.join(".hyp/x"), ".hyp/"),
    ] {
        let out = run(
            p,
            &["data", "get", &d, "--output", target.to_str().unwrap()],
        );
        assert_eq!(out.status.code(), Some(1), "{}", target.display());
        let expected = format!("inside the notebook's {own} directory");
        assert!(stderr_of(&out).contains(&expected), "{}", stderr_of(&out));
    }
    assert!(files(p) == before, "nothing in hyp/ changed");
    assert!(!p.join(".hyp/transaction.json").exists());
    assert!(!p.join(".hyp/x").exists());
    let copy = p.join("copy.log");
    std::fs::write(&copy, "a much longer older content that must go").unwrap();
    ok(p, &["data", "get", &d, "--output", copy.to_str().unwrap()]);
    assert_eq!(read(&copy), "some bytes");
    let leftovers: Vec<_> = std::fs::read_dir(p)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| n.starts_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

/// After a merge brings an attachment into a schema-3 notebook, the next
/// write converts it, reusing the derived record, and says so without
/// claiming a schema raise.
#[test]
fn a_merged_attachment_is_converted_by_the_next_write_and_named() {
    use hyp::{model::*, store};
    let l = legacy();
    let p = l.dir.path();
    let file = p.join("new.txt");
    std::fs::write(&file, "trigger").unwrap();
    ok(p, &["capture", file.to_str().unwrap(), "--origin", "here"]);
    assert_eq!(schema_of(p), 3);
    // An older branch attached the same bytes to a third observation.
    let out = ok(p, &["observe", "Third", "--source", "rig"]);
    let e3 = out.trim().to_string();
    let path = p.join("hyp/evidence").join(format!("{e3}.md"));
    let mut r = store::decode(&read(&path)).unwrap();
    if let Data::Evidence { attachments, .. } = &mut r.data {
        attachments.push(Attachment {
            path: format!("assets/{}", l.shas[0]),
            sha256: l.shas[0].clone(),
        });
    }
    std::fs::write(&path, store::encode(&r).unwrap()).unwrap();
    let out = run(p, &["add", "Unrelated"]);
    assert!(out.status.success(), "{}", stderr_of(&out));
    let stderr = stderr_of(&out);
    assert!(
        stderr.contains(&format!("converted the attachments of evidence {e3}")),
        "{stderr}"
    );
    assert!(!stderr.contains("raised"), "{stderr}");
    assert!(!stderr.contains("created data records"), "{stderr}");
    assert_eq!(
        record(p, &e3)["data"],
        serde_json::json!([hyp::store::migrated_id(&l.shas[0])])
    );
    ok(p, &["check"]);
}

/// The migration is one journaled write: a crash after the journal was
/// saved leaves the old files, and the next hyp rolls it forward to exactly
/// what the uninterrupted write produced.
#[test]
fn an_interrupted_migration_rolls_forward() {
    let l = legacy();
    let p = l.dir.path();
    // A second copy of the same notebook, before the migration.
    let crashed = TempDir::new().unwrap();
    let c = crashed.path();
    for (path, bytes) in files(p) {
        let target = c.join(&path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, bytes).unwrap();
    }
    let before = files(c);
    let file = p.join("new.txt");
    std::fs::write(&file, "unrelated capture").unwrap();
    ok(p, &["capture", file.to_str().unwrap(), "--origin", "here"]);
    let done = files(p);
    // What the journal of that write holds: every record file and the config
    // it changed. The captured file itself is stored before the journal.
    let mut journal = serde_json::Map::new();
    for (path, bytes) in &done {
        let relative = path.strip_prefix("hyp").unwrap();
        if before.get(path) == Some(bytes) {
            continue;
        }
        if relative.starts_with("assets") {
            let target = c.join(path);
            std::fs::write(target, bytes).unwrap();
            continue;
        }
        let text = String::from_utf8(bytes.clone()).unwrap();
        journal.insert(relative.to_str().unwrap().to_string(), text.into());
    }
    assert!(journal.contains_key("config.toml"), "{:?}", journal.keys());
    assert!(journal.keys().any(|k| k.starts_with("data/")));
    assert!(journal.keys().any(|k| k.starts_with("evidence/")));
    std::fs::create_dir_all(c.join(".hyp")).unwrap();
    std::fs::write(
        c.join(".hyp/transaction.json"),
        serde_json::Value::Object(journal).to_string(),
    )
    .unwrap();
    assert_eq!(schema_of(c), 2, "crashed before applying");
    ok(c, &["list"]);
    assert!(!c.join(".hyp/transaction.json").exists());
    assert!(files(c) == done, "rolled forward to the same files");
    ok(c, &["check"]);
}

/// Attaching bytes the evidence holds already changes nothing: as a data
/// reference, or as an attachment of hyp 0.2.0 (no data record yet, and no
/// migration either).
#[test]
fn attaching_bytes_the_evidence_holds_already_is_a_no_op() {
    let l = legacy();
    let p = l.dir.path();
    let e1 = &l.evidence[0];
    let file = p.join("same.txt");
    std::fs::write(&file, "timeout at transfer 8,142\n").unwrap();
    let before = files(p);
    let out = run(p, &["evidence", "attach", e1, file.to_str().unwrap()]);
    assert!(out.status.success(), "{}", stderr_of(&out));
    assert_eq!(
        String::from_utf8(out.stdout.clone()).unwrap(),
        format!("{e1}\n")
    );
    assert!(
        stderr_of(&out).contains("no changes"),
        "{}",
        stderr_of(&out)
    );
    assert!(files(p) == before, "nothing written");
    assert_eq!(schema_of(p), 2);

    // Migrated, the bytes are a data reference: still a no-op.
    let other = p.join("other.txt");
    std::fs::write(&other, "trigger").unwrap();
    ok(p, &["capture", other.to_str().unwrap(), "--origin", "here"]);
    let evidence_file = p.join("hyp/evidence").join(format!("{e1}.md"));
    let stored = read(&evidence_file);
    let out = run(p, &["evidence", "attach", e1, file.to_str().unwrap()]);
    assert!(out.status.success(), "{}", stderr_of(&out));
    let derived = hyp::store::migrated_id(&l.shas[0]);
    assert_eq!(
        String::from_utf8(out.stdout.clone()).unwrap(),
        format!("{e1}\n{derived}\n")
    );
    assert!(
        stderr_of(&out).contains("no changes"),
        "{}",
        stderr_of(&out)
    );
    assert_eq!(read(&evidence_file), stored, "not rewritten");
    assert_eq!(record(p, e1)["updated_at"], l.updated[0]);
}
