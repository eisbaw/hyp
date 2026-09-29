/* Optional DOM/API integration test. This complements, not replaces, renderer testing. */
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
  await wait(
    () =>
      w.document.querySelector("#form-error").textContent.includes("conflict:"),
    "stale edit rejected",
  );
  assert.equal(
    JSON.parse(cli("--json", "show", h.record.id)).entry.record.title,
    "Concurrent change",
  );
  click(w, "#cancel-editor");
  await wait(() => h1(w) === "Concurrent change", "cancel and refresh");
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
  assert.equal(
    JSON.parse(cli("--json", "show", h.record.id)).state.judgment,
    "falsified",
  );
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
    "PASS: DOM forms, real HTTP writes, CLI↔UI SSE updates, editor changes, two tabs, dirty-form preservation, conflicts, criteria, evidence interpretations, falsification assessment, all views, malformed-file recovery and offline export.",
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
