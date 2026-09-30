---
id: HYPO-0077
title: Let agents chain records without shell variables
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 17:08'
updated_date: '2026-09-30 18:30'
labels:
  - agents
  - ux
dependencies:
  - HYPO-0053
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Dogfood v2, 2026-09-30 (Claude Code and Codex resuming yesterday's notebook on a timezone bug; both succeeded; friction reported by the agents). Under common Claude Code permission rules, commands with $VAR or $(...) are blocked ('Contains simple_expansion'); the skill's recommended `H1=$(hyp add ...)` capture fails, so the agent copied IDs by hand and skipped recording the experiment and run (too many ID round-trips). Two parts: the skill must not depend on shell variables (show the read-the-printed-ID way first), and hyp apply should accept batch-local references so a multi-record step (hypothesis+criterion+prediction, or experiment+run+evidence) is one command. Related: HYPO-0053 (apply defaults and patch updates).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The skill's main flow works with commands that contain no shell expansion (IDs read from printed output; short prefixes); the variable-capture example is optional
- [x] #2 hyp apply accepts batch-local references (e.g. "id": "@x" on a create and "@x" wherever an ID is expected later in the batch), resolved server-side; documented in apply --help
- [x] #3 A test creates hypothesis, criterion, experiment, run and evidence in one apply batch using local references
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- apply resolves batch-local references server-side (store.rs `References`): a create with "id": "@name" gets a generated full ID; later changes may use "@name" in record ID fields (via `Data::references_mut`, drift-guarded against `references`), update/patch/archive/delete ids and expected.* keys. Unknown, forward and duplicate refs: exit 1, kind invalid_input.
- `written[i].ref` added (only for creates with a reference). Statements naming batch-created records are not checked; a create depending only on batch-created records may omit `expected`.
- Skill: IDs read from output (10-char prefixes) is the primary flow; Example keeps variables with a note; new ```json batch example is executed by test skill_batch_example_applies_as_written. Skill line budget raised 200 -> 220.
- Tests: apply_chains_a_whole_step_with_batch_local_references (red on baseline, red with resolution disabled), references_mut_reaches_every_reference_in_order.

- Review round: updating/patching/archiving/deleting a record created in the same batch (by @ref or full ID) is now invalid_input "… is created by this batch; put its values in the create" (checked in stated() against created), not a conflict. Test changing_a_record_created_in_the_same_batch_is_invalid_input (red: exit 3 conflict before).
- Skill no longer teaches $VAR/$(...): Example is an ID-reading flow; the variable script moved to tests/fixtures/skill_flow.sh, still run by skill_flow_runs_against_this_binary_and_ends_assessed_and_closed; skill_commands_use_no_shell_expansion guards the bash blocks. Budget back to <200 (199 lines). Docs narrowed to "record fields that take an ID and expected keys".
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Committed in agent-UX batch 2 (see commit message). Review: QA GO, architect NO-GO (batch-created record as the target of update/patch/archive/delete returned a retry-forever conflict) -> fixed as invalid_input, plus skill without shell variables under 200 lines; confirmation QA GO, architect GO. Gate: 119 Rust tests + DOM, nix flake check.
<!-- SECTION:FINAL_SUMMARY:END -->
