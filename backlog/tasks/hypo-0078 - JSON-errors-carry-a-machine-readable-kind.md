---
id: HYPO-0078
title: JSON errors carry a machine-readable kind
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 17:08'
updated_date: '2026-09-30 18:30'
labels:
  - agents
  - cli
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Agent wishlist (orchestrator, from driving hyp as an agent). With --json, errors are {"error": "<message>"}; to decide between re-reading and retrying (conflict), fixing the input, or stopping (project blocked), an agent parses message text or relies only on exit codes (1 covers several different situations). Decision-0002 wants a stable machine contract.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 --json errors are {"error", "kind"} with kind one of: conflict, invalid_input, not_found, ambiguous_id, blocked (project has write-blocking errors), io; exit codes unchanged
- [x] #2 Conflict errors also list the IDs whose preconditions failed (an ids array) when known
- [x] #3 README machine contract and skill document the kinds; tests assert kind for each class
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- src/error.rs: ErrorKind {conflict, invalid_input, not_found, ambiguous_id, blocked, io}; `Classified` typed error set at source (Snapshot::find, assert_writable, Store::open, missing experiment/target in commit); `Conflict` moved there with `ids`; io = std::io::Error in chain; default invalid_input. `to_json` used by main.rs and the WebUI ApiError (additive `kind`). Exit codes unchanged.
- ids: stated() (update/patch/archive/delete), stale expected statements, `hyp assess` token conflict; none for --expected-revision / files changed during transaction.
- hyp edit non-interactive rejection keeps the underlying kind.
- Tests: json_errors_carry_a_machine_readable_kind (each kind), api.rs kind/ids asserts, workflow type-not-text check.

- Review round: a missing --project directory is not_found; README says unknown kinds are handled by exit code; apply/patch parse errors hint that every record also takes title, body, tags, archived.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Committed in agent-UX batch 2 (see commit message). Review: QA GO, architect NO-GO (batch-created record as the target of update/patch/archive/delete returned a retry-forever conflict) -> fixed as invalid_input, plus skill without shell variables under 200 lines; confirmation QA GO, architect GO. Gate: 119 Rust tests + DOM, nix flake check.
<!-- SECTION:FINAL_SUMMARY:END -->
