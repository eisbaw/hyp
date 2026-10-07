# Related tools: strengths and weaknesses against hyp

Checked against each project's repository (README and, where noted, other files) in October 2026. None of the tools was installed or run for this comparison: their features are as documented, not as tested. Projects in this niche are young and change quickly, so check the source before relying on a detail here.

Claims about hyp refer to this repository's README and the files in `docs/`.

## Summary

| Tool | What it is | Overlap with hyp |
|---|---|---|
| [backlog.md](https://github.com/MrLesk/Backlog.md) | Task tracker: Markdown files, CLI for agents, browser view | Shape, not content |
| [Doubt](https://github.com/alsoleg89/doubt) | One contested question → a source-grounded evidence map | Evidence model |
| [RigorGraph](https://github.com/f0909172434/rigorgraph) | Claim → evidence → independent verification, with deterministic audits | Integrity checks, agent workflow |
| [Epistemos](https://github.com/Voltolini-SPACE/epistemos) | Memory and provenance engine for AI agents | Claims, evidence, reviews |
| [POPPER](https://github.com/snap-stanford/POPPER) (Stanford) | LLM agents that design and run falsification tests | Falsification, but it executes |
| [popper](https://github.com/kliewerdaniel/popper) (Kliewer) | Research pipeline that compiles hypotheses from literature | Strict hypothesis schema |
| [Argdown](https://github.com/argdown/argdown) | Plain-text argument maps | Pros/cons structure |
| [go-argmap](https://github.com/BaseMax/go-argmap) | Small Go TUI for argument trees | Argument structure |
| [Arguman](https://github.com/arguman/arguman.org) | Web platform for collaborative argument mapping | Visual support/objection |
| Analysis of Competing Hypotheses (Heuer, 1999) | A method, not software | Evidence matrix across rival hypotheses |

None of the tools above combines everything hyp does:

- hypotheses with explicit falsification criteria and predictions;
- evidence with a stance per hypothesis;
- experiments that freeze their targets, and runs that freeze the plan they executed;
- assessments that (except `untested`) must cite linked evidence and are flagged for review when their recorded basis changes;
- gaps, and observation-first work (`hyp observe`);
- one Markdown file per record, Git-friendly without requiring Git;
- a stable `--json` and exit-code contract for agents.

## In brief

- [backlog.md](https://github.com/MrLesk/Backlog.md) (MIT), the model for hyp's shape: Markdown files in any directory, a CLI for agents, a browser view for humans, agent instructions installed by the tool. It is imperative (what to do); hyp is descriptive (what is believed, and on what evidence).
- Analysis of Competing Hypotheses (Richards J. Heuer Jr., *Psychology of Intelligence Analysis*, CIA Center for the Study of Intelligence, 1999, chapter 8), the methodological precedent: weigh every observation against every rival hypothesis and look for the evidence that refutes, not the evidence that fits. hyp's evidence matrix compares hypotheses the same way; hyp adds explicit falsification criteria and records the judgments.
- [POPPER](https://github.com/snap-stanford/POPPER) (Huang et al., *Automated Hypothesis Validation with Agentic Sequential Falsifications*, 2025), an agentic falsification framework: LLM agents design and run falsification experiments under statistical error control. It performs the testing; hyp records an investigation and executes nothing.
- [Doubt](https://github.com/alsoleg89/doubt) (MIT), the closest evidence model: an agent skill and CLI that turn one contested question into a source-grounded evidence map with supporting, contradicting, qualifying and missing evidence. A map is a document per question; hyp is a notebook kept through an investigation, with falsification criteria, experiments and judgments that are flagged when their basis changes.
- [Argdown](https://github.com/argdown/argdown) (MIT), argument mapping: a plain-text syntax and tools that turn pros, cons and premise-conclusion structures into argument maps. It models the structure of an argument; hyp models evidence and judgments about a tentative claim.

## Per tool

### backlog.md

The model for hyp's shape.

- **Better than hyp:** mature and widely used; a richer board UI; Git awareness across branches.
- **Worse for this job:** it tracks what to do, not what is believed or on what evidence.

### Doubt

The closest evidence model: supports, contradicts, qualifies and missing edges, with strict sourcing.

- **Better than hyp:**
  - Installs from npm with one `npx` command (Node 18 or newer); hyp is built from source.
  - Every evidence node needs a URL or path, a retrieval date and a locator (section, page, timestamp or line). hyp requires a source and stores an observation date, but its locator is optional.
  - `doubt verify` fetches each source and matches the cited excerpt against its bytes.
  - It has a GitHub Action and an in-browser playground. A shared map travels in the URL fragment and is never uploaded.
  - Its Agent Skill installs through standard installers (`gh skill`, `npx skills`) into more hosts (Claude Code, Codex, Copilot, Cursor, Gemini CLI); hyp's skill covers Claude Code and Codex.
- **Worse than hyp:**
  - One document per question, with one provisional position. It is not a notebook of rival hypotheses kept through an investigation.
  - No falsification criteria, predictions, experiments or runs.
  - No assessment history, and nothing flags judgments whose basis changed.
  - No editing UI: its HTML map is read-only and editing happens in JSON. hyp has an editing WebUI.
- **About even:** both produce a self-contained offline HTML view (in hyp, `hyp export --format html`).

### RigorGraph

An auditable claim-evidence workflow. It is the closest to hyp in integrity checking.

- **Better than hyp:**
  - `VERIFIED` requires a recorded independent review. hyp lets any agent record a judgment, as long as it cites linked evidence (except `untested`) and states the current review token.
  - Claims and evidence are typed. Claim types: formal, literature, empirical, benchmark, synthesis. Evidence types: proof, source, dataset, computation, benchmark_run. Each claim type requires specific evidence before `VERIFIED`.
  - It ships a published Python package, a GitHub Action and a report in four interface languages.
- **Worse than hyp:**
  - Claim → evidence → verification only, with no explicit falsification criteria, predictions or experiments.
  - Records are JSONL lines. They are harder to read in a diff and to edit by hand than one Markdown file per record.
  - No editing UI: the report is read-only.
- **Shared ideas:**
  - SHA-256 checks of evidence bytes (hyp: data records, `changed_bytes`).
  - Reviews bound to a snapshot of what was reviewed (hyp: review fingerprint and token).
  - Stable audit codes (hyp: diagnostic codes).

### Epistemos

A much broader system: memory, provenance and claims for AI agents.

- **Better than hyp:**
  - Bitemporal history: it can say what the system believed at a past time. hyp keeps past judgments in its assessment history, but past states of other records only in Git or another VCS.
  - SDK, REST and MCP interfaces; authorization per knowledge space; a live web panel.
- **Different by design:** belief is derived from evidence and reviews, never stored. hyp derives nothing itself: it records the judgments agents and people make.
- **Worse for this job:**
  - It stores a single SQLite file. A JSON event-log export exists, but records are not files you edit by hand.
  - No falsification workflow.
  - Its scope is much larger than a hypothesis notebook needs: agent memory, context compression, multi-tenant authorization and a context protocol.

### POPPER (Stanford)

An agentic falsification framework (Huang et al., 2025).

- **Better than hyp:** it runs falsification tests on data under statistical error control. hyp records an investigation and executes nothing.
- **Worse for this job:** it is an automation engine, not a record of an investigation.
- **Repository state:** there is no LICENSE file in the repository, though `setup.py` and the PyPI package `popper_agent` declare MIT. The last commit was in May 2025.
- **Complementary:** a POPPER-style runner could record its experiments and results as hyp runs and evidence.

### popper (Kliewer): Falsifiable Hypothesis Compiler

- **Better than hyp:** a stricter hypothesis schema. A hypothesis is rejected unless it names:
  - independent and dependent variables;
  - population and measurement;
  - mechanism;
  - a falsification condition;
  - a quantitative prediction (direction, magnitude window, unit, confidence).

  Every evidence claim must be tagged with a source id.
- **Worse for this job:** it is a research experiment about generating hypotheses from literature, not a general tracker. It has no license file.

### Argdown

- **Better than hyp:** a readable plain-text syntax for argument structure, and polished rendering of argument maps.
- **Worse for this job:** it models the structure of an argument, not evidence, tests and judgments about a tentative claim.

### go-argmap

- **Better than hyp:** very small and keyboard-driven. It rejects and reports cycles among support links, as hyp does for depends-on and supersedes.
- **Worse for this job:**
  - Its nodes are claims, premises, supports and objections only.
  - No sources, falsification or experiments.
  - One JSON or plain-text file per map.

### Arguman

- **Better than hyp:** a long-established, visual web interface for collaborative argument mapping.
- **Worse for this job:**
  - It needs a self-hosted Django 1.7 server with PostgreSQL, MongoDB and Redis (per `requirements.txt`).
  - There is no CLI, only a dated REST API (`web/api/v1`).
  - It is built for debate rather than empirical investigation.
  - No recent activity.

### Analysis of Competing Hypotheses

Richards J. Heuer Jr., *Psychology of Intelligence Analysis*, 1999, chapter 8.

This is the method behind hyp's evidence matrix: weigh every observation against every rival hypothesis, and look for what refutes rather than what fits. hyp adds explicit falsification criteria and a record of judgments.

## Where hyp is weaker overall

- Young (0.x, no tagged releases). CI tests x86_64 Linux and macOS (Apple Silicon and Intel); aarch64 Linux and Windows are untested.
- Local and single-worktree: it handles concurrent processes and browser tabs, but has no networked multi-user editing and no access control.
- No rule that someone other than the author must review a judgment (RigorGraph).
- No quantitative fields on predictions (Kliewer's popper).
- No point-in-time view of the notebook (Epistemos).
- No automated testing of hypotheses (POPPER).

## Ideas worth borrowing

Candidates only; nothing here is a commitment.

- An optional rule that a judgment needs a review by a different author (RigorGraph).
- A required locator, and a retrieval date distinct from the observation date, for cited sources (Doubt).
- Checking that a cited excerpt really appears in the source's bytes (Doubt's `verify`).
- Optional quantitative fields on predictions, such as a magnitude window and unit (Kliewer's popper).
- A point-in-time view of judgments, derived from the assessment history (Epistemos).
