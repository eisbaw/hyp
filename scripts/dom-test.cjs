/* DOM/API integration test in jsdom against the real server and CLI. Run by `just e2e` and the e2e-dom flake check. Complements, not replaces, renderer testing. */
const { JSDOM, VirtualConsole } = require("jsdom");
const { spawn, execFileSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const assert = require("node:assert/strict");
const bin =
  process.env.HYP_BIN || path.resolve(__dirname, "../target/debug/hyp");
const root = fs.mkdtempSync(path.join(os.tmpdir(), "hyp-dom-"));
const cli = (...args) =>
  execFileSync(bin, ["--project", root, ...args], { encoding: "utf8" }).trim();
let server;
const windows = [],
  streams = [],
  errors = [];
const wait = async (fn, label) => {
  const end = Date.now() + 10000;
  while (Date.now() < end) {
    if (fn()) return;
    await new Promise((r) => setTimeout(r, 25));
  }
  throw new Error("Timed out: " + label);
};
const virtualConsole = new VirtualConsole();
virtualConsole.on("jsdomError", (e) => errors.push(e.message));
function options(url) {
  return {
    runScripts: "dangerously",
    resources: "usable",
    pretendToBeVisual: true,
    virtualConsole,
    beforeParse(w) {
      w.fetch = (u, o) => fetch(new URL(u, url), o);
      w.structuredClone = structuredClone;
      w.confirm = () => true;
      w.HTMLDialogElement.prototype.showModal = function () {
        this.setAttribute("open", "");
      };
      w.HTMLDialogElement.prototype.close = function () {
        this.removeAttribute("open");
      };
      w.EventSource = class {
        constructor(u) {
          this.listeners = {};
          this.abort = new AbortController();
          streams.push(this);
          this.start(new URL(u, url));
        }
        addEventListener(name, cb) {
          this.listeners[name] = cb;
        }
        async start(u) {
          try {
            const r = await fetch(u, { signal: this.abort.signal });
            this.onopen?.();
            const reader = r.body.getReader();
            const dec = new TextDecoder();
            let text = "";
            while (true) {
              const { value, done } = await reader.read();
              if (done) break;
              text += dec.decode(value, { stream: true });
              let i;
              while ((i = text.indexOf("\n\n")) >= 0) {
                const event = text.slice(0, i);
                text = text.slice(i + 2);
                if (event.includes("event: change"))
                  this.listeners.change?.({ data: event });
              }
            }
          } catch (e) {
            if (e.name !== "AbortError") this.onerror?.(e);
          }
        }
        close() {
          this.abort.abort();
        }
      };
    },
  };
}
function field(w, name, value) {
  const el = w.document.querySelector(`[name="${name}"]`);
  assert.ok(el, "field " + name);
  el.value = value;
  el.dispatchEvent(new w.Event("input", { bubbles: true }));
  el.dispatchEvent(new w.Event("change", { bubbles: true }));
}
function click(w, selector) {
  const el = w.document.querySelector(selector);
  assert.ok(el, "element " + selector);
  el.click();
}
const formError = (w) => w.document.querySelector("#form-error").textContent;
const DRAFT_HINT = "Copy your draft before closing";
/** Opens the editor through `open`, lets `concurrent` write through the CLI
 * while the draft is dirty, and waits until the form has noticed it. */
async function editDuring(w, open, fill, concurrent) {
  open();
  fill();
  concurrent();
  await wait(
    () =>
      w.document
        .querySelector("#notice")
        .textContent.includes("while you were editing"),
    "dirty form notified of a concurrent write",
  );
}
/** Submits with the next transaction answered by `status` and `error`,
 * as a way to check how the form classifies failures. */
async function submitAnswered(w, status, error) {
  const real = w.fetch;
  w.fetch = async (u, o) =>
    o?.method === "POST"
      ? new Response(JSON.stringify({ error }), { status })
      : real(u, o);
  try {
    click(w, 'button[type="submit"]');
    await wait(() => formError(w).includes(error), "stubbed answer shown");
  } finally {
    w.fetch = real;
  }
  return formError(w);
}
function h1(w) {
  return w.document.querySelector("h1")?.textContent;
}
async function save(w) {
  click(w, 'button[type="submit"]');
  await wait(
    () =>
      !w.document.querySelector("#editor").open ||
      w.document.querySelector("#form-error").textContent,
    "save",
  );
  assert.equal(w.document.querySelector("#form-error").textContent, "");
}
async function main() {
  cli("init", "--demo");
  server = spawn(bin, ["--project", root, "web", "--port", "0"]);
  const url = await new Promise((resolve, reject) => {
    const timer = setTimeout(
      () => reject(new Error("server startup timeout")),
      10000,
    );
    server.stdout.on("data", (d) => {
      const m = String(d).match(/http:\/\/127\.0\.0\.1:\d+/);
      if (m) {
        clearTimeout(timer);
        resolve(m[0]);
      }
    });
    server.on("exit", (code) => reject(new Error("server exited " + code)));
  });
  const dom = await JSDOM.fromURL(url, options(url));
  const w = dom.window;
  windows.push(w);
  await wait(() => h1(w) === "Hypotheses", "overview");
  assert.ok(
    w.document.body.textContent.includes(
      "DMA timeout is caused by cache coherency",
    ),
  );
  click(w, "#new");
  field(w, "title", "GUI hypothesis");
  field(w, "scope", "DOM + actual HTTP server");
  field(w, "body", "Created from a real form");
  await save(w);
  await wait(() => h1(w) === "GUI hypothesis", "created hypothesis");
  const h = JSON.parse(cli("--json", "list", "--kind", "hypothesis")).find(
    (e) => e.record.title === "GUI hypothesis",
  );
  assert.ok(h, "GUI create persisted");
  cli("set", h.record.id, "--title", "Updated by CLI");
  await wait(
    () => h1(w) === "Updated by CLI",
    "CLI update delivered through SSE",
  );
  const dom2 = await JSDOM.fromURL(url, options(url));
  const w2 = dom2.window;
  windows.push(w2);
  await wait(() => h1(w2) === "Hypotheses", "second tab");
  w2.location.hash = "record/" + h.record.id;
  await wait(() => h1(w2) === "Updated by CLI", "second tab detail");
  const filename = path.join(root, "hyp", "hypotheses", h.record.id + ".md");
  fs.writeFileSync(
    filename,
    fs
      .readFileSync(filename, "utf8")
      .replace("Updated by CLI", "Updated by editor"),
  );
  await wait(
    () => h1(w) === "Updated by editor" && h1(w2) === "Updated by editor",
    "editor changes in both tabs",
  );
  click(w, '[data-action="edit"][data-id="' + h.record.id + '"]');
  field(w, "title", "Preserve this draft");
  cli("set", h.record.id, "--title", "Concurrent change");
  await wait(
    () =>
      w.document
        .querySelector("#notice")
        .textContent.includes("while you were editing"),
    "dirty form notification",
  );
  assert.equal(
    w.document.querySelector('[name="title"]').value,
    "Preserve this draft",
  );
  click(w, 'button[type="submit"]');
  await wait(() => formError(w).includes("conflict:"), "stale edit rejected");
  assert.ok(formError(w).includes(DRAFT_HINT), formError(w));
  assert.equal(
    JSON.parse(cli("--json", "show", h.record.id)).entry.record.title,
    "Concurrent change",
  );
  click(w, "#cancel-editor");
  await wait(() => h1(w) === "Concurrent change", "cancel and refresh");
  // Conflict or not is decided by the HTTP status, not by the message text.
  click(w, '[data-action="edit"][data-id="' + h.record.id + '"]');
  assert.ok(
    (await submitAnswered(w, 409, "precondition failed")).includes(DRAFT_HINT),
  );
  assert.ok(
    !(await submitAnswered(w, 422, "conflict: only text")).includes(DRAFT_HINT),
  );
  click(w, "#cancel-editor");
  await wait(() => !w.document.querySelector("#editor").open, "closed");
  // An agent's write to an unrelated record does not reject the form.
  await editDuring(
    w,
    () => click(w, '[data-action="edit"][data-id="' + h.record.id + '"]'),
    () => field(w, "scope", "Saved despite an unrelated write"),
    () => cli("add", "Unrelated hypothesis from an agent"),
  );
  await save(w);
  assert.equal(
    JSON.parse(cli("--json", "show", h.record.id)).entry.record.scope,
    "Saved despite an unrelated write",
  );
  click(w, '[data-action="create:criterion"]');
  field(w, "title", "Reject if output is zero");
  await save(w);
  await wait(() => h1(w) === "Reject if output is zero", "criterion persisted");
  w.location.hash = "record/" + h.record.id;
  await wait(() => h1(w) === "Concurrent change", "return to hypothesis");
  click(w, '[data-action="create:evidence"]');
  field(w, "title", "Output was zero");
  field(w, "source", "capture/run-1.txt");
  field(w, "relation", "contradicts");
  field(w, "reason", "The rejection condition was met");
  await save(w);
  await wait(() => h1(w) === "Output was zero", "evidence");
  const ev = JSON.parse(cli("--json", "list", "--kind", "evidence")).find(
    (e) => e.record.title === "Output was zero",
  );
  assert.ok(ev);
  w.location.hash = "record/" + h.record.id;
  await wait(() => h1(w) === "Concurrent change", "return to assess");
  // Counter-evidence that arrives while an assessment is being written is a
  // conflict: the assessment would otherwise claim to be based on it.
  await editDuring(
    w,
    () => click(w, '[data-action="create:assessment"]'),
    () => {
      field(w, "title", "Judged before the counter-evidence");
      field(w, "body", "Written without seeing the agent's observation.");
      field(w, "judgment", "supported");
      field(w, "evidence", ev.record.id);
    },
    () =>
      cli(
        "evidence",
        "add",
        h.record.id,
        "Agent saw the failure again",
        "--source",
        "agent/run-2.txt",
        "--against",
      ),
  );
  click(w, 'button[type="submit"]');
  await wait(() => formError(w).includes("conflict:"), "stale assessment");
  assert.ok(formError(w).includes(DRAFT_HINT), formError(w));
  assert.equal(
    w.document.querySelector('[name="title"]').value,
    "Judged before the counter-evidence",
  );
  assert.ok(
    !cli("list", "--kind", "assessment").includes(
      "Judged before the counter-evidence",
    ),
  );
  click(w, "#cancel-editor");
  await wait(
    () =>
      !w.document.querySelector("#editor").open &&
      w.document.body.textContent.includes("Agent saw the failure again"),
    "discarded, and the counter-evidence shown for review",
  );
  click(w, '[data-action="create:assessment"]');
  field(w, "title", "Reviewed the output");
  field(w, "body", "The criterion is satisfied by the observation.");
  field(w, "judgment", "falsified");
  field(w, "confidence", "0.05");
  field(w, "evidence", ev.record.id);
  const f = JSON.parse(cli("--json", "list", "--kind", "criterion")).find(
    (e) => e.record.title === "Reject if output is zero",
  );
  field(w, "criterion", f.record.id);
  await save(w);
  await wait(() => h1(w) === "Reviewed the output", "assessment");
  const assessed = JSON.parse(cli("--json", "show", h.record.id)).state;
  assert.equal(assessed.judgment, "falsified");
  assert.equal(assessed.needs_review, false);
  // Every judgment except untested needs evidence (422, not a conflict), and
  // the form says so before the author tries.
  w.location.hash = "record/" + h.record.id;
  await wait(() => h1(w) === "Concurrent change", "return to assess again");
  click(w, '[data-action="create:assessment"]');
  assert.ok(
    w.document
      .querySelector("#editor-fields")
      .textContent.includes("Required for every judgment except untested"),
  );
  field(w, "title", "Supported without evidence");
  field(w, "body", "No observation cited.");
  field(w, "judgment", "supported");
  click(w, 'button[type="submit"]');
  await wait(
    () => formError(w).includes("must cite evidence"),
    "evidence required",
  );
  assert.ok(!formError(w).includes(DRAFT_HINT), formError(w));
  // Only evidence already linked to this hypothesis may be cited; the error
  // names the command that links it.
  const demoEvidence = JSON.parse(
    cli("--json", "list", "--kind", "evidence"),
  ).find((e) => e.record.title.startsWith("Timeout reproduced"));
  field(w, "evidence", demoEvidence.record.id);
  click(w, 'button[type="submit"]');
  await wait(
    () => formError(w).includes("hyp link " + demoEvidence.record.id),
    "unlinked evidence rejected",
  );
  assert.ok(!formError(w).includes(DRAFT_HINT), formError(w));
  assert.ok(
    !cli("list", "--kind", "assessment").includes("Supported without evidence"),
  );
  click(w, "#cancel-editor");
  await wait(() => !w.document.querySelector("#editor").open, "closed");
  cli(
    "link",
    demoEvidence.record.id,
    h.record.id,
    "--relation",
    "supports",
    "--reason",
    "The demo observation matters here too",
  );
  await wait(
    () =>
      w.document.body.textContent.includes(
        "The demo observation matters here too",
      ),
    "new link shown for review",
  );
  click(w, '[data-action="create:assessment"]');
  field(w, "title", "Cites shared evidence");
  field(w, "body", "The demo observation matters here too.");
  field(w, "judgment", "weakened");
  field(w, "evidence", demoEvidence.record.id);
  await save(w);
  await wait(() => h1(w) === "Cites shared evidence", "shared evidence");
  assert.equal(
    JSON.parse(cli("--json", "show", h.record.id)).state.needs_review,
    false,
  );
  // Experiments and runs freeze what the form showed, stated by revision.
  w.location.hash = "record/" + h.record.id;
  await wait(() => h1(w) === "Concurrent change", "return to plan");
  click(w, '[data-action="create:experiment"]');
  field(w, "title", "Repeat with output capture");
  field(w, "targets", f.record.id);
  field(w, "body", "Capture the output of 100 runs.");
  await save(w);
  await wait(() => h1(w) === "Repeat with output capture", "experiment");
  const x = JSON.parse(cli("--json", "list", "--kind", "experiment")).find(
    (e) => e.record.title === "Repeat with output capture",
  );
  assert.deepEqual(
    x.record.targets.map((t) => t.id),
    [h.record.id, f.record.id],
  );
  click(w, '[data-action="create:run"]');
  field(w, "title", "First capture");
  field(w, "outcome", "observed");
  field(w, "evidence", ev.record.id);
  await save(w);
  await wait(() => h1(w) === "First capture", "run");
  const run = JSON.parse(cli("--json", "list", "--kind", "run")).find(
    (e) => e.record.title === "First capture",
  );
  assert.equal(run.record.plan.revision, x.revision);
  assert.deepEqual(run.record.evidence, [ev.record.id]);
  for (const [view, title] of [
    ["matrix", "Evidence matrix"],
    ["graph", "Relationships"],
    ["experiments", "Experiments"],
    ["evidence", "Evidence"],
    ["all", "All records"],
  ]) {
    w.location.hash = view;
    await wait(() => h1(w) === title, "view " + view);
  }
  const original = fs.readFileSync(filename, "utf8");
  fs.writeFileSync(filename, "broken");
  await wait(
    () =>
      w.document
        .querySelector("#notice")
        .textContent.includes("Invalid project files"),
    "invalid file diagnostics",
  );
  fs.writeFileSync(filename, original);
  await wait(() => w.document.querySelector("#notice").hidden, "recovery");
  const hypDir = path.join(root, "hyp");
  fs.renameSync(hypDir, hypDir + ".moved");
  await wait(
    () =>
      w.document
        .querySelector("#connection")
        .textContent.startsWith("Unavailable") &&
      w.document
        .querySelector("#notice")
        .textContent.includes(path.basename(root) + "/hyp"),
    "unavailable project explained with its cause",
  );
  fs.renameSync(hypDir + ".moved", hypDir);
  await wait(
    () =>
      w.document.querySelector("#notice").hidden &&
      w.document.querySelector("#connection").textContent === "Live",
    "recovery from unavailable project",
  );
  const html = cli("export", "--format", "html");
  const report = new JSDOM(html, {
    url: "file:///hyp-report.html",
    ...options("file:///hyp-report.html"),
  });
  windows.push(report.window);
  await wait(() => h1(report.window) === "Hypotheses", "offline report");
  assert.ok(report.window.document.body.classList.contains("read-only"));
  assert.ok(
    report.window.document.body.textContent.includes("Concurrent change"),
  );
  assert.deepEqual(errors, []);
  console.log(
    "PASS: DOM forms, real HTTP writes, CLI↔UI SSE updates, editor changes, two tabs, dirty-form preservation, conflicts by status, saves despite unrelated writes, stale assessment rejected, evidence required and linked-only, criteria, evidence interpretations, falsification assessment, experiment and run, all views, malformed-file recovery, unavailable-project cause and offline export.",
  );
}
main()
  .catch((e) => {
    console.error(e);
    process.exitCode = 1;
  })
  .finally(async () => {
    for (const s of streams) s.close();
    for (const w of windows) w.close();
    if (server) {
      server.kill("SIGTERM");
      await new Promise((r) => server.once("exit", r));
    }
    fs.rmSync(root, { recursive: true, force: true });
  });
