---
id: HYPO-0074
title: Review tokens that are easy to take from the reviewed output
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
Dogfood v2, 2026-09-30 (Claude Code and Codex resuming yesterday's notebook on a timezone bug; both succeeded; friction reported by the agents). Both agents found copying 64-hex review tokens the most tedious step; with two hypotheses in one output they had to pick the right token out of a long listing. Codex asked for 'a command that safely uses the reviewed state directly'. Constraint: the token must come from the output the agent reviewed (do not add a token-only command that tempts assessing without reading).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Plain show prints the 12-hex short token prominently in its header line (e.g. 'H-02c7fa80  review 46d8d8f5c79b'); --json keeps the full token
- [x] #2 The skill's example uses the short token
- [x] #3 Test that the short token printed by show is accepted by assess
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Plain show line 1: `H-…  hypothesis  review <12 hex>`; the full `review token:` line is gone (one token to copy). `--json` keeps .state.review_token. Skill Example extracts with sed from line 1; README, assess --reviewed help and its error updated.
- Test: show_header_prints_the_short_review_token_that_assess_accepts (red before). Existing token tests updated.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Committed in agent-UX batch 1 (see commit message). Review: QA GO, architect GO with a meaning fix (Mixed only for for+against; qualifies never changes direction) plus status listing closed hypotheses that need review; confirmation GO from both. Gate: 111 Rust tests + DOM, nix flake check.
<!-- SECTION:FINAL_SUMMARY:END -->
