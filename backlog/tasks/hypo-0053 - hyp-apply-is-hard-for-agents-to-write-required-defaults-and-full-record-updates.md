---
id: HYPO-0053
title: >-
  hyp apply is hard for agents to write: required defaults and full-record
  updates
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 12:18'
updated_date: '2026-09-30 18:30'
labels:
  - agents
  - cli
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
From the CLI ergonomics QA (2026-09-30). Creating a hypothesis via hyp apply fails with serde's 'missing field lifecycle' (no default; the CLI sets draft). An apply update must repeat the full stored record, including created_at, or fails with 'cannot change creation time', so an agent must round-trip the whole record to change one field. HYPO-0027 already notes that the apply format is only documented in the uninstalled README.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Create inputs default lifecycle (draft), status (planned) and other fields the CLI defaults, so a minimal create works
- [x] #2 A patch-style update op changes only the given fields (with expected_revision), or update keeps stored created_at when it is omitted
- [x] #3 hyp apply --help shows a minimal create and update example
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Serde defaults (model.rs new_lifecycle/new_experiment_status/new_outcome, shared with the CLI): lifecycle draft, experiment status planned, run outcome observed; an experiment create gets its hypothesis as a target as `hyp experiment add` does (replaces the "must include its hypothesis" error with the precise missing-statement one).
- New op {"op":"patch","id","expected_revision","set":{...}}: merges into the stored record, validated as an update (check_update shared); id/kind/created_at/updated_at refused; assessments and runs refused. update unchanged.
- Not defaulted: link title (the CLI generates one), evidence observed_at (the CLI uses now). Filed separately.
- Test: apply_creates_with_cli_defaults_and_patches_only_given_fields.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Committed in agent-UX batch 2 (see commit message). Review: QA GO, architect NO-GO (batch-created record as the target of update/patch/archive/delete returned a retry-forever conflict) -> fixed as invalid_input, plus skill without shell variables under 200 lines; confirmation QA GO, architect GO. Gate: 119 Rust tests + DOM, nix flake check.
<!-- SECTION:FINAL_SUMMARY:END -->
