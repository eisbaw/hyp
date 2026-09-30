---
id: HYPO-0073
title: Say whether evidence on a prediction or criterion counts for the hypothesis
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 17:08'
updated_date: '2026-09-30 17:36'
labels:
  - agents
  - docs
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Dogfood v2, 2026-09-30 (Claude Code and Codex resuming yesterday's notebook on a timezone bug; both succeeded; friction reported by the agents). Agents could not tell whether `hyp evidence add P-...` counts for H, so they added a second link to H 'just in case'. It does count (linked_evidence includes evidence linked to active criteria/predictions), but neither --help nor the skill says so.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 evidence add --help and the skill state that evidence linked to a criterion or prediction is part of the hypothesis's basis and citable
- [x] #2 hyp show makes the path visible (see the 'meets criterion' task)
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- `evidence add --help`, `link --help` (long about and target arg) and the skill say evidence on an active criterion/prediction is in the basis and citable, and that supporting a criterion counts against H.
- Test: help_and_skill_say_evidence_on_a_criterion_or_prediction_counts (text new in this change); the path in show is covered by HYPO-0071 tests.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Committed in agent-UX batch 1 (see commit message). Review: QA GO, architect GO with a meaning fix (Mixed only for for+against; qualifies never changes direction) plus status listing closed hypotheses that need review; confirmation GO from both. Gate: 111 Rust tests + DOM, nix flake check.
<!-- SECTION:FINAL_SUMMARY:END -->
