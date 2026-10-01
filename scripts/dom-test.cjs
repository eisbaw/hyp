/* DOM/API integration test in jsdom against the real server and CLI. Run by `just e2e` and the e2e-dom flake check. Complements, not replaces, renderer testing. */
const { JSDOM, VirtualConsole } = require("jsdom");
const { spawn, execFileSync } = require("node:child_process");
const crypto = require("node:crypto");
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
// While true, the test's EventSource drops the server's change events.
let muted = false;
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
                if (event.includes("event: change") && !muted)
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
const listed = (kind) => JSON.parse(cli("--json", "list", "--kind", kind));
/** The demo record of `kind` whose title starts with `title`. */
function demo(kind, title) {
  const e = listed(kind).find((e) => e.record.title.startsWith(title));
  assert.ok(e, `demo ${kind} ${title}`);
  return e.record;
}
async function open(w, id, title) {
  w.location.hash = "record/" + id;
  await wait(() => h1(w) === title, "page of " + title);
}
const main$ = (w) => w.document.querySelector("main");
const section$ = (w, name) =>
  w.document.querySelector(`[data-section="${name}"]`);
const href = (id) => `a[href="#record/${id}"]`;
/** HYPO-0041: a relationship card names the other end of the link on the
 * page of either end, never the page's own hypothesis. */
async function relationshipEnds(w) {
  const cache = demo("hypothesis", "DMA timeout is caused by cache");
  const bus = demo("hypothesis", "DMA timeout is caused by bus");
  await open(w, bus.id, bus.title);
  const to = w.document.querySelector('[data-direction="to"]');
  assert.ok(to, "incoming relationship on the 'to' page");
  assert.ok(to.querySelector(href(cache.id)), to.innerHTML);
  assert.equal(to.querySelector(href(bus.id)), null, "names itself");
  assert.equal(to.textContent, `${cache.title} competes-with this hypothesis`);
  await open(w, cache.id, cache.title);
  const from = w.document.querySelector('[data-direction="from"]');
  assert.ok(from.querySelector(href(bus.id)), from.innerHTML);
  assert.equal(from.textContent, `This hypothesis competes-with ${bus.title}`);
}
/** HYPO-0037 and HYPO-0045: runs on the hypothesis page; frozen targets,
 * status, outcome, roles and directions on the record pages. */
async function recordPages(w) {
  const cache = demo("hypothesis", "DMA timeout is caused by cache");
  const bus = demo("hypothesis", "DMA timeout is caused by bus");
  const x = demo("experiment", "Run 10,000 transfers");
  const run = demo("run", "Cache-disabled run");
  const ev = demo("evidence", "Timeout reproduced");
  const p = demo("prediction", "Clean + invalidate");
  await open(w, cache.id, cache.title);
  const runLine = w.document.querySelector(`[data-run="${run.id}"]`);
  assert.ok(runLine, "the experiment's run on the hypothesis page");
  assert.ok(runLine.textContent.includes("observed"), runLine.textContent);
  assert.ok(runLine.querySelector(href(ev.id)), "the run's cited evidence");
  // Every frozen target; a target edited since is marked.
  cli("set", p.id, "--title", "Clean + invalidate removes every failure");
  await open(w, x.id, x.title);
  await wait(
    () =>
      section$(w, "targets")?.querySelector(`[data-target="${p.id}"]`)
        ?.dataset.changed === "true",
    "changed target marked",
  );
  const targets = [...section$(w, "targets").querySelectorAll("[data-target]")];
  assert.deepEqual(
    targets.map((t) => [t.dataset.target, t.dataset.changed]),
    x.targets.map((t) => [t.id, String(t.id === p.id)]),
  );
  const changed = section$(w, "targets").querySelector(
    `[data-target="${p.id}"]`,
  ).textContent;
  for (const part of ["changed since frozen", "Frozen as: Clean + invalidate eliminates failures"])
    assert.ok(changed.includes(part), part + " in " + changed);
  const badges = () => w.document.querySelector(".page-title .badges").textContent;
  assert.ok(badges().includes("completed"), "experiment status: " + badges());
  const page = () => main$(w).textContent;
  assert.ok(page().includes("Hypothesis under test"), page());
  assert.ok(!page().includes("Referenced records"));
  // The structured record leaves out the frozen copies shown above.
  const raw = w.document.querySelector("details.section pre.raw").textContent;
  assert.ok(!raw.includes("untestable_reason"), raw);
  await open(w, run.id, run.title);
  assert.ok(badges().includes("observed"), "run outcome: " + badges());
  for (const part of ["Frozen plan", "Plan: the experiment it ran", "Cited evidence", "Runs are history"])
    assert.ok(page().includes(part), part + " in " + page());
  // A link page shows its direction.
  const competes = listed("link").find(
    (e) => e.record.relation === "competes_with",
  ).record;
  await open(w, competes.id, competes.title);
  const dir = w.document.querySelector(".direction");
  assert.ok(dir, "direction panel");
  assert.ok(dir.textContent.includes("FROM") && dir.textContent.includes("competes-with"));
  const ends = [...dir.querySelectorAll(".end a")].map((a) => a.getAttribute("href"));
  assert.deepEqual(ends, [`#record/${cache.id}`, `#record/${bus.id}`]);
  // Evidence cards show their interpretations; experiment cards their hypothesis.
  w.location.hash = "evidence";
  await wait(() => h1(w) === "Evidence", "evidence view");
  const evCard = [...w.document.querySelectorAll(".card")].find((c) =>
    c.textContent.includes(ev.title),
  );
  const lines = [...evCard.querySelectorAll(".interpretations li")].map(
    (li) => li.textContent,
  );
  assert.deepEqual(lines.sort(), [
    `against ${cache.title} contradicts H`,
    `qualifies ${bus.title} qualifies H`,
  ]);
  w.location.hash = "experiments";
  await wait(() => h1(w) === "Experiments", "experiments view");
  assert.ok(main$(w).textContent.includes("Tests " + cache.title));
  // Editing an interpretation keeps its ends, in the form and in JSON.
  const interpretation = listed("link").find(
    (e) => e.record.from === ev.id && e.record.to === cache.id,
  );
  await open(w, ev.id, ev.title);
  click(w, `[data-section="bears-on"] [data-action="edit"][data-id="${interpretation.record.id}"]`);
  assert.equal(w.document.querySelector("#editor-title").textContent, "Edit interpretation");
  assert.equal(w.document.querySelector('[name="from"]'), null);
  assert.equal(w.document.querySelector('[name="to"]'), null);
  field(w, "body", "Edited without touching its ends");
  await save(w);
  await wait(() => h1(w) === interpretation.record.title, "link page after edit");
  const edited = JSON.parse(cli("--json", "show", interpretation.record.id)).entry.record;
  assert.deepEqual(
    [edited.from, edited.to, edited.body],
    [ev.id, cache.id, "Edited without touching its ends"],
  );
  click(w, `[data-action="edit"][data-id="${interpretation.record.id}"]`);
  field(w, "advanced", JSON.stringify({ ...edited, to: bus.id }, null, 2));
  click(w, 'button[type="submit"]');
  await wait(() => formError(w).includes("ends of a link cannot change"), "ends kept");
  click(w, "#cancel-editor");
  assert.equal(JSON.parse(cli("--json", "show", edited.id)).entry.record.to, cache.id);
}
/** HYPO-0092: unexplained observations on the overview, what an observation
 * bears on, and a hypothesis created to explain one, linked in one write. */
async function observations(w) {
  const ev = demo("evidence", "Timeout reproduced");
  const cache = demo("hypothesis", "DMA timeout is caused by cache");
  const statusSet = () =>
    JSON.parse(cli("--json", "status")).unexplained_observations.map((o) => o.id);
  const overviewSet = () =>
    [...w.document.querySelectorAll("#unexplained .detail-item h3 a")].map((a) =>
      a.getAttribute("href").replace("#record/", ""),
    );
  const blip = cli("observe", "Unexplained bus blip", "--source", "scope capture 7");
  w.location.hash = "overview";
  await wait(
    () => h1(w) === "Hypotheses" && overviewSet().includes(blip),
    "unexplained observation on the overview",
  );
  assert.deepEqual(overviewSet(), statusSet());
  // What the demo observation bears on, and how.
  await open(w, ev.id, ev.title);
  const bears = section$(w, "bears-on");
  assert.ok(bears.querySelector(href(cache.id)), bears.innerHTML);
  assert.ok(bears.textContent.includes("against") && bears.textContent.includes("qualifies"));
  assert.ok(!bears.textContent.includes("No live hypothesis accounts"));
  await open(w, blip, "Unexplained bus blip");
  assert.ok(
    section$(w, "bears-on").textContent.includes("No live hypothesis accounts for this observation"),
  );
  click(w, `[data-section="bears-on"] [data-action="create:hypothesis"][data-id="${blip}"]`);
  assert.ok(w.document.querySelector(`[data-explains="${blip}"]`));
  field(w, "title", "USB bursts starve the DMA");
  field(w, "reason", "A blip on the scope matches a USB burst");
  await save(w);
  await wait(() => h1(w) === "USB bursts starve the DMA", "explaining hypothesis");
  const h = demo("hypothesis", "USB bursts starve the DMA");
  const links = listed("link").filter((e) => e.record.from === blip);
  assert.deepEqual(
    links.map((e) => [e.record.to, e.record.relation, e.record.body]),
    [[h.id, "supports", "A blip on the scope matches a USB burst"]],
  );
  await open(w, blip, "Unexplained bus blip");
  const now = section$(w, "bears-on");
  assert.ok(now.querySelector(href(h.id)) && now.textContent.includes("for"));
  assert.ok(!now.textContent.includes("No live hypothesis accounts"));
  w.location.hash = "overview";
  await wait(() => h1(w) === "Hypotheses", "overview again");
  assert.ok(!overviewSet().includes(blip));
  // An observation deleted while its hypothesis is being written: the save
  // fails rather than create the hypothesis without its link.
  const gone = cli("observe", "Deleted before the save", "--source", "scope");
  await open(w, gone, "Deleted before the save");
  click(w, `[data-section="bears-on"] [data-action="create:hypothesis"]`);
  // Deleted while the form is still untouched, so the page takes the new
  // snapshot (the page behind the form says so) before the author types.
  cli("archive", gone);
  cli("delete", gone);
  await wait(
    () => main$(w).textContent.includes("Record not found"),
    "page refreshed behind the form",
  );
  field(w, "title", "Explains an observation that is gone");
  click(w, 'button[type="submit"]');
  await wait(() => formError(w).includes(gone), "save rejected: " + formError(w));
  assert.ok(!listed("hypothesis").some((e) => e.record.title.startsWith("Explains an observation")));
  click(w, "#cancel-editor");
  await wait(() => !w.document.querySelector("#editor").open, "closed");
  assert.deepEqual(overviewSet(), statusSet());
}
/** HYPO-0042: a title made of one long path wraps instead of widening the
 * page. jsdom has no layout, so this checks the rule that wraps it applies
 * to the title on the overview card and the record page. */
async function longTitles(w) {
  const title =
    "/nix/store/0r1whf0bmjajadz5xils4bpzz84nckv3-firmware-0.8/lib/dma/controllers/stm32h7/channel_timeout_regression.c";
  const id = cli("add", title);
  await open(w, id, title);
  const wraps = (el) => w.getComputedStyle(el).overflowWrap;
  assert.equal(wraps(w.document.querySelector("h1")), "anywhere");
  // Buttons and badges are words: they must not break ("Edi/t") beside it.
  for (const selector of [
    `[data-action="edit"][data-id="${id}"]`,
    ".page-title .badge",
    ".meta-line span",
  ])
    assert.equal(wraps(w.document.querySelector(selector)), "normal", selector);
  w.location.hash = "overview";
  await wait(
    () => h1(w) === "Hypotheses" && main$(w).textContent.includes(title),
    "long title card",
  );
  const cardTitle = [...w.document.querySelectorAll(".card h3 a")].find(
    (a) => a.textContent === title,
  );
  assert.equal(wraps(cardTitle), "anywhere");
  assert.equal(wraps(w.document.querySelector(".card .id")), "anywhere");
}
/** An observation whose time is unknown (`observed_at` empty) stays so when
 * the WebUI edits something else: only a new record defaults to now. */
async function unknownObservedAt(w) {
  w.location.hash = "evidence";
  await wait(() => h1(w) === "Evidence", "evidence view");
  click(w, '[data-action="create:evidence"]');
  assert.ok(w.document.querySelector('[name="observed_at"]').value, "new: now");
  field(w, "title", "Seen at an unknown time");
  field(w, "source", "memory");
  field(w, "observed_at", "");
  await save(w);
  await wait(() => h1(w) === "Seen at an unknown time", "evidence saved");
  const e = demo("evidence", "Seen at an unknown time");
  assert.equal(e.observed_at, "");
  click(w, `[data-action="edit"][data-id="${e.id}"]`);
  assert.equal(w.document.querySelector('[name="observed_at"]').value, "");
  field(w, "title", "Seen at a time nobody noted");
  await save(w);
  await wait(() => h1(w) === "Seen at a time nobody noted", "evidence edited");
  assert.equal(demo("evidence", "Seen at a time nobody noted").observed_at, "");
}
/** The backstop re-read (`retrySoon` in app.js): the page's snapshot reads
 * are answered with `answer` (given the real read) until it shows `shown`;
 * then reads work again while the server's events are dropped, so only the
 * page's own re-read can recover it. */
async function recoversWithoutEvents(w, answer, shown) {
  // A re-read still pending from an earlier failure could recover the page
  // instead of the one this failure schedules.
  await wait(() => w.eval("retry === null"), "no re-read pending");
  const real = w.fetch;
  w.fetch = async (u, o) =>
    String(u).includes("/api/snapshot") ? answer(() => real(u, o)) : real(u, o);
  try {
    cli("add", "Write that makes the page read: " + shown);
    await wait(
      () => w.document.querySelector("#notice").textContent.includes(shown),
      shown + " shown",
    );
    muted = true;
  } finally {
    w.fetch = real;
  }
  try {
    await wait(
      () =>
        w.document.querySelector("#notice").hidden &&
        w.document.querySelector("#connection").textContent === "Live",
      "recovery without a server event after " + shown,
    );
  } finally {
    muted = false;
  }
}
/** A read answered after a save must not replace the save's newer
 * snapshot (`adopt` in app.js). The read is held until the save is done;
 * events are dropped meanwhile, so no later read hides the effect. */
async function olderReadAfterSave(w) {
  const real = w.fetch;
  let release,
    held = 0,
    consumed = 0;
  const gate = new Promise((r) => (release = r));
  w.fetch = async (u, o) => {
    if (!String(u).includes("/api/snapshot")) return real(u, o);
    const res = await real(u, o);
    const body = await res.text();
    held += 1;
    await gate;
    const answer = new Response(body, { status: res.status });
    const json = answer.json.bind(answer);
    answer.json = async () => {
      const value = await json();
      consumed += 1;
      return value;
    };
    return answer;
  };
  try {
    cli("add", "Write whose read is held back");
    await wait(() => held > 0, "a read held back");
    muted = true;
    w.fetch = real;
    click(w, "#new");
    field(w, "title", "Saved while an older read was on its way");
    await save(w);
    await wait(
      () => h1(w) === "Saved while an older read was on its way",
      "saved record shown",
    );
    release();
    // The page has decoded every held answer; one more task lets it act.
    await wait(() => consumed === held, "held answers taken by the page");
    await new Promise((r) => setTimeout(r, 0));
    assert.equal(h1(w), "Saved while an older read was on its way");
  } finally {
    w.fetch = real;
    release();
    muted = false;
  }
}
/** A read issued while a save is on its way may carry newer state (here a
 * CLI write to the saved record): the save's answer must not replace it
 * (`write` in app.js). The save's answer is held until a read has returned
 * the newer state; events are then dropped, so no later read can hide what
 * the page did with the save's answer. */
async function newerReadDuringSave(w) {
  const real = w.fetch;
  const renamed = "Renamed while the save was answered";
  let release,
    held = 0,
    newer = 0;
  const gate = new Promise((r) => (release = r));
  w.fetch = async (u, o) => {
    if (String(u).includes("/api/snapshot")) {
      const res = await real(u, o);
      const body = await res.text();
      if (body.includes(renamed)) newer += 1;
      return new Response(body, { status: res.status });
    }
    if (!String(u).includes("/api/transaction")) return real(u, o);
    const res = await real(u, o);
    const body = await res.text();
    held += 1;
    await gate;
    return new Response(body, { status: res.status });
  };
  try {
    click(w, "#new");
    field(w, "title", "Saved before a newer write");
    click(w, 'button[type="submit"]');
    await wait(() => held > 0, "the save's answer held back");
    const saved = demo("hypothesis", "Saved before a newer write");
    cli("set", saved.id, "--title", renamed);
    await wait(() => newer > 0, "a read returned the newer state");
    muted = true;
    release();
    await wait(() => h1(w) === renamed, "the newer state wins over the save's answer");
  } finally {
    w.fetch = real;
    release();
    muted = false;
  }
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
  await relationshipEnds(w);
  await recordPages(w);
  await observations(w);
  await longTitles(w);
  await unknownObservedAt(w);
  w.location.hash = "overview";
  await wait(() => h1(w) === "Hypotheses", "back to the overview");
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
  const f = JSON.parse(cli("--json", "list", "--kind", "criterion")).find(
    (e) => e.record.title === "Reject if output is zero",
  );
  const falsify = () => {
    click(w, '[data-action="create:assessment"]');
    field(w, "title", "Reviewed the output");
    field(w, "body", "The criterion is satisfied by the observation.");
    field(w, "judgment", "falsified");
    field(w, "confidence", "0.05");
    field(w, "evidence", ev.record.id);
    field(w, "criterion", f.record.id);
  };
  // The observation contradicts the hypothesis but was not recorded as
  // meeting the criterion: falsified is input to fix (422), not a conflict.
  falsify();
  assert.ok(
    w.document
      .querySelector("#editor-fields")
      .textContent.includes("one cited observation must meet it"),
  );
  click(w, 'button[type="submit"]');
  await wait(
    () => formError(w).includes("must cite evidence that meets its criterion"),
    "falsified without evidence meeting the criterion",
  );
  assert.ok(!formError(w).includes(DRAFT_HINT), formError(w));
  click(w, "#cancel-editor");
  cli(
    "link",
    ev.record.id,
    f.record.id,
    "--relation",
    "supports",
    "--reason",
    "Zero output is what the criterion describes",
  );
  await wait(
    () =>
      !w.document.querySelector("#editor").open &&
      w.document.body.textContent.includes("meets criterion"),
    "the observation shown as meeting the criterion",
  );
  falsify();
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
  // The assessment it replaces is listed under its role (HYPO-0045).
  const replaced = listed("assessment").find(
    (e) => e.record.title === "Reviewed the output",
  );
  assert.ok(
    [...w.document.querySelectorAll("section.section")]
      .find((s) => s.querySelector("h2")?.textContent.startsWith("Supersedes"))
      ?.querySelector(href(replaced.record.id)),
    "superseded assessment listed",
  );
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
  // HYPO-0071: evidence that supports a falsification criterion meets it, so
  // it counts against the hypothesis, in the detail and in the matrix.
  cli(
    "evidence",
    "add",
    f.record.id,
    "Output zero on replication",
    "--source",
    "capture/run-3.txt",
  );
  w.location.hash = "record/" + h.record.id;
  const column = (stance) =>
    w.document.querySelector(`[data-stance="${stance}"]`)?.textContent || "";
  await wait(
    () => column("against").includes("Output zero on replication"),
    "criterion-meeting evidence shown against the hypothesis",
  );
  assert.ok(
    column("against").includes(
      `meets criterion ${f.record.id.slice(0, 10)} (counts against H)`,
    ),
    column("against"),
  );
  assert.ok(!column("for").includes("Output zero on replication"));
  w.location.hash = "matrix";
  await wait(() => h1(w) === "Evidence matrix", "matrix");
  const row = [...w.document.querySelectorAll("tbody tr")].find((tr) =>
    tr.textContent.includes("Output zero on replication"),
  );
  assert.ok(row, "matrix row");
  const stances = [...row.querySelectorAll("a[data-stance]")].map(
    (a) => a.dataset.stance,
  );
  assert.deepEqual(stances, ["against"], row.textContent);
  assert.ok(row.textContent.includes("meets criterion"), row.textContent);
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
  // Relations as `hyp link --relation` spells them (the demo's competes_with link).
  const allText = w.document.querySelector("main").textContent;
  assert.ok(allText.includes("competes-with"), "competes-with in All records");
  assert.ok(!/competes.with/.test(allText.replaceAll("competes-with", "")));
  // Captured data (HYPO-0090): listed with metadata; text previewed as
  // escaped text, binary not at all; referrers shown both ways.
  const textFile = path.join(root, "boot.log");
  const hostile = '<img src=x onerror="window.pwned=1"> boot ok\n';
  fs.writeFileSync(textFile, hostile);
  const textData = cli("capture", textFile, "--origin", "serial console");
  const binFile = path.join(root, "dump.bin");
  fs.writeFileSync(binFile, Buffer.from([0, 1, 2, 255]));
  const binData = cli("capture", binFile, "--origin", "dd if=/dev/mem");
  const observed = cli(
    "observe",
    "Boot log shows ok",
    "--source",
    "serial",
    "--data",
    textData,
  );
  w.location.hash = "data";
  await wait(
    () =>
      h1(w) === "Data" &&
      w.document.querySelector("main").textContent.includes("dump.bin"),
    "data view lists captures",
  );
  const dataText = w.document.querySelector("main").textContent;
  for (const part of ["boot.log", "text/plain", "application/octet-stream", "serial console"])
    assert.ok(dataText.includes(part), part + " in " + dataText);
  w.location.hash = "record/" + textData;
  await wait(() => h1(w) === "boot.log", "text data detail");
  const pre = w.document.querySelector("pre.preview");
  assert.ok(pre, "text preview");
  assert.equal(pre.textContent, hostile);
  assert.equal(w.document.querySelector("main img"), null, "preview is not HTML");
  assert.equal(w.pwned, undefined);
  const detailText = w.document.querySelector("main").textContent;
  assert.ok(detailText.includes("Boot log shows ok"), "referrer listed");
  assert.ok(detailText.includes("serial console"));
  assert.equal(
    w.document.querySelector('[data-action="edit"][data-id="' + textData + '"]'),
    null,
    "data records cannot be edited",
  );
  assert.ok(w.document.querySelector('[data-action="archive"][data-id="' + textData + '"]'));
  w.location.hash = "record/" + binData;
  await wait(() => h1(w) === "dump.bin", "binary data detail");
  assert.equal(w.document.querySelector("pre.preview"), null, "no binary preview");
  assert.ok(
    w.document
      .querySelector("main")
      .textContent.includes("No preview for application/octet-stream"),
  );
  w.location.hash = "record/" + observed;
  await wait(() => h1(w) === "Boot log shows ok", "evidence with data");
  assert.ok(
    w.document.querySelector('main a[href="#record/' + textData + '"]'),
    "evidence links its data",
  );
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
  // A sync brings a link to evidence that did not arrive: an error between
  // records. The UI keeps rendering, names the repair and still saves.
  const dangling = "L-" + crypto.randomUUID();
  const missing = "E-" + crypto.randomUUID();
  const stamp = new Date().toISOString();
  fs.writeFileSync(
    path.join(root, "hyp", "links", dangling + ".md"),
    `---\nid: ${dangling}\ntitle: Synced link\ntags: []\narchived: false\ncreated_at: ${stamp}\nupdated_at: ${stamp}\nkind: link\nfrom: ${missing}\nto: ${h.record.id}\nrelation: supports\n---\nArrived with a sync.\n`,
  );
  const noticeText = () => w.document.querySelector("#notice").textContent;
  await wait(
    () => noticeText().includes("Saving still works unless it adds a new error"),
    "error between records explained",
  );
  assert.equal(w.document.querySelector("#connection").textContent, "Live");
  for (const [view, title] of [
    ["overview", "Hypotheses"],
    ["matrix", "Evidence matrix"],
    ["graph", "Relationships"],
    ["all", "All records"],
  ]) {
    w.location.hash = view;
    await wait(() => h1(w) === title, "view with dangling link " + view);
  }
  const checks = w.document.querySelector(".diagnostics").textContent;
  for (const line of [
    `Note: Restore ${missing} from the source of the merge or sync`,
    `Repair: hyp archive ${dangling}\nRepair: hyp delete ${dangling}`,
  ])
    assert.ok(checks.includes(line), line + " in " + checks);
  click(w, "#new");
  field(w, "title", "Saved while the notebook has an error");
  await save(w);
  await wait(
    () => h1(w) === "Saved while the notebook has an error",
    "save despite an error between records",
  );
  cli("archive", dangling);
  cli("delete", dangling);
  await wait(
    () => w.document.querySelector("#notice").hidden,
    "repair clears the notice",
  );
  // Should the event that a project is valid again be lost, the page reads
  // again itself (the server announces it: tests/api.rs).
  await recoversWithoutEvents(
    w,
    async () =>
      new Response(JSON.stringify({ error: "briefly unreadable" }), {
        status: 422,
      }),
    "briefly unreadable",
  );
  await recoversWithoutEvents(
    w,
    async (read) => {
      const s = await (await read()).json();
      s.diagnostics.push({
        path: "hyp/hypotheses/briefly-invalid.md",
        code: "malformed",
        severity: "error",
        blocks_writes: true,
        message: "briefly invalid",
      });
      return new Response(JSON.stringify(s), { status: 200 });
    },
    "briefly invalid",
  );
  await olderReadAfterSave(w);
  await newerReadDuringSave(w);
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
    "PASS: relationships named from both ends, runs on the hypothesis page, frozen targets marked when changed, record status/outcome/roles/direction, evidence card interpretations, interpretation ends fixed, unexplained observations and bears-on, hypothesis created from an observation in one write, long titles wrap while controls do not, unknown observation time kept on edit, DOM forms, real HTTP writes, CLI↔UI SSE updates, editor changes, two tabs, dirty-form preservation, conflicts by status, saves despite unrelated writes, stale assessment rejected, evidence required and linked-only, criteria, evidence interpretations, criterion-meeting evidence counted against, falsification assessment, experiment and run, all views, captured data with escaped text previews, malformed-file recovery, saves and CLI repair despite a dangling link, unavailable-project cause, recovery when an event is lost, an older read not replacing a save nor a save a newer read, and offline export.",
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
