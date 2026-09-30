---
id: HYPO-0087
title: 'A stated ID that never existed should be not_found, not conflict'
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 18:30'
updated_date: '2026-09-30 21:10'
labels:
  - agents
  - cli
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
From the agent-UX batch 2 confirmation (architect). In hyp apply, an update/patch/archive/delete or an expected statement naming an ID that never existed returns conflict ('deleted since you read it, or never existed', exit 3). An agent following the contract (conflict: re-read and retry) can loop forever on a typo. Without tombstones the server cannot tell 'deleted' from 'never existed'; but when no record in the pre-batch state even shares the ID's prefix, not_found (exit 1) is the better answer.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A stated ID with no match in the pre-batch state gives kind not_found (exit 1); a record that existed and was deleted is still a conflict when detectable (decide how; document the limit)
- [x] #2 Test: a typo'd ID in apply returns not_found
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. stated(): look up in the batch state; if absent and absent before the batch -> not_found (exit 1); deleted earlier in the batch -> invalid_input.
2. stale_statements: an expected ID with no record before the batch -> not_found.
3. Decision: an absent full ID is not_found too (no tombstones: deleted and never-existed are indistinguishable); document in README.
4. Tests: typo'd ID in patch/delete and in expected -> not_found, exit 1.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- stated() and expected lookups: an ID matching no record before the batch is kind not_found (exit 1, HTTP 422), full ID or prefix.
- Decision: an absent full ID is not_found too. Without tombstones deleted-since-read and never-existed are indistinguishable; the old conflict made a typo loop forever, and a retry cannot succeed in either case. Documented in README Preconditions and the not_found kind row.
- Naming a record deleted by an earlier change of the same batch is invalid_input; expected.hypotheses naming a non-hypothesis is invalid_input.
- Tests: workflow stated_records_that_do_not_exist_are_not_found (typo patch/archive/delete/expected, and deleted-since-read), run_is_a_conflict... tail updated; cli apply_with_a_mistyped_id_is_not_found_not_a_conflict.

- Review round: not_found errors carry ids (Classified::not_found; error::to_json prints ids of the deciding Conflict or Classified). CLI ID lookups (Snapshot::find) carry them too.
- A create whose reference fields name an ID matching nothing before or within the batch is not_found (store::references_exist, before check_create). A prefix or wrong-kind reference is still reported by validate.
- Test: cli a_create_naming_an_id_that_matches_nothing_is_not_found_with_its_ids; apply_with_a_mistyped_id asserts ids. README not_found row and Preconditions, skill exit codes updated.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Committed in the schema-evolution batch (0.2.0; see commit message). Deep review: QA GO (4 scenarios, 7 extra DOM runs clean), architect GO; small fix round (min_hyp_version in Config, falsified check skipped on missing evidence, not_found with ids, apply creates with unmatched references not_found); confirmation GO from both. Codex review not run: the configured Codex model is rejected for the account. Gate: 130 Rust tests + DOM, nix flake check.
<!-- SECTION:FINAL_SUMMARY:END -->
