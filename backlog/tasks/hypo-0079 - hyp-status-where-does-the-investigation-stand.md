---
id: HYPO-0079
title: 'hyp status: where does the investigation stand'
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 17:08'
updated_date: '2026-09-30 17:37'
labels:
  - agents
  - cli
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Agent wishlist (orchestrator). An agent resuming work must reconstruct the state from hyp list plus several hyp show calls. Both dogfood v2 agents spent their first turns doing exactly that. One compact command should answer: what needs review, what is open, and what is missing.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 hyp status prints, per active hypothesis: judgment, needs-review, missing criterion (or untestable reason), linked evidence count, open gaps, experiments without runs; plus project-level write-blocking diagnostics
- [x] #2 --json gives the same structure; plain output fits in ~40 lines for a 10-hypothesis notebook
- [x] #3 The skill tells agents to start with hyp status
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- New src/status.rs: `report(&Snapshot)` + `plain`. Active = not archived, not closed (counted as not shown). Needs-review first. Blocking diagnostics listed right after the header. No review token printed.
- JSON: {hypotheses[{id,title,lifecycle,judgment,confidence,needs_review,criteria,untestable_reason,missing_criterion,linked_evidence,open_gaps,experiments_without_runs}], not_shown{closed,archived}, writes_blocked, blocking, errors, warnings, revision}.
- Skill: Method step 1 and Example start with `hyp status`.
- Tests: status_shows_where_each_active_hypothesis_stands, status_reports_diagnostics_that_block_writes, status_of_ten_hypotheses_fits_in_forty_lines (all red before: unknown subcommand).

- Review fix: closed hypotheses that need review are listed (repro: close, then new evidence on its prediction); `not_shown.needs_review` counts hidden (archived) ones. "open" replaces "active"; with none open the header points to `hyp list` for the conclusions. `hyp init --demo` now suggests `hyp status` instead of the empty-project hint.
- Tests: status_lists_a_closed_hypothesis_that_needs_review, init_demo_does_not_suggest_starting_from_an_empty_project (both red before).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Committed in agent-UX batch 1 (see commit message). Review: QA GO, architect GO with a meaning fix (Mixed only for for+against; qualifies never changes direction) plus status listing closed hypotheses that need review; confirmation GO from both. Gate: 111 Rust tests + DOM, nix flake check.
<!-- SECTION:FINAL_SUMMARY:END -->
