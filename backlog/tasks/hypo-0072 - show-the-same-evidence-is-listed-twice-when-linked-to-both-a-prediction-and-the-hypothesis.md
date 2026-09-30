---
id: HYPO-0072
title: >-
  show: the same evidence is listed twice when linked to both a prediction and
  the hypothesis
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 17:08'
updated_date: '2026-09-30 17:36'
labels:
  - agents
  - ux
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Dogfood v2, 2026-09-30 (Claude Code and Codex resuming yesterday's notebook on a timezone bug; both succeeded; friction reported by the agents). Plain `hyp show H` listed one observation twice (via its prediction and via a direct link), each with the full body; easy to misread as two observations.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Each evidence record appears once, with all its links/relations listed under it
- [x] #2 Test
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Each observation once in plain show, all its links under it; one `.evidence[]` entry with every link in `.bearings`.
- Test: show_lists_each_observation_once_with_all_its_links (red before: body printed twice).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Committed in agent-UX batch 1 (see commit message). Review: QA GO, architect GO with a meaning fix (Mixed only for for+against; qualifies never changes direction) plus status listing closed hypotheses that need review; confirmation GO from both. Gate: 111 Rust tests + DOM, nix flake check.
<!-- SECTION:FINAL_SUMMARY:END -->
