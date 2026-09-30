---
id: HYPO-0075
title: assess --help lists judgments and what each requires
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 17:08'
updated_date: '2026-09-30 17:36'
labels:
  - agents
  - docs
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Dogfood v2, 2026-09-30 (Claude Code and Codex resuming yesterday's notebook on a timezone bug; both succeeded; friction reported by the agents). An agent only learned from the skill that 'falsified' needs --criterion and that other judgments need linked evidence; hyp assess --help lists the values but not their requirements, and a bad value's error does not explain them.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 assess --help: one line per judgment with its requirements (untested: none; inconclusive/supported/weakened: linked evidence; falsified: linked evidence + --criterion)
- [x] #2 The error for a missing requirement names the judgment's rule
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- `Judgment::requirement` is the single source: possible-value help of `assess --status` and `Judgment::rule()`, which the missing-evidence and new falsified-without-criterion errors (validate_new) quote.
- Test: assess_help_lists_each_judgment_with_what_it_requires (red before).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Committed in agent-UX batch 1 (see commit message). Review: QA GO, architect GO with a meaning fix (Mixed only for for+against; qualifies never changes direction) plus status listing closed hypotheses that need review; confirmation GO from both. Gate: 111 Rust tests + DOM, nix flake check.
<!-- SECTION:FINAL_SUMMARY:END -->
