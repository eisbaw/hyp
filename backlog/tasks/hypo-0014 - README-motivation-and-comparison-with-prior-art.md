---
id: HYPO-0014
title: 'README: motivation and comparison with prior art'
status: Done
assignee:
  - '@claude'
created_date: '2026-09-29 22:16'
updated_date: '2026-10-01 19:02'
labels:
  - docs
dependencies:
  - HYPO-0015
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The README explains what hyp does but not why it exists or how it differs from similar tools. The motivating conversation (ChatGPT share: https://chatgpt.com/share/6abaaa09-4cc8-83ed-a649-3644c7ab9e4d) asked for "backlog.md, but for hypotheses": a CRUD tool with a CLI and GUI for hypotheses, evidence and falsification, not a statistics package. It surveyed these tools:
- Doubt (github.com/alsoleg89/doubt): question -> claims/observations/unknowns, evidence supports/contradicts/qualifies/missing, zero-dependency CLI plus a self-contained HTML GUI.
- RigorGraph (github.com/f0909172434/rigorgraph): local-first claim -> evidence -> verification in JSONL, deterministic audit, GitHub Action, HTML report.
- Epistemos (github.com/Voltolini-SPACE/epistemos): an epistemic database with a claim lifecycle, a web panel, bitemporal history, SDK/REST/MCP.
- go-argmap (github.com/BaseMax/go-argmap): a small Go TUI for argument trees with JSON storage.
- Arguman (github.com/arguman/arguman.org): a mature self-hosted web app for argument mapping.
- POPPER (github.com/snap-stanford/POPPER): an agentic engine that runs sequential falsification experiments, not a CRUD tool.
- popper (github.com/kliewerdaniel/popper): a falsifiable-hypothesis "compiler" whose schema is a strict hypothesis schema.
The search also surfaced ach-workbench (Analysis of Competing Hypotheses), falsification-ledger, falsify, ReproDeck and honest-signal, which are worth a look.

The ChatGPT star ratings are unverified LLM output. Several of these repos are recent and small. Every row in the table must be checked against the actual repository before it is published.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Comparison table (hyp vs the tools above, plus backlog.md as the UX inspiration) with columns such as: hypothesis/falsification model, evidence polarity, GUI, CLI, storage/Git-friendliness, agent interface, maturity/licence
- [x] #2 Each row verified against the live repository (existence, licence, last activity, claimed features); rows that cannot be verified are dropped
- [x] #3 An honest 'when not to use hyp' paragraph (e.g. statistics, literature mining, networked multi-user)
- [x] #4 readme-improver review passes with no high-priority findings
- [x] #5 README opens with a short motivation: hyp is primarily for agents to work in a structured way with tentative, unconfirmed information (hypotheses, falsification criteria, cited evidence) while humans inspect and steer via the WebUI; it works in any directory, Git-friendly but not Git-dependent, like backlog.md
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Architect review: columns like maturity or last-activity go stale quickly; a short 'related tools' list with one-line differences may be better than a wide table.

- README opening rewritten: backlog.md, but for hypotheses; agents primary (decision-0002); tool-enforced rules; VCS-optional (decision-0001); pointer to the new sections.
- New sections at the end: When not to use hyp, Conceptual model (direction of fit: Anscombe 1957, Searle 1983, Zave and Jackson TOSEM 6(1) 1997; epistemic-status axis), Related tools, License.
- Related tools is a list with one-line differences instead of a wide table (architect note): maturity, stars and last-activity columns go stale.
- Verified through the hosting API (existence, licence, description) and each README, October 2026: backlog.md MIT; Doubt MIT; RigorGraph MIT; Epistemos MIT; POPPER (Stanford) MIT in setup.py, no LICENSE file; popper (kliewerdaniel) no licence; go-argmap MIT; Arguman GPL-3.0 per README (API reports NOASSERTION). None dropped.
- Not looked up: ach-workbench, falsification-ledger, falsify, ReproDeck, honest-signal (no repository URLs recorded).
- readme-improver self-audit: no high findings; decision references now link to their files.

- AC 1 left unchecked: the comparison is a list with one-line differences, not a table, following the architect note. Edit the AC or accept the list.
- AC 4: the readme-improver rubric was applied by this agent itself (no separate sub-agent fan-out); no high findings in the opening or the new sections.

- Review round: Related tools cut to backlog.md, Heuer's Analysis of Competing Hypotheses (1999, the methodological precedent), POPPER, Doubt and Argdown (MIT, active; replaces Arguman, whose last commit is from 2021). Dropped RigorGraph, Epistemos, popper (kliewerdaniel), go-argmap and Arguman as small or dormant. When not to use hyp now names categories and links Scope of this release for the technical limits. Conceptual model: Searle cited for A Taxonomy of Illocutionary Acts (1975; Expression and Meaning, 1979), normative and imperative share world-to-word fit and differ by persistence, the status axis branches (refuted) and anything can go stale.
- AC 4 remains a self-audit against the readme-improver rubric, not a separate review.

Closing review 2026-10-01: AC #1 checked. docs/related-tools.md (e4e4ecc) has a summary table of hyp against backlog.md and the tools above (Doubt, RigorGraph, Epistemos, POPPER, Kliewer's popper, Argdown, go-argmap, Arguman, plus Heuer's ACH), linked from the README Related tools section. Its columns are what the tool is and where it overlaps with hyp; the dimensions the AC names (falsification model, evidence polarity, GUI/CLI, storage, agent interface, licence) are covered per tool in prose rather than as columns, following the architect note that maturity and activity columns go stale. AC #4 was a self-audit against the readme-improver rubric, not a separate reviewer run; e4e4ecc additionally had an mped-architect fact-check (NO-GO with corrections, all applied, then GO).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The README says why hyp exists and when not to use it, and hyp is compared with related tools in docs/related-tools.md.

Changes (193f872, a6d5c1c, 76f1c94, e4e4ecc):
- README opening: "backlog.md, but for hypotheses"; agents as primary users with a stable --json contract (decision-0002); discipline enforced as tool rules; Markdown records, Git-friendly but not Git-dependent (decision-0001); observation-first work.
- New README sections: When not to use hyp (by category, linking Scope of this release), Conceptual model (direction of fit: Anscombe, Searle, Zave and Jackson; the epistemic-status axis), Related tools (short list), License.
- docs/related-tools.md: summary table plus per-tool strengths and weaknesses against hyp for backlog.md, Doubt, RigorGraph, Epistemos, POPPER, Kliewer's popper, Argdown, go-argmap, Arguman and ACH, and ideas worth borrowing; checked against each repository in October 2026 (licence, activity, documented features); no tool was installed or run.

Review: readme-improver rubric applied by the authoring agent (no high findings); mped-architect fact-check on e4e4ecc (18 corrections applied, then GO). Gate: just fmt-check, lint, e2e exit 0.

Caveat: the comparison describes documented features, not tested ones, and dates quickly; the doc says so.
<!-- SECTION:FINAL_SUMMARY:END -->
