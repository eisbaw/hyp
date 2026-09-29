---
id: HYPO-0014
title: 'README: motivation and comparison with prior art'
status: To Do
assignee: []
created_date: '2026-09-29 22:16'
updated_date: '2026-09-29 22:30'
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
- [ ] #1 Comparison table (hyp vs the tools above, plus backlog.md as the UX inspiration) with columns such as: hypothesis/falsification model, evidence polarity, GUI, CLI, storage/Git-friendliness, agent interface, maturity/licence
- [ ] #2 Each row verified against the live repository (existence, licence, last activity, claimed features); rows that cannot be verified are dropped
- [ ] #3 An honest 'when not to use hyp' paragraph (e.g. statistics, literature mining, networked multi-user)
- [ ] #4 readme-improver review passes with no high-priority findings
- [ ] #5 README opens with a short motivation: hyp is primarily for agents to work in a structured way with tentative, unconfirmed information (hypotheses, falsification criteria, cited evidence) while humans inspect and steer via the WebUI; it works in any directory, Git-friendly but not Git-dependent, like backlog.md
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Architect review: columns like maturity or last-activity go stale quickly; a short 'related tools' list with one-line differences may be better than a wide table.
<!-- SECTION:NOTES:END -->
