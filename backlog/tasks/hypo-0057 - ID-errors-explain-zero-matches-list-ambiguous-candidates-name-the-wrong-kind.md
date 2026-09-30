---
id: HYPO-0057
title: >-
  ID errors: explain zero matches, list ambiguous candidates, name the wrong
  kind
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 12:36'
updated_date: '2026-09-30 13:56'
labels:
  - cli
  - ux
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Interactive terminal test-drive 2026-09-30 (xterm+tmux, first-time user, release build of fda0faa). `hyp falsify-if bd43 ...` (no H- prefix) says 'ID "bd43" matches 0 objects; use a longer prefix', which can never help; typos get the same advice. An ambiguous prefix ('hyp show H-') says 'matches 2 objects' without listing them. 'hyp predict E-4f2f x' says 'expected a hypothesis' without naming what it got. Related note already on HYPO-0022.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Zero matches: 'no record with ID prefix X' plus a hint about the kind letters when X lacks one
- [x] #2 Ambiguous: list the candidate IDs with kind and title
- [x] #3 Wrong kind: name the argument, the expected kind and the kind found
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Snapshot::find: "no record with ID (prefix) X" plus the kind-letter list when X has none; ambiguous lists up to 10 "ID  kind  title" lines (model::listing). cli::find_kind names the argument (<HYPOTHESIS>, <EXPERIMENT>, <FROM>, <TO>, --evidence, --criterion, --targets) and expected/actual kind.
- Test: id_errors_explain_no_match_list_candidates_and_name_the_wrong_kind.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Committed in human CLI batch 1 (see commit message). Review: QA GO and architect GO; small cleanup round (wrap_help dropped, web relation spelling, --project help, multi-line-title repair note); confirmation QA GO, architect GO. Gate: 90 Rust tests + DOM, nix flake check.
<!-- SECTION:FINAL_SUMMARY:END -->
