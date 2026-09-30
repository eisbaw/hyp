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
  connected = false;
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
async function refresh() {
  if (readOnly) return;
  try {
    const next = await fetchJSON("/api/snapshot");
    const blocking = next.diagnostics.filter((d) => d.blocks_writes);
    if (blocking.length) {
      if (!snapshot) {
        snapshot = next;
        render();
      }
      notice(
        "Invalid project files. Showing the last readable state; writes are blocked. " +
          blocking.map((d) => `${d.path}: ${d.message}`).join(" · "),
      );
      connectivity("Invalid files · stale");
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
    snapshot = next;
    if (!pending) notice(repairNotice(next));
    if (connected) connectivity("Live", true);
    if (changed) render();
  } catch (e) {
    notice(e.message);
    connectivity("Unavailable · retrying");
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
    `<div class="toolbar"><input id="search" aria-label="Search hypotheses" placeholder="Search statements, notes, tags…" value="${esc(filters.query)}"><select id="status-filter" aria-label="Assessment filter"><option value="">All assessments</option>${judgments.map((x) => `<option ${filters.status === x ? "selected" : ""} value="${x}">${human(x)}</option>`).join("")}</select><select id="tag-filter" aria-label="Tag filter"><option value="">All tags</option>${tags.map((t) => `<option ${filters.tag === t ? "selected" : ""} value="${esc(t)}">${esc(t)}</option>`).join("")}</select><label><input id="review-filter" type="checkbox" ${filters.review ? "checked" : ""}>Needs review</label><label><input id="archived-filter" type="checkbox" ${filters.archived ? "checked" : ""}>Archived</label></div><div class="cards" id="hypothesis-cards">${hypothesisCards()}</div>`
  );
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
function card({ record: r }) {
  const st = state(r.id);
  const rel = related(r.id);
  return `<article class="card"><div class="card-top"><span class="id">${esc(short(r.id))}</span>${badge(r.kind)}${st ? badge(st.judgment) : ""}${st?.needs_review ? badge("needs review", "review") : ""}${r.archived ? badge("archived") : ""}<span class="badges">${(r.tags || []).map((t) => `<span class="tag">${esc(t)}</span>`).join("")}</span></div><h3>${link(r)}</h3><p class="summary">${esc((r.scope || r.body || r.source || r.conditions || "").slice(0, 210))}</p><div class="card-bottom"><span>${r.kind === "hypothesis" ? `${rel.filter((e) => e.record.kind === "link").length} evidence / relation links &nbsp; · &nbsp; ${rel.filter((e) => e.record.kind === "experiment").length} experiments` : esc(r.source || r.status || r.outcome || r.judgment || (r.relation ? relationName(r.relation) : human(r.kind)))}</span><span>${r.lifecycle ? esc(r.lifecycle) + " &nbsp; · &nbsp; " : ""}${esc((r.updated_at || "").slice(0, 10))} <a class="arrow" aria-label="Open ${esc(r.title)}" href="#record/${esc(r.id)}">↗</a></span></div></article>`;
}
function item(e, extra = "") {
  const r = e.record;
  return `<div class="detail-item"><div class="item-top"><h3>${link(r)}</h3>${r.kind !== "assessment" && r.kind !== "run" ? button("edit", "Edit", r.id, 'class="mini-button"') : ""}</div><span class="id">${esc(short(r.id))}${r.archived ? " · archived" : ""}</span>${extra}${r.body ? `<div class="notes">${esc(r.body)}</div>` : ""}</div>`;
}
function section(title, entries, action = "", owner = "") {
  return `<section class="section"><div class="section-head"><h2>${esc(title)} <span class="small">${entries.length}</span></h2>${action ? button("create:" + action, "＋ Add", owner) : ""}</div>${entries.map((e) => item(e)).join("") || '<p class="small">Nothing recorded yet.</p>'}</section>`;
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
  const top = `<div class="page-title"><div><span class="eyebrow">${esc(human(r.kind))} · ${esc(short(r.id))}</span><h1>${esc(r.title)}</h1><div class="badges">${st ? badge(st.judgment) : ""}${st?.needs_review ? badge("needs review", "review") : ""}${r.lifecycle ? badge(r.lifecycle) : ""}${r.archived ? badge("archived") : ""}</div></div><div class="actions">${!["assessment", "run"].includes(r.kind) ? button("edit", "Edit record", r.id) + button(r.archived ? "restore" : "archive", r.archived ? "Restore" : "Archive", r.id) : ""}${r.archived ? button("delete", "Delete", r.id) : ""}</div></div>`;
  if (r.kind !== "hypothesis") {
    let extra = "";
    if (r.kind === "experiment") {
      extra = section(
        "Runs",
        all("run").filter((e) => e.record.experiment === id),
        "run",
        id,
      );
    }
    if (r.kind === "evidence") {
      extra = section(
        "Interpretations",
        all("link").filter((e) => e.record.from === id),
        "link",
        id,
      );
    }
    const references = [
      r.hypothesis,
      r.experiment,
      r.from,
      r.to,
      ...(r.evidence || []),
    ]
      .filter(Boolean)
      .map(find)
      .filter(Boolean);
    return (
      top +
      `<div class="panel"><div class="notes">${esc(r.body || "No additional notes.")}</div>${r.source ? `<p class="small">Source: ${esc(r.source)} · ${esc(r.locator)}</p>` : ""}${r.confidence != null ? `<p>Subjective confidence: ${Math.round(r.confidence * 100)}%</p>` : ""}</div>` +
      extra +
      section("Referenced records", references) +
      `<details class="section"><summary>Structured record · ${esc(e.revision.slice(0, 12))}</summary><pre class="raw">${esc(JSON.stringify(r, null, 2))}</pre></details>`
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
  const left = `<div>${r.scope ? `<div class="panel"><span class="eyebrow">SCOPE & CONTEXT</span><p>${esc(r.scope)}</p>${r.body ? `<div class="notes">${esc(r.body)}</div>` : ""}${r.assumptions ? `<p class="small">Assumptions: ${esc(r.assumptions)}</p>` : ""}</div>` : `<div class="panel notes">${esc(r.body || "Add scope, assumptions and notes to make this claim precise.")}</div>`}${section("What would falsify this?", criteria, "criterion", id)}${section("Predictions", predictions, "prediction", id)}<section class="section"><div class="section-head"><h2>Evidence & interpretation</h2>${button("create:evidence", "＋ Record evidence", id)}</div><p class="small">Evidence on a criterion or prediction is part of this hypothesis's basis. Evidence that meets a falsification criterion counts against it.</p><div class="evidence-columns"><div data-stance="for"><p class="evidence-heading">FOR THIS HYPOTHESIS</p>${evidenceBlock("for") || '<p class="small">No observations for it.</p>'}</div><div data-stance="against"><p class="evidence-heading negative">AGAINST THIS HYPOTHESIS</p>${evidenceBlock("against") || '<p class="small">No observations against it.</p>'}</div></div>${moreEvidence("qualifies", "QUALIFIES THIS HYPOTHESIS")}${moreEvidence("mixed", "MIXED: ITS LINKS DISAGREE")}</section>${section(
    "Experiments",
    owned.filter((e) => e.record.kind === "experiment"),
    "experiment",
    id,
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
      .map((e) =>
        item(
          e,
          `<p>${esc(relationName(e.record.relation))} ${find(e.record.to) ? link(find(e.record.to).record) : esc(e.record.to)}</p>`,
        ),
      )
      .join("") || '<p class="small">No hypothesis relationships.</p>'
  }</section></div>`;
  const side = `<div class="detail-side"><div class="panel"><span class="eyebrow">CURRENT ASSESSMENT</span><h2>${esc(human(st?.judgment || "untested"))}</h2>${current.length > 1 ? '<p class="diagnostics">Conflicting assessment branches. Add an assessment to reconcile them.</p>' : ""}${st?.confidence != null ? `<div class="meta-line"><span>Subjective confidence</span><strong>${Math.round(st.confidence * 100)}%</strong></div>` : ""}<p class="small">${st?.needs_review ? "The underlying record changed. This judgment needs review." : "Judgment is explicit. Evidence never changes it automatically."}</p>${current.map((e) => `<p class="notes">${esc(e.record.body)}</p>`).join("")}<div class="section">${button("create:assessment", "Review hypothesis", id, 'class="primary"')}</div></div><div class="panel section"><span class="eyebrow">NOTEBOOK DETAILS</span><div class="meta-line"><span>Lifecycle</span><strong>${esc(r.lifecycle)}</strong></div><div class="meta-line"><span>Created</span><strong>${esc(r.created_at.slice(0, 10))}</strong></div><p class="small">${r.tags.map((t) => "#" + esc(t)).join(" ")}</p><p class="small">${esc(r.untestable_reason || "")}</p><a href="#graph/${esc(id)}">Explore relationships ↗</a><details><summary class="small">Full ID & revision</summary><pre class="raw">${esc(id)}\n${esc(e.revision)}</pre></details></div></div>`;
  return top + `<div class="detail-grid">${left}${side}</div>`;
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
  editing = { kind, owner, entry, seen: snapshot };
  dirty = false;
  pending = false;
  $("#form-error").textContent = "";
  $("#editor-title").textContent = (entry ? "Edit " : "New ") + human(kind);
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
      field(
        "observed_at",
        "Observation date",
        r.observed_at || new Date().toISOString(),
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
    f +=
      select("from", "From", opts, r.from || owner) +
      select(
        "relation",
        "Relation",
        relations.map((r) => [r, relationName(r)]),
        r.relation || "supports",
      ) +
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
      );
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
  if (entry && get("advanced") !== JSON.stringify(entry.record, null, 2))
    return JSON.parse(get("advanced"));
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
      Object.assign(r, {
        from: get("from"),
        to: get("to"),
        relation: get("relation"),
      });
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
        record: {
          id: "L-" + crypto.randomUUID(),
          kind: "link",
          title: "Interpretation of " + r.title,
          from: r.id,
          to: fd.get("target"),
          relation: fd.get("relation"),
          body: fd.get("reason") || r.title,
          tags: [],
          archived: false,
          created_at: "",
          updated_at: "",
        },
      });
    snapshot = await transact(changes);
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
    snapshot = await transact([
      {
        op: action === "delete" ? "delete" : "archive",
        id,
        expected_revision: entry.revision,
        ...(action === "delete" ? {} : { archived: action === "archive" }),
      },
    ]);
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
