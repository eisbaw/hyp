"use strict";
const $ = (s, root = document) => root.querySelector(s);
const esc = (v) =>
  String(v ?? "").replace(
    /[&<>"']/g,
    (c) =>
      ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[
        c
      ],
  );
const short = (id) => String(id).slice(0, 10).toUpperCase();
const human = (s) => String(s ?? "").replaceAll("_", " ");
// A relation as `hyp link --relation` spells it: competes_with -> competes-with.
const relationName = (s) => String(s ?? "").replaceAll("_", "-");
const readOnly = !!window.HYP_EXPORT;
let snapshot = null,
  token = "",
  view = "overview",
  selected = "",
  editing = null,
  dirty = false,
  pending = false,
  connected = false,
  retry = null,
  retries = 0,
  reads = 0;
// The backstop re-read (`retrySoon`): how long after a failed or blocked
// read, and how many times in a row; the server's own poll is every 2 s.
const RETRY_MS = 2000;
const RETRY_LIMIT = 15;
let filters = {
  query: "",
  status: "",
  tag: "",
  review: false,
  archived: false,
};
const kinds = [
  "hypothesis",
  "prediction",
  "criterion",
  "evidence",
  "link",
  "experiment",
  "run",
  "assessment",
  "gap",
];
const judgments = [
  "untested",
  "inconclusive",
  "supported",
  "weakened",
  "falsified",
];
const lifecycles = ["draft", "investigating", "paused", "closed"];
const relations = [
  "supports",
  "contradicts",
  "qualifies",
  "depends_on",
  "competes_with",
  "supersedes",
];
const records = () => snapshot?.objects || [];
const all = (kind) =>
  records().filter(
    (e) => !e.record.archived && (!kind || e.record.kind === kind),
  );
const find = (id) => records().find((e) => e.record.id === id);
// Records whose `data` references name data record `id`, archived ones too:
// each keeps it from being deleted.
const dataUsers = (id) =>
  records().filter((e) => (e.record.data || []).includes(id));
// A size in bytes for people.
const bytes = (n) =>
  n < 1024
    ? `${n} B`
    : n < 1048576
      ? `${(n / 1024).toFixed(1)} KiB`
      : `${(n / 1048576).toFixed(1)} MiB`;
// Data records and records of kinds that cannot change are not edited.
const immutable = (r) => ["assessment", "run", "data"].includes(r.kind);
const state = (id) => snapshot?.hypotheses[id];
// What each observation linked to hypothesis `id` means for it, derived by
// the server (`Snapshot::evidence_bearings`): evidence that supports a
// criterion meets it and counts against the hypothesis.
const bearings = (id) => snapshot?.bearings?.[id] || [];
// A stance drawn with the colours of the relation it amounts to.
const stanceClass = {
  for: "supports",
  against: "contradicts",
  qualifies: "qualifies",
  mixed: "review",
};
const stanceBadge = (stance) => badge(stance, stanceClass[stance]);
// The hypotheses (archived ones included) that evidence `id` bears on, in
// project order, each with what it means for it: `Snapshot::bears_on`, read
// from the server's bearings.
const bearsOn = (id) =>
  records().flatMap((h) => {
    const b = snapshot?.bearings?.[h.record.id]?.find((x) => x.evidence === id);
    return b ? [{ hypothesis: h, bearing: b }] : [];
  });
// Observations no live hypothesis accounts for, the set `hyp status` lists
// (`Snapshot::unexplained`, derived by the server).
const unexplainedIds = () => snapshot?.unexplained_observations || [];
const isUnexplained = (id) => unexplainedIds().includes(id);
const runsOf = (experiment) =>
  all("run").filter((e) => e.record.experiment === experiment);
// A record by ID as a link, or the bare ID when it is missing.
const linkTo = (id) => (find(id) ? link(find(id).record) : esc(id));
// A record by ID as plain text: short ID and title.
const named = (id) =>
  short(id) + (find(id) ? " · " + find(id).record.title : "");
const ENDPOINTS_FIXED =
  "The ends of a link cannot change: link the records anew and archive this link.";
// The reason of a link from `hyp add --explains` when none is given
// (cli.rs, Command::Add).
const EXPLAINS_REASON = "Proposed as an explanation of this observation";
// The observation a new hypothesis is created to explain: the evidence
// whose page the form was opened from (`hyp add --explains`).
const observationToExplain = (kind, owner, r = {}) =>
  kind === "hypothesis" && !r.id && find(owner)?.record.kind === "evidence"
    ? find(owner).record
    : null;
// A new link record, as `hyp link` and `hyp add --explains` write it.
const newLink = (from, to, relation, title, body) => ({
  id: "L-" + crypto.randomUUID(),
  kind: "link",
  title,
  from,
  to,
  relation,
  body,
  tags: [],
  archived: false,
  created_at: "",
  updated_at: "",
});
const related = (id) =>
  all().filter(
    (e) =>
      e.record.hypothesis === id ||
      e.record.experiment === id ||
      e.record.from === id ||
      e.record.to === id,
  );
const badge = (text, cls = text) =>
  `<span class="badge ${esc(cls)}">${esc(human(text))}</span>`;
const button = (action, label, id = "", extra = "") =>
  `<button data-action="${esc(action)}" data-id="${esc(id)}" ${extra}>${esc(label)}</button>`;
const link = (r, text = r.title) =>
  `<a href="#record/${esc(r.id)}">${esc(text)}</a>`;
const empty = (title, desc, action = "") =>
  `<div class="empty"><h2>${esc(title)}</h2><p>${esc(desc)}</p>${action}</div>`;
const heading = (eyebrow, title, desc, num = "") =>
  `<div class="page-title"><div><span class="eyebrow">${esc(eyebrow)}</span><h1>${esc(title)}</h1><p class="subtitle">${esc(desc)}</p></div><span class="page-number">${esc(num)}</span></div>`;
function toast(text) {
  $("#toast").textContent = text;
  $("#toast").hidden = false;
  setTimeout(() => ($("#toast").hidden = true), 3000);
}
function notice(text) {
  $("#notice").textContent = text;
  $("#notice").hidden = !text;
}
function connectivity(text, live = false) {
  $("#connection").textContent = text;
  $("#connection").className = live ? "live" : "";
}
async function fetchJSON(url, options) {
  const res = await fetch(url, options);
  let data;
  try {
    data = await res.json();
  } catch {
    throw new Error(`Request failed (${res.status})`);
  }
  if (!res.ok) {
    const error = new Error(data.error || `Request failed (${res.status})`);
    error.status = res.status;
    throw error;
  }
  return data;
}
/** A backstop, not the way the page learns of recovery: the server
 * announces every state any read of it saw (`announce` in src/web.rs), so
 * an event follows when the project is valid again. Should that event be
 * lost (the event stream reconnecting, say), the page reads again itself,
 * `RETRY_MS` apart and at most `RETRY_LIMIT` times in a row, then waits
 * for the next event. Asked repeatedly, it keeps one timer. */
function retrySoon() {
  if (retry || retries >= RETRY_LIMIT) return;
  retries += 1;
  retry = setTimeout(() => {
    retry = null;
    refresh();
  }, RETRY_MS);
}
/** Takes `next` as the page's state: from a read, or the answer to a
 * write. Any read still on its way is older and is then ignored. */
function adopt(next) {
  reads += 1;
  snapshot = next;
}
async function refresh() {
  if (readOnly) return;
  // Reads overlap (server events, retries, closing the editor); an older
  // answer arriving last must not replace a newer one.
  const read = ++reads;
  try {
    const next = await fetchJSON("/api/snapshot");
    if (read !== reads) return;
    const blocking = next.diagnostics.filter((d) => d.blocks_writes);
    if (blocking.length) {
      if (!snapshot) {
        adopt(next);
        render();
      }
      notice(
        "Invalid project files. Showing the last readable state; writes are blocked. " +
          blocking.map((d) => `${d.path}: ${d.message}`).join(" · "),
      );
      connectivity("Invalid files · stale · retrying");
      retrySoon();
      return;
    }
    if (
      $("#editor").open &&
      dirty &&
      snapshot &&
      next.revision !== snapshot.revision
    ) {
      pending = true;
      notice(
        "The notebook changed while you were editing. Your draft is preserved; saving will check for conflicts.",
      );
      return;
    }
    const changed = !snapshot || snapshot.revision !== next.revision;
    adopt(next);
    retries = 0;
    if (!pending) notice(repairNotice(next));
    if (connected) connectivity("Live", true);
    if (changed) render();
  } catch (e) {
    if (read !== reads) return;
    notice(e.message);
    connectivity("Unavailable · retrying");
    retrySoon();
  }
}
function route() {
  const hash = location.hash.slice(1) || "overview";
  [view, selected = ""] = hash.split("/");
  render();
}
/** Errors between records, as a merge or sync leaves them, do not block
 * writes; the server rejects only writes that add an error. */
function repairNotice(s) {
  const n = s.diagnostics.filter((d) => d.severity === "error").length;
  if (!n) return "";
  return `${n} notebook error${n === 1 ? "" : "s"}, for example from a merge or sync. Saving still works unless it adds a new error. See the checks below for repairs.`;
}
/** A diagnostic's repair as `hyp check` prints it: the note, then each
 * command (an argv array, run in the project directory). */
function repairLines(r) {
  if (!r) return "";
  return [
    ...(r.note ? [`Note: ${r.note}`] : []),
    ...r.commands.map((c) => `Repair: ${c.join(" ")}`),
  ]
    .map((l) => "\n" + l)
    .join("");
}
function diagnostics() {
  const ds = snapshot.diagnostics;
  if (!ds.length) return "";
  return `<details class="diagnostics"><summary>${ds.length} notebook check${ds.length === 1 ? "" : "s"} to review</summary><pre>${esc(ds.map((d) => `${d.severity.toUpperCase()} · ${d.path}\n${d.message}${repairLines(d.repair)}`).join("\n\n"))}</pre></details>`;
}
function render() {
  if (!snapshot) return;
  $("#hyp-count").textContent = all("hypothesis").length;
  const names = {
    overview: "Hypotheses",
    experiments: "Experiments",
    evidence: "Evidence",
    data: "Data",
    matrix: "Evidence matrix",
    graph: "Relationships",
    all: "All records",
    record: "Record",
  };
  $("#crumb").textContent = names[view] || "Notebook";
  document
    .querySelectorAll("nav a")
    .forEach((a) =>
      a.classList.toggle(
        "active",
        a.dataset.view === view ||
          (view === "record" && a.dataset.view === "overview"),
      ),
    );
  let content = "";
  if (view === "record") {
    content = detail(selected);
  } else if (view === "matrix") {
    content = matrix();
  } else if (view === "graph") {
    content = graph();
  } else if (view === "experiments") {
    content =
      heading(
        "PLAN · OBSERVE · REPEAT",
        "Experiments",
        "Keep the plan, the execution, and the observation separate.",
        "02",
      ) +
      `<div class="actions">${button("create:experiment", "＋ Plan experiment")}</div>` +
      experimentQueue();
  } else if (view === "evidence") {
    content =
      heading(
        "OBSERVATIONS, WITH PROVENANCE",
        "Evidence",
        "What happened, where it came from, and what it means.",
        "03",
      ) +
      `<div class="actions">${button("create:evidence", "＋ Add evidence")}</div>` +
      `<div class="section cards">${all("evidence").map(card).join("") || empty("No observations yet", "Record the first result, including its source.")}</div>`;
  } else if (view === "data") {
    content =
      heading(
        "THE RAW MATERIAL",
        "Data",
        "Captured bytes, with where they came from. Capture with hyp capture; they cannot change.",
        "04",
      ) +
      `<div class="section cards">${all("data").map(card).join("") || empty("No data captured yet", "hyp capture FILE --origin \"where it came from\" keeps a log, output or file here.")}</div>`;
  } else if (view === "all") {
    content =
      heading(
        "THE COMPLETE NOTEBOOK",
        "All records",
        "Every claim, criterion, observation and assessment in one place.",
      ) +
      `<div class="actions">${kinds.map((k) => button("create:" + k, "＋ " + human(k))).join("")}</div><div class="section cards">${records().map(card).join("") || empty("Your notebook is empty", "Begin with a hypothesis.")}</div>`;
  } else {
    content = overview();
  }
  $("#content").innerHTML = diagnostics() + content;
  if (view === "overview") {
    bindFilters();
  }
  if (view === "graph")
    $("#graph-focus")?.addEventListener("change", (e) => {
      location.hash = "graph/" + e.target.value;
    });
}
function stats() {
  const hs = all("hypothesis");
  return `<div class="stats"><div class="stat"><strong>${hs.length}</strong><span>Hypotheses in play</span></div><div class="stat"><strong>${hs.filter((e) => state(e.record.id)?.needs_review).length}</strong><span>Need another look</span></div><div class="stat"><strong>${all("evidence").length}</strong><span>Recorded observations</span></div><div class="stat"><strong>${all("experiment").filter((e) => ["planned", "running"].includes(e.record.status)).length}</strong><span>Experiments ahead</span></div></div>`;
}
function overview() {
  const tags = [
    ...new Set(all("hypothesis").flatMap((e) => e.record.tags)),
  ].sort();
  return (
    heading(
      "ASK BETTER QUESTIONS",
      "Hypotheses",
      "Make your assumptions explicit. Follow the evidence. Leave room to be wrong.",
      "01",
    ) +
    stats() +
    unexplainedSection() +
    `<div class="toolbar"><input id="search" aria-label="Search hypotheses" placeholder="Search statements, notes, tags…" value="${esc(filters.query)}"><select id="status-filter" aria-label="Assessment filter"><option value="">All assessments</option>${judgments.map((x) => `<option ${filters.status === x ? "selected" : ""} value="${x}">${human(x)}</option>`).join("")}</select><select id="tag-filter" aria-label="Tag filter"><option value="">All tags</option>${tags.map((t) => `<option ${filters.tag === t ? "selected" : ""} value="${esc(t)}">${esc(t)}</option>`).join("")}</select><label><input id="review-filter" type="checkbox" ${filters.review ? "checked" : ""}>Needs review</label><label><input id="archived-filter" type="checkbox" ${filters.archived ? "checked" : ""}>Archived</label></div><div class="cards" id="hypothesis-cards">${hypothesisCards()}</div>`
  );
}
/** The observations no live hypothesis accounts for, each with a way to
 * propose one that explains it. Nothing while every observation is
 * explained. */
function unexplainedSection() {
  const es = unexplainedIds().map(find).filter(Boolean);
  if (!es.length) return "";
  return `<section class="section" id="unexplained"><div class="section-head"><h2>Unexplained observations <span class="small">${es.length}</span></h2></div><p class="small">No live hypothesis accounts for these: none is supported or qualified by them, or each one that is has been falsified or archived.</p>${es
    .map((e) =>
      item(
        e,
        `<p class="small">${esc(e.record.source)}</p>${button("create:hypothesis", "＋ Explain it", e.record.id, 'class="mini-button"')}`,
      ),
    )
    .join("")}</section>`;
}
function hypothesisCards() {
  const rows = records().filter(
    ({ record: r }) =>
      r.kind === "hypothesis" &&
      (filters.archived || !r.archived) &&
      (!filters.status || state(r.id)?.judgment === filters.status) &&
      (!filters.review || state(r.id)?.needs_review) &&
      (!filters.tag || r.tags.includes(filters.tag)) &&
      JSON.stringify(r).toLowerCase().includes(filters.query.toLowerCase()),
  );
  return (
    rows.map(card).join("") ||
    empty(
      "Start with a question",
      "Capture a hypothesis, then write down what would change your mind.",
      button("create:hypothesis", "＋ New hypothesis"),
    )
  );
}
function bindFilters() {
  for (const [id, key] of [
    ["search", "query"],
    ["status-filter", "status"],
    ["tag-filter", "tag"],
    ["review-filter", "review"],
    ["archived-filter", "archived"],
  ]) {
    $("#" + id).addEventListener("input", (e) => {
      filters[key] =
        e.target.type === "checkbox" ? e.target.checked : e.target.value;
      $("#hypothesis-cards").innerHTML = hypothesisCards();
    });
  }
}
/** What a card says about a record beyond its summary: the hypothesis an
 * experiment tests; for evidence, what it means for each hypothesis it
 * bears on, or that nothing explains it. */
function cardContext(r) {
  if (r.kind === "experiment")
    return `<p class="context">Tests ${linkTo(r.hypothesis)}</p>`;
  if (r.kind !== "evidence") return "";
  const lines = bearsOn(r.id).map(
    ({ hypothesis: h, bearing: b }) =>
      `<li>${stanceBadge(b.stance)} ${link(h.record)} <span class="small">${esc(b.bearings.map((x) => x.meaning).join("; "))}</span></li>`,
  );
  if (isUnexplained(r.id))
    lines.push(
      `<li>${badge("unexplained", "review")} <span class="small">No live hypothesis accounts for it.</span></li>`,
    );
  return lines.length
    ? `<ul class="context interpretations">${lines.join("")}</ul>`
    : "";
}
function card({ record: r }) {
  const st = state(r.id);
  const rel = related(r.id);
  return `<article class="card"><div class="card-top"><span class="id">${esc(short(r.id))}</span>${badge(r.kind)}${st ? badge(st.judgment) : ""}${st?.needs_review ? badge("needs review", "review") : ""}${r.archived ? badge("archived") : ""}<span class="badges">${(r.tags || []).map((t) => `<span class="tag">${esc(t)}</span>`).join("")}</span></div><h3>${link(r)}</h3><p class="summary">${esc((r.scope || r.body || r.source || r.origin || r.conditions || "").slice(0, 210))}</p>${cardContext(r)}<div class="card-bottom"><span>${r.kind === "hypothesis" ? `${rel.filter((e) => e.record.kind === "link").length} evidence / relation links &nbsp; · &nbsp; ${rel.filter((e) => e.record.kind === "experiment").length} experiments` : esc(r.kind === "data" ? `${r.media_type} · ${bytes(r.size)} · used by ${dataUsers(r.id).length}` : r.source || r.status || r.outcome || r.judgment || (r.relation ? relationName(r.relation) : human(r.kind)))}</span><span>${r.lifecycle ? esc(r.lifecycle) + " &nbsp; · &nbsp; " : ""}${esc((r.updated_at || "").slice(0, 10))} <a class="arrow" aria-label="Open ${esc(r.title)}" href="#record/${esc(r.id)}">↗</a></span></div></article>`;
}
function item(e, extra = "", withBody = true) {
  const r = e.record;
  return `<div class="detail-item"><div class="item-top"><h3>${link(r)}</h3>${!immutable(r) ? button("edit", "Edit", r.id, 'class="mini-button"') : ""}</div><span class="id">${esc(short(r.id))}${r.archived ? " · archived" : ""}</span>${extra}${withBody && r.body ? `<div class="notes">${esc(r.body)}</div>` : ""}</div>`;
}
function section(title, entries, action = "", owner = "", render = item) {
  return `<section class="section"><div class="section-head"><h2>${esc(title)} <span class="small">${entries.length}</span></h2>${action ? button("create:" + action, "＋ Add", owner) : ""}</div>${entries.map((e) => render(e)).join("") || '<p class="small">Nothing recorded yet.</p>'}</section>`;
}
/** The evidence run `r` cites, as links. */
const cites = (r) =>
  (r.evidence || []).length
    ? "cites " + r.evidence.map(linkTo).join(" · ")
    : "cites no evidence";
/** A run: its outcome and the evidence it cites. */
function runItem(e) {
  return item(
    e,
    `<div class="badges">${badge(e.record.outcome)}</div><p class="small cited">${cites(e.record)}</p>`,
  );
}
/** An experiment with its status and each of its runs. */
function experimentItem(e) {
  const runs = runsOf(e.record.id);
  return item(
    e,
    `<div class="badges">${badge(e.record.status)}</div><div class="runs">${
      runs
        .map(
          ({ record: r }) =>
            `<div class="run" data-run="${esc(r.id)}">${badge(r.outcome)} ${link(r)} <span class="small">${cites(r)}</span></div>`,
        )
        .join("") || '<p class="small">No runs yet.</p>'
    }</div>`,
  );
}
/** The records `r` references, each under the role it plays for `r`
 * (empty roles left out). Links show theirs as a direction instead. */
function roles(r) {
  const ids = (v) => (Array.isArray(v) ? v : [v]).filter(Boolean);
  const list = {
    prediction: [["Hypothesis", r.hypothesis]],
    criterion: [["Hypothesis", r.hypothesis]],
    gap: [
      ["Hypothesis", r.hypothesis],
      ["Resolved by", r.resolved_by],
    ],
    experiment: [["Hypothesis under test", r.hypothesis]],
    run: [
      ["Plan: the experiment it ran", r.experiment],
      ["Cited evidence", r.evidence],
    ],
    assessment: [
      ["Assessed hypothesis", r.hypothesis],
      ["Evidence considered", r.evidence],
      ["Falsification criterion", r.criterion],
      ["Supersedes", r.supersedes],
    ],
  }[r.kind];
  // A reference to a record that is gone is shown as its ID, marked.
  const shown = (x) =>
    x.missing
      ? `<div class="detail-item" data-missing="${esc(x.missing)}"><div class="item-top"><h3>${esc(x.missing)}</h3>${badge("missing", "falsified")}</div></div>`
      : item(x);
  return (list || [])
    .map(([title, v]) => [title, ids(v).map((id) => find(id) || { missing: id })])
    .filter(([, entries]) => entries.length)
    .map(([title, entries]) => section(title, entries, "", "", shown))
    .join("");
}
/** An experiment's frozen targets: each as it was when the experiment was
 * created, marked when the record has changed (or gone) since. */
function targetsSection(x) {
  const rows = (x.targets || []).map((t) => {
    const now = find(t.id);
    const changed = !now || now.revision !== t.revision;
    const mark = !now
      ? badge("missing", "falsified")
      : changed
        ? badge("changed since frozen", "review")
        : badge("as frozen");
    const renamed =
      now && now.record.title !== t.title
        ? `<p class="small">Frozen as: ${esc(t.title)}</p>`
        : "";
    return `<div class="detail-item target" data-target="${esc(t.id)}" data-changed="${changed}"><div class="item-top"><h3>${now ? link(now.record) : esc(t.title)}</h3><span class="badges">${now ? badge(now.record.kind) : ""}${mark}</span></div><span class="id">${esc(short(t.id))} · frozen revision ${esc(t.revision.slice(0, 12))}</span>${renamed}${changed ? `<details><summary class="small">Frozen content</summary>${frozenFields(t.body)}</details>` : ""}</div>`;
  });
  return `<section class="section" data-section="targets"><div class="section-head"><h2>Frozen targets <span class="small">${rows.length}</span></h2></div><p class="small">What this experiment tests, as it was when planned. A changed target needs a new experiment.</p>${rows.join("") || '<p class="small">No targets.</p>'}</section>`;
}
/** A frozen copy of a record (`Entry::frozen`: the record as JSON) as its
 * fields; text that is not a JSON record (older notebooks) as it is. */
function frozenFields(body) {
  let r;
  try {
    r = JSON.parse(body);
  } catch {
    r = null;
  }
  if (!r || typeof r !== "object")
    return `<pre class="raw">${esc(body)}</pre>`;
  const skip = ["id", "kind", "archived", "created_at", "updated_at"];
  return Object.entries(r)
    .filter(([k, v]) => !skip.includes(k) && v !== "" && !(Array.isArray(v) && !v.length))
    .map(
      ([k, v]) =>
        `<div class="meta-line"><span>${esc(human(k))}</span><strong class="notes">${esc(Array.isArray(v) ? v.join(", ") : v)}</strong></div>`,
    )
    .join("");
}
/** A run's plan: the experiment as it was when the run was recorded. */
function planSection(run) {
  const p = run.plan || {};
  const now = find(p.id);
  const changed = !now || now.revision !== p.revision;
  return `<section class="section" data-section="plan" data-changed="${changed}"><div class="section-head"><h2>Frozen plan</h2></div><p class="small">${changed ? "The experiment has changed since this run; this is the plan as it ran." : "The experiment is unchanged since this run."} Revision ${esc((p.revision || "").slice(0, 12))}.</p><details><summary class="small">${esc(p.title)}</summary>${frozenFields(p.body)}</details></section>`;
}
/** A link's direction: from, relation, to. */
function directionPanel(r) {
  const end = (label, id) =>
    `<div class="end"><span class="eyebrow">${label}</span><strong>${linkTo(id)}</strong><span class="id">${esc(find(id) ? human(find(id).record.kind) + " · " : "")}${esc(short(id))}</span></div>`;
  return `<div class="panel direction" data-direction>${end("FROM", r.from)}<div class="relation">${badge(relationName(r.relation), r.relation)} →</div>${end("TO", r.to)}</div>`;
}
/** The hypotheses evidence `id` bears on with what it means for each, its
 * links to claims (each editable) and whether no live hypothesis accounts
 * for it, as `hyp show E-…` lists them under "Bears on". */
function bearsOnSection(id) {
  const rows = bearsOn(id).map(({ hypothesis: h, bearing: b }) =>
    item(
      h,
      `<div class="badges">${stanceBadge(b.stance)}${state(h.record.id) ? badge(state(h.record.id).judgment) : ""}${badge(h.record.lifecycle)}</div>${b.bearings
        .map(
          (x) =>
            `<div class="bearing"><a class="small" href="#record/${esc(x.link)}">${esc(x.meaning)}</a><p class="small">${esc(find(x.link)?.record.body)}</p>${button("edit", "Edit interpretation", x.link, 'class="mini-button"')}</div>`,
        )
        .join("")}`,
      false,
    ),
  );
  const none = isUnexplained(id)
    ? `<p class="unexplained">${badge("unexplained", "review")} No live hypothesis accounts for this observation.</p>`
    : "";
  return `<section class="section" data-section="bears-on"><div class="section-head"><h2>Bears on <span class="small">${rows.length}</span></h2><span class="actions">${button("create:link", "＋ Interpret", id)}${button("create:hypothesis", "＋ Explain with a new hypothesis", id)}</span></div>${none}${rows.join("") || '<p class="small">It bears on no hypothesis yet.</p>'}</section>`;
}
/** A record as JSON, frozen copies left out: they are shown above. */
function structured(r) {
  const shown = structuredClone(r);
  for (const t of [...(shown.targets || []), shown.plan].filter(Boolean))
    t.body = "(frozen content, shown above)";
  return JSON.stringify(shown, null, 2);
}
function detail(id) {
  const e = find(id);
  if (!e)
    return empty(
      "Record not found",
      "It may have been removed. Select another record from the notebook.",
    );
  const r = e.record,
    st = state(id);
  // What state the record is in, by kind: an experiment's status, a run's
  // outcome, an assessment's judgment (a link's relation: `directionPanel`).
  const facets = [
    r.status && badge(r.status),
    r.outcome && badge(r.outcome),
    r.judgment && badge(r.judgment),
  ]
    .filter(Boolean)
    .join("");
  const top = `<div class="page-title"><div><span class="eyebrow">${esc(human(r.kind))} · ${esc(short(r.id))}</span><h1>${esc(r.title)}</h1><div class="badges">${st ? badge(st.judgment) : ""}${st?.needs_review ? badge("needs review", "review") : ""}${r.lifecycle ? badge(r.lifecycle) : ""}${facets}${r.archived ? badge("archived") : ""}</div></div><div class="actions">${!immutable(r) ? button("edit", "Edit record", r.id) : ""}${!["assessment", "run"].includes(r.kind) ? button(r.archived ? "restore" : "archive", r.archived ? "Restore" : "Archive", r.id) : ""}${r.archived ? button("delete", "Delete", r.id) : ""}</div></div>`;
  // The data records this record references (decision-0005).
  const dataRefs = section(
    "Data",
    (r.data || []).map(find).filter(Boolean),
  );
  if (r.kind === "data") return top + dataDetail(e);
  if (r.kind !== "hypothesis") {
    let extra = "";
    if (r.kind === "experiment")
      extra =
        targetsSection(r) + section("Runs", runsOf(id), "run", id, runItem);
    if (r.kind === "run") extra = planSection(r);
    if (r.kind === "link") extra = directionPanel(r);
    if (r.kind === "evidence") {
      // Links from it that bear on no hypothesis (to an archived claim, say).
      const shown = new Set(
        bearsOn(id).flatMap(({ bearing: b }) => b.bearings.map((x) => x.link)),
      );
      const other = all("link").filter(
        (e) => e.record.from === id && !shown.has(e.record.id),
      );
      extra =
        bearsOnSection(id) +
        (other.length ? section("Other interpretations", other) : "");
    }
    const history = ["assessment", "run"].includes(r.kind)
      ? `<p class="small">${r.kind === "run" ? "Runs" : "Assessments"} are history: they cannot be edited or archived. Record a new one instead.</p>`
      : "";
    return (
      top +
      `<div class="panel"><div class="notes">${esc(r.body || "No additional notes.")}</div>${r.source ? `<p class="small">Source: ${esc(r.source)}${r.locator ? " · " + esc(r.locator) : ""}</p>` : ""}${r.confidence != null ? `<p>Subjective confidence: ${Math.round(r.confidence * 100)}%</p>` : ""}${history}</div>` +
      extra +
      (r.data?.length ? dataRefs : "") +
      roles(r) +
      `<details class="section"><summary>Structured record · ${esc(e.revision.slice(0, 12))}</summary><pre class="raw">${esc(structured(r))}</pre></details>`
    );
  }
  const owned = all().filter((e) => e.record.hypothesis === id);
  const criteria = owned.filter((e) => e.record.kind === "criterion");
  const predictions = owned.filter((e) => e.record.kind === "prediction");
  // Each observation once, with every link that brings it in and what that
  // link means for this hypothesis.
  const evidenceBlock = (stance) =>
    bearings(id)
      .filter((b) => b.stance === stance)
      .map((b) => {
        const ev = find(b.evidence);
        return ev
          ? item(
              ev,
              b.bearings
                .map(
                  (x) =>
                    `<div class="bearing">${stanceBadge(x.stance)} <strong class="small">${esc(x.meaning)}</strong><p class="small">${esc(find(x.link)?.record.body)}</p>${button("edit", "Edit interpretation", x.link, 'class="mini-button"')}</div>`,
                )
                .join(""),
            )
          : "";
      })
      .join("");
  const moreEvidence = (stance, title) => {
    const block = evidenceBlock(stance);
    return block
      ? `<div data-stance="${stance}"><p class="evidence-heading">${title}</p>${block}</div>`
      : "";
  };
  const assessments = records()
    .filter((e) => e.record.kind === "assessment" && e.record.hypothesis === id)
    .sort((a, b) => b.record.created_at.localeCompare(a.record.created_at));
  const current = st?.assessment_ids?.map(find).filter(Boolean) || [];
  const left = `<div>${r.scope ? `<div class="panel"><span class="eyebrow">SCOPE & CONTEXT</span><p>${esc(r.scope)}</p>${r.body ? `<div class="notes">${esc(r.body)}</div>` : ""}${r.assumptions ? `<p class="small">Assumptions: ${esc(r.assumptions)}</p>` : ""}</div>` : `<div class="panel notes">${esc(r.body || "Add scope, assumptions and notes to make this claim precise.")}</div>`}${r.data?.length ? dataRefs : ""}${section("What would falsify this?", criteria, "criterion", id)}${section("Predictions", predictions, "prediction", id)}<section class="section"><div class="section-head"><h2>Evidence & interpretation</h2>${button("create:evidence", "＋ Record evidence", id)}</div><p class="small">Evidence on a criterion or prediction is part of this hypothesis's basis. Evidence that meets a falsification criterion counts against it.</p><div class="evidence-columns"><div data-stance="for"><p class="evidence-heading">FOR THIS HYPOTHESIS</p>${evidenceBlock("for") || '<p class="small">No observations for it.</p>'}</div><div data-stance="against"><p class="evidence-heading negative">AGAINST THIS HYPOTHESIS</p>${evidenceBlock("against") || '<p class="small">No observations against it.</p>'}</div></div>${moreEvidence("qualifies", "QUALIFIES THIS HYPOTHESIS")}${moreEvidence("mixed", "MIXED: ITS LINKS DISAGREE")}</section>${section(
    "Experiments",
    owned.filter((e) => e.record.kind === "experiment"),
    "experiment",
    id,
    experimentItem,
  )}${section(
    "Open questions",
    owned.filter((e) => e.record.kind === "gap" && !e.record.resolved),
    "gap",
    id,
  )}${section("Assessment history", assessments)}<section class="section"><div class="section-head"><h2>Relationships</h2>${button("create:link", "＋ Link records", id)}</div>${
    all("link")
      .filter(
        (e) =>
          ["depends_on", "supersedes", "competes_with"].includes(
            e.record.relation,
          ) &&
          (e.record.from === id || e.record.to === id),
      )
      .map((e) => {
        // Named from this hypothesis's side: the other end of the link.
        const rel = esc(relationName(e.record.relation));
        return item(
          e,
          e.record.from === id
            ? `<p data-direction="from">This hypothesis ${rel} ${linkTo(e.record.to)}</p>`
            : `<p data-direction="to">${linkTo(e.record.from)} ${rel} this hypothesis</p>`,
        );
      })
      .join("") || '<p class="small">No hypothesis relationships.</p>'
  }</section></div>`;
  const side = `<div class="detail-side"><div class="panel"><span class="eyebrow">CURRENT ASSESSMENT</span><h2>${esc(human(st?.judgment || "untested"))}</h2>${current.length > 1 ? '<p class="diagnostics">Conflicting assessment branches. Add an assessment to reconcile them.</p>' : ""}${st?.confidence != null ? `<div class="meta-line"><span>Subjective confidence</span><strong>${Math.round(st.confidence * 100)}%</strong></div>` : ""}<p class="small">${st?.needs_review ? "The underlying record changed. This judgment needs review." : "Judgment is explicit. Evidence never changes it automatically."}</p>${current.map((e) => `<p class="notes">${esc(e.record.body)}</p>`).join("")}<div class="section">${button("create:assessment", "Review hypothesis", id, 'class="primary"')}</div></div><div class="panel section"><span class="eyebrow">NOTEBOOK DETAILS</span><div class="meta-line"><span>Lifecycle</span><strong>${esc(r.lifecycle)}</strong></div><div class="meta-line"><span>Created</span><strong>${esc(r.created_at.slice(0, 10))}</strong></div><p class="small">${r.tags.map((t) => "#" + esc(t)).join(" ")}</p><p class="small">${esc(r.untestable_reason || "")}</p><a href="#graph/${esc(id)}">Explore relationships ↗</a><details><summary class="small">Full ID & revision</summary><pre class="raw">${esc(id)}\n${esc(e.revision)}</pre></details></div></div>`;
  return top + `<div class="detail-grid">${left}${side}</div>`;
}
/** A data record: its metadata, a preview of text (escaped, as derived by
 * the server for text/* media types; the bytes themselves are never served)
 * and every record that references it. */
function dataDetail(e) {
  const r = e.record;
  const meta = [
    ["Origin", r.origin],
    ["Captured", r.captured_at],
    ["Media type", r.media_type],
    ["Size", `${bytes(r.size)} (${r.size} bytes)`],
    ["SHA-256", r.sha256],
  ]
    .map(
      ([k, v]) =>
        `<div class="meta-line"><span>${esc(k)}</span><strong>${esc(v)}</strong></div>`,
    )
    .join("");
  const preview = snapshot.previews?.[r.id];
  const shown =
    preview == null
      ? `<p class="small">${r.media_type?.startsWith("text/") ? "No preview: the stored bytes are missing or changed (see the checks)." : "No preview for " + esc(r.media_type) + "; hyp data get " + esc(short(r.id)) + " writes the bytes out."}</p>`
      : `<p class="small">${r.size > preview.length ? "The start of the data:" : "The data:"}</p><pre class="raw preview">${esc(preview)}</pre>`;
  return `<div class="panel"><span class="eyebrow">CAPTURED DATA · IMMUTABLE</span>${meta}${r.body ? `<div class="notes">${esc(r.body)}</div>` : ""}</div><section class="section"><div class="section-head"><h2>Preview</h2></div>${shown}</section>${section("Referenced by", dataUsers(r.id))}`;
}
function experimentQueue() {
  return ["planned", "running", "completed", "cancelled"]
    .map((status) => {
      const es = all("experiment").filter((e) => e.record.status === status);
      return `<section class="section"><div class="section-head"><h2>${human(status)} <span class="small">${es.length}</span></h2></div><div class="cards">${es.map(card).join("") || '<p class="small">No experiments.</p>'}</div></section>`;
    })
    .join("");
}
function matrix() {
  const hs = all("hypothesis"),
    es = all("evidence");
  return (
    heading(
      "COMPARE EXPLANATIONS",
      "Evidence matrix",
      "One observation can count for one explanation and against another. Evidence that meets a falsification criterion counts against its hypothesis. Blank cells mean no interpretation has been recorded.",
      "04",
    ) +
    (!hs.length || !es.length
      ? empty(
          "A matrix needs hypotheses and evidence",
          "Add both to compare their relationships.",
        )
      : `<div class="matrix-wrap"><table><thead><tr><th>Observation / source</th>${hs.map((h) => `<th>${link(h.record)}<br><span class="id">${short(h.record.id)}</span></th>`).join("")}</tr></thead><tbody>${es
          .map(
            (e) =>
              `<tr><td><span class="table-title">${link(e.record)}</span><span class="small">${esc(e.record.source)}</span></td>${hs
                .map((h) => {
                  const b = bearings(h.record.id).find(
                    (x) => x.evidence === e.record.id,
                  );
                  return `<td>${
                    b?.bearings
                      .map(
                        (x) =>
                          `<a data-stance="${esc(x.stance)}" title="${esc(find(x.link)?.record.body)}" href="#record/${esc(x.link)}">${stanceBadge(x.stance)}</a> <span class="small">${esc(x.meaning)}</span>`,
                      )
                      .join("<br>") || '<span class="small">—</span>'
                  }</td>`;
                })
                .join("")}</tr>`,
          )
          .join("")}</tbody></table></div>`)
  );
}
function graph() {
  const hs = all("hypothesis");
  const h = find(selected) || hs[0];
  if (!h)
    return (
      heading(
        "FOLLOW THE CONNECTIONS",
        "Relationships",
        "Explore one hypothesis and its immediate neighbours.",
      ) +
      empty(
        "No relationships yet",
        "Start with a hypothesis and connect evidence.",
      )
    );
  const id = h.record.id;
  const children = all().filter(
    (e) => e.record.hypothesis === id && e.record.kind !== "assessment",
  );
  const targetIDs = [id, ...children.map((e) => e.record.id)];
  const ls = all("link").filter(
    (e) => targetIDs.includes(e.record.to) || e.record.from === id,
  );
  const neighbors = [
    ...new Set(ls.flatMap((e) => [e.record.from, e.record.to])),
  ]
    .filter((x) => !targetIDs.includes(x))
    .map(find)
    .filter(Boolean);
  const nodes = [
    ...neighbors.map((e, i) => ({ e, x: 25, y: 35 + i * 95 })),
    {
      e: h,
      x: 340,
      y: Math.max(35, (Math.max(neighbors.length, children.length) - 1) * 47),
    },
    ...children.map((e, i) => ({ e, x: 655, y: 35 + i * 95 })),
  ];
  const byID = new Map(nodes.map((n) => [n.e.record.id, n]));
  const height = Math.max(
    300,
    Math.max(neighbors.length, children.length) * 95 + 40,
  );
  const edges = [
    ...ls.map((e) => ({
      from: e.record.from,
      to: e.record.to,
      relation: e.record.relation,
    })),
    ...children.map((e) => ({
      from: id,
      to: e.record.id,
      relation: e.record.kind,
    })),
  ];
  const edgeSVG = edges
    .map((l) => {
      const a = byID.get(l.from),
        b = byID.get(l.to);
      if (!a || !b) return "";
      const forward = a.x < b.x,
        x1 = a.x + (forward ? 260 : 0),
        x2 = b.x + (forward ? 0 : 260),
        y1 = a.y + 32,
        y2 = b.y + 32;
      const color =
        l.relation === "contradicts"
          ? "#b77460"
          : l.relation === "qualifies"
            ? "#a697b8"
            : "#91a58c";
      return `<path d="M${x1} ${y1} C${(x1 + x2) / 2} ${y1},${(x1 + x2) / 2} ${y2},${x2} ${y2}" stroke="${color}" stroke-width="1.5" fill="none"><title>${esc(relationName(l.relation))}</title></path>`;
    })
    .join("");
  const svg = nodes
    .map(
      ({ e, x, y }) =>
        `<a href="#record/${esc(e.record.id)}"><rect x="${x}" y="${y}" width="260" height="65" rx="7" fill="${e.record.id === id ? "#e5eddc" : "#fbfcf8"}" stroke="#ccd6c4"/><text x="${x + 13}" y="${y + 20}" font-size="9" fill="#7b8974">${esc(e.record.kind.toUpperCase())} · ${esc(short(e.record.id))}</text><text x="${x + 13}" y="${y + 43}" font-size="11" fill="#284432">${esc(e.record.title.slice(0, 36))}${e.record.title.length > 36 ? "…" : ""}</text><title>${esc(e.record.title)}</title></a>`,
    )
    .join("");
  return (
    heading(
      "FOLLOW THE CONNECTIONS",
      "Relationships",
      "A focused view of evidence, alternative explanations, and testable implications.",
      "05",
    ) +
    `<div class="toolbar"><select id="graph-focus" aria-label="Focus hypothesis">${hs.map((e) => `<option value="${esc(e.record.id)}" ${e.record.id === id ? "selected" : ""}>${esc(e.record.title)}</option>`).join("")}</select></div><div class="graph"><svg viewBox="0 0 940 ${height}" role="img" aria-label="Hypothesis relationship graph">${edgeSVG}${svg}</svg></div><div class="legend"><span><i></i>Supports / relationship</span><span><i class="negative"></i>Contradicts</span><span><i class="neutral"></i>Qualifies</span></div><p class="graph-note">Select a node to inspect its record. Hover over a connection to see its relation.</p>`
  );
}
function field(name, label, value = "", type = "text", help = "") {
  return `<label>${esc(label)}${type === "textarea" ? `<textarea name="${name}">${esc(value)}</textarea>` : `<input name="${name}" type="${type}" value="${esc(value)}" ${type === "number" ? 'step="0.01" min="0" max="1"' : ""}>`}${help ? `<span class="help">${esc(help)}</span>` : ""}</label>`;
}
function select(
  name,
  label,
  options,
  value = "",
  multiple = false,
  optional = false,
) {
  return `<label>${esc(label)}<select name="${name}" ${multiple ? "multiple" : ""}>${optional ? '<option value="">None</option>' : ""}${options
    .map((o) => {
      const [v, t] = Array.isArray(o) ? o : [o, human(o)];
      return `<option value="${esc(v)}" ${(multiple ? (value || []).includes(v) : v === value) ? "selected" : ""}>${esc(t)}</option>`;
    })
    .join("")}</select></label>`;
}
function objectOptions(kind) {
  return all(kind).map((e) => [
    e.record.id,
    short(e.record.id) + " · " + e.record.title,
  ]);
}
function openEditor(kind, owner = "", entry = null) {
  if (readOnly) return;
  // The form shows this snapshot, so every precondition of the save is taken
  // from it, never from a refresh that arrives while the form is open.
  // The observation a new hypothesis explains is fixed when the form opens:
  // the save then always sends its link, and the server rejects it if the
  // observation is gone meanwhile.
  const explains = observationToExplain(kind, owner, entry?.record)?.id || "";
  editing = { kind, owner, entry, seen: snapshot, explains };
  dirty = false;
  pending = false;
  $("#form-error").textContent = "";
  // A link from evidence is an interpretation of it.
  const interpretation =
    kind === "link" &&
    find(entry?.record.from ?? owner)?.record.kind === "evidence";
  $("#editor-title").textContent =
    (entry ? "Edit " : "New ") + (interpretation ? "interpretation" : human(kind));
  $("#editor-fields").innerHTML = formFields(kind, owner, entry?.record);
  $("#editor").showModal();
  $("#editor-fields input")?.focus();
}
function formFields(kind, owner, r = {}) {
  const hs = objectOptions("hypothesis");
  let f = field("title", "Statement / title", r.title || "");
  if (kind === "hypothesis") {
    f +=
      field("scope", "Scope", r.scope || "") +
      field("assumptions", "Assumptions", r.assumptions || "", "textarea") +
      select("lifecycle", "Lifecycle", lifecycles, r.lifecycle || "draft") +
      field(
        "untestable_reason",
        "Reason if no falsification criterion",
        r.untestable_reason || "",
      );
    const ev = observationToExplain(kind, owner, r);
    if (ev)
      f +=
        `<p class="small" data-explains="${esc(ev.id)}">Explains ${esc(short(ev.id))} · ${esc(ev.title)}: saved together with a supports link from the observation to the new hypothesis.</p>` +
        field(
          "reason",
          "Why it would explain the observation",
          "",
          "textarea",
          `Left empty: ${EXPLAINS_REASON}.`,
        );
  }
  if (
    ["prediction", "criterion", "gap", "experiment", "assessment"].includes(
      kind,
    )
  )
    f += select(
      "hypothesis",
      "Hypothesis",
      hs,
      r.hypothesis || owner || hs[0]?.[0],
    );
  if (kind === "prediction")
    f += field("conditions", "Conditions", r.conditions || "");
  if (kind === "gap")
    f += select(
      "resolved",
      "Resolved?",
      [
        ["false", "Open question"],
        ["true", "Resolved"],
      ],
      String(r.resolved || false),
    );
  if (kind === "evidence") {
    f +=
      field(
        "source",
        "Source",
        r.source || "",
        "text",
        "A URL, project-relative path, or description of the observation source.",
      ) +
      field("locator", "Precise locator", r.locator || "") +
      // RFC 3339 with an offset (toISOString: UTC) or a date; empty means
      // unknown. Only a new record defaults to now: an edit keeps "unknown".
      field(
        "observed_at",
        "Observation date",
        r.id ? r.observed_at || "" : new Date().toISOString(),
        "text",
        "An RFC 3339 timestamp with offset, or YYYY-MM-DD; empty if unknown.",
      );
    if (!r.id)
      f +=
        select(
          "target",
          "Interpretation target",
          all()
            .filter((e) =>
              ["hypothesis", "prediction", "criterion"].includes(e.record.kind),
            )
            .map((e) => [e.record.id, e.record.title]),
          owner,
          false,
          true,
        ) +
        select(
          "relation",
          "Relation",
          ["supports", "contradicts", "qualifies"],
          "supports",
        ) +
        field("reason", "Interpretation / why it matters", "", "textarea");
  }
  if (kind === "link") {
    const opts = all()
      .filter((e) =>
        ["hypothesis", "prediction", "criterion", "evidence"].includes(
          e.record.kind,
        ),
      )
      .map((e) => [e.record.id, short(e.record.id) + " · " + e.record.title]);
    const relation = select(
      "relation",
      "Relation",
      relations.map((r) => [r, relationName(r)]),
      r.relation || "supports",
    );
    // The ends of an existing link are fixed (`buildRecord`).
    f += r.id
      ? `<p class="small" data-endpoints>From ${esc(named(r.from))} to ${esc(named(r.to))}. ${ENDPOINTS_FIXED}</p>` +
        relation
      : select("from", "From", opts, r.from || owner) +
        relation +
        select("to", "To", opts, r.to || "");
  }
  if (kind === "experiment") {
    f += select(
      "status",
      "Status",
      ["planned", "running", "completed", "cancelled"],
      r.status || "planned",
    );
    if (!r.id)
      f += select(
        "targets",
        "Predictions / criteria to freeze",
        all()
          .filter((e) => ["prediction", "criterion"].includes(e.record.kind))
          .map((e) => [e.record.id, e.record.title]),
        [],
        true,
      );
    else
      f +=
        '<p class="small">Target revisions are frozen. Create a new experiment to test revised predictions.</p>';
  }
  if (kind === "run")
    f +=
      select("experiment", "Experiment", objectOptions("experiment"), owner) +
      select(
        "outcome",
        "Outcome",
        ["observed", "inconclusive", "failed"],
        "observed",
      ) +
      select(
        "evidence",
        "Resulting evidence",
        objectOptions("evidence"),
        [],
        true,
      );
  if (kind === "assessment")
    f +=
      select("judgment", "Assessment", judgments, "inconclusive") +
      field(
        "confidence",
        "Subjective confidence (optional)",
        r.confidence ?? "",
        "number",
        "0–1. This is your judgment, not a calculated probability.",
      ) +
      select(
        "evidence",
        "Evidence considered",
        objectOptions("evidence"),
        [],
        true,
      ) +
      '<p class="small">Required for every judgment except untested. Cite only evidence already linked to this hypothesis or its criteria or predictions, so the judgment rests on what you reviewed; link other evidence first.</p>' +
      select(
        "criterion",
        "Falsification criterion",
        objectOptions("criterion"),
        "",
        false,
        true,
      ) +
      '<p class="small">Required for falsified, and one cited observation must meet it: evidence recorded on the criterion (linked to it as supports). Evidence merely against the hypothesis does not.</p>';
  f +=
    field(
      "body",
      kind === "assessment"
        ? "Assessment rationale"
        : kind === "link"
          ? "Explanation"
          : kind === "experiment"
            ? "Procedure"
            : "Notes",
      r.body || "",
      "textarea",
    ) + field("tags", "Tags (comma-separated)", (r.tags || []).join(", "));
  if (r.id && !["assessment", "run"].includes(kind))
    f += `<details><summary class="small">Advanced: complete JSON record</summary><p class="small">If edited, this replaces the fields above. IDs, kinds and frozen references are validated.</p><textarea name="advanced" class="json" aria-label="Complete JSON record">${esc(JSON.stringify(r, null, 2))}</textarea></details>`;
  return f;
}
function buildRecord(form) {
  const { kind, entry } = editing;
  const fd = new FormData(form);
  const get = (k) => String(fd.get(k) ?? "");
  const many = (k) => fd.getAll(k).map(String);
  let r = entry
    ? structuredClone(entry.record)
    : {
        id: "",
        title: "",
        body: "",
        tags: [],
        archived: false,
        created_at: "",
        updated_at: "",
        kind,
      };
  if (entry && get("advanced") !== JSON.stringify(entry.record, null, 2)) {
    const edited = JSON.parse(get("advanced"));
    const old = entry.record;
    if (
      old.kind === "link" &&
      (edited.from !== old.from || edited.to !== old.to)
    )
      throw new Error(ENDPOINTS_FIXED);
    return edited;
  }
  r.title = get("title");
  r.body = get("body");
  r.tags = get("tags")
    .split(",")
    .map((t) => t.trim())
    .filter(Boolean);
  if (
    ["prediction", "criterion", "gap", "experiment", "assessment"].includes(
      kind,
    )
  )
    r.hypothesis = get("hypothesis");
  switch (kind) {
    case "hypothesis":
      Object.assign(r, {
        scope: get("scope"),
        assumptions: get("assumptions"),
        lifecycle: get("lifecycle"),
        untestable_reason: get("untestable_reason"),
      });
      break;
    case "prediction":
      r.conditions = get("conditions");
      break;
    case "gap":
      r.resolved = get("resolved") === "true";
      // Reopening forgets what resolved it, as `hyp set --resolved false`.
      if (!r.resolved) delete r.resolved_by;
      break;
    case "evidence":
      Object.assign(r, {
        source: get("source"),
        locator: get("locator"),
        observed_at: get("observed_at"),
        attachments: r.attachments || [],
      });
      break;
    case "link":
      // An edit keeps the ends (`ENDPOINTS_FIXED`); its form has no fields for them.
      if (!entry) Object.assign(r, { from: get("from"), to: get("to") });
      r.relation = get("relation");
      break;
    case "experiment":
      r.status = get("status");
      if (!entry) r.targets = many("targets").map((id) => ({ id }));
      break;
    case "run":
      Object.assign(r, {
        experiment: get("experiment"),
        outcome: get("outcome"),
        evidence: many("evidence"),
      });
      break;
    case "assessment":
      Object.assign(r, {
        judgment: get("judgment"),
        confidence: get("confidence") === "" ? null : Number(get("confidence")),
        evidence: many("evidence"),
        criterion: get("criterion") || null,
        based_on: "",
        supersedes: [],
      });
      break;
  }
  if (!r.id) {
    const prefixes = {
      hypothesis: "H",
      prediction: "P",
      criterion: "F",
      evidence: "E",
      link: "L",
      experiment: "X",
      run: "R",
      assessment: "A",
      gap: "G",
    };
    r.id = prefixes[kind] + "-" + crypto.randomUUID();
  }
  return r;
}
// Mirrors Change::create_seen: the `expected` of a create, stating what
// `seen` (the snapshot the form was opened on) held of what the server
// derives or freezes the new record's content from: for an assessment its
// hypothesis' review token, which covers the linked evidence it may cite.
function expectedFrom(r, seen) {
  const at = (id) => seen.objects.find((e) => e.record.id === id);
  const revisions = {};
  const state = (id) => {
    if (at(id)) revisions[id] = at(id).revision;
  };
  const hypotheses = {};
  const st = r.kind === "assessment" && seen.hypotheses[r.hypothesis];
  if (st) hypotheses[r.hypothesis] = { review_token: st.review_token };
  if (r.kind === "experiment") {
    if (!r.targets.some((t) => t.id === r.hypothesis))
      r.targets.unshift({ id: r.hypothesis });
    r.targets.forEach((t) => state(t.id));
  }
  if (r.kind === "run") [r.experiment, ...r.evidence].forEach(state);
  return { hypotheses, revisions };
}
// Each change carries its own precondition; no whole-project revision, so
// unrelated concurrent writes do not reject the save.
async function transact(changes) {
  return fetchJSON("/api/transaction", {
    method: "POST",
    headers: { "Content-Type": "application/json", "X-Hyp-Token": token },
    body: JSON.stringify({ changes }),
  });
}
async function save(ev) {
  ev.preventDefault();
  const submit = $("button[type=submit]", ev.target);
  submit.disabled = true;
  try {
    const r = buildRecord(ev.target);
    const changes = [
      editing.entry
        ? { op: "update", record: r, expected_revision: editing.entry.revision }
        : {
            op: "create",
            record: r,
            expected: expectedFrom(r, editing.seen),
          },
    ];
    const fd = new FormData(ev.target);
    if (!editing.entry && r.kind === "evidence" && fd.get("target"))
      changes.push({
        op: "create",
        record: newLink(
          r.id,
          fd.get("target"),
          fd.get("relation"),
          "Interpretation of " + r.title,
          fd.get("reason") || r.title,
        ),
      });
    // A hypothesis proposed for an observation, linked in the same write.
    if (editing.explains)
      changes.push({
        op: "create",
        record: newLink(
          editing.explains,
          r.id,
          "supports",
          `${r.id.slice(0, 10)} explains ${editing.explains.slice(0, 10)}`,
          fd.get("reason") || EXPLAINS_REASON,
        ),
      });
    adopt(await transact(changes));
    dirty = false;
    pending = false;
    $("#editor").close();
    notice("");
    toast("Saved to your notebook");
    location.hash = "record/" + r.id;
    render();
  } catch (e) {
    $("#form-error").textContent =
      e.message +
      (e.status === 409
        ? " Copy your draft before closing, then reopen the record to reconcile changes."
        : "");
  } finally {
    submit.disabled = false;
  }
}
async function closeEditor() {
  if (dirty && !confirm("Discard this unsaved draft?")) return;
  dirty = false;
  $("#editor").close();
  pending = false;
  await refresh();
}
async function action(e) {
  const b = e.target.closest("[data-action]");
  if (!b || readOnly) return;
  const action = b.dataset.action,
    id = b.dataset.id;
  if (action.startsWith("create:")) {
    openEditor(action.split(":")[1], id);
    return;
  }
  if (action === "edit") {
    const entry = find(id);
    openEditor(entry.record.kind, "", entry);
    return;
  }
  const entry = find(id);
  if (!entry) return;
  if (
    action === "delete" &&
    !confirm(
      "Permanently delete this archived record? Referenced records cannot be deleted.",
    )
  )
    return;
  try {
    adopt(
      await transact([
        {
          op: action === "delete" ? "delete" : "archive",
          id,
          expected_revision: entry.revision,
          ...(action === "delete" ? {} : { archived: action === "archive" }),
        },
      ]),
    );
    toast(
      action === "restore"
        ? "Restored"
        : action === "delete"
          ? "Deleted"
          : "Archived",
    );
    if (action === "delete") location.hash = "overview";
    render();
  } catch (err) {
    notice(err.message);
  }
}
async function start() {
  document.body.classList.toggle("read-only", readOnly);
  document.addEventListener("click", action);
  window.addEventListener("hashchange", route);
  $("#new").addEventListener("click", () => openEditor("hypothesis"));
  $("#editor-form").addEventListener("submit", save);
  $("#editor-form").addEventListener("input", () => (dirty = true));
  $("#editor-form").addEventListener("change", () => (dirty = true));
  $("#close-editor").addEventListener("click", closeEditor);
  $("#cancel-editor").addEventListener("click", closeEditor);
  $("#editor").addEventListener("cancel", (e) => {
    e.preventDefault();
    closeEditor();
  });
  window.addEventListener("beforeunload", (e) => {
    if (dirty) {
      e.preventDefault();
      e.returnValue = "";
    }
  });
  if (readOnly) {
    snapshot = window.HYP_EXPORT;
    connectivity("Read-only snapshot");
    route();
    return;
  }
  try {
    token = (await fetchJSON("/api/session")).token;
    await refresh();
    route();
    const stream = new EventSource("/api/events");
    stream.onopen = () => {
      connected = true;
      connectivity("Live", true);
    };
    stream.onerror = () => {
      connected = false;
      connectivity("Reconnecting…");
    };
    stream.addEventListener("change", async () => {
      await refresh();
    });
  } catch (e) {
    notice(e.message);
    connectivity("Unavailable");
  }
}
if (document.readyState === "loading")
  document.addEventListener("DOMContentLoaded", start);
else start();
