---
id: HYPO-0092
title: 'WebUI: surface unexplained observations and what an observation bears on'
status: In Progress
assignee:
  - '@implementer-A'
created_date: '2026-09-30 21:25'
updated_date: '2026-10-01 09:05'
labels:
  - webui
  - agents
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
HYPO-0091 added observation-first work to the CLI: hyp observe, hyp add --explains/--competes-with, unexplained observations in hyp status (Snapshot::unexplained) and a Bears on section in hyp show E-… (Snapshot::bears_on). The WebUI shows none of it: its overview does not list unexplained observations, an evidence page does not say which hypotheses it bears on and how, and there is no way to create a hypothesis that explains an existing observation in one step.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The WebUI overview lists unexplained observations (the same set as hyp status)
- [x] #2 An evidence page lists the hypotheses it bears on with stance and links, and says when none explains it
- [x] #3 A hypothesis can be created from an observation page, linked to it (supports) in the same write
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- src/web.rs: the WebUI snapshot (GET /api/snapshot, the POST /api/transaction answer, the HTML export) adds unexplained: the IDs of Snapshot::unexplained, through a WebSnapshot wrapper (serde flatten), so the UI does not re-derive the rule. hyp export --format json is unchanged.
- Overview: 'Unexplained observations' section with an 'Explain it' button each. Evidence page: 'Bears on' lists each hypothesis with stance, judgment, lifecycle and each link's meaning (with Edit interpretation), says when no live hypothesis accounts for it, and offers 'Explain with a new hypothesis'. Links from the evidence that bear on no hypothesis are listed under 'Other interpretations'.
- A hypothesis created from an evidence page is saved in one transaction with a supports link from the evidence (title and default reason as hyp add --explains).
- Tests: tests/api.rs snapshot_lists_unexplained_observations; DOM test observations() compares the overview set with hyp --json status before and after, checks Bears on and the one-write link. Red without each.

- After review: the observation a new hypothesis explains is fixed when the form opens, so the save always sends the link and fails (the server names the missing ID) if the observation was deleted meanwhile, instead of creating the hypothesis without it. DOM test covers it (red with a save-time lookup).
- Also in this change: the page re-reads every 2 s while its last read failed or showed blocking diagnostics, and drops answers older than the latest read; this fixed the intermittent 'recovery' timeouts of the DOM test (a state only the page's read saw got no server event afterwards).
<!-- SECTION:NOTES:END -->
