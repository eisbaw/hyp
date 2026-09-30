---
id: HYPO-0078
title: JSON errors carry a machine-readable kind
status: To Do
assignee: []
created_date: '2026-09-30 17:08'
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
- [ ] #1 --json errors are {"error", "kind"} with kind one of: conflict, invalid_input, not_found, ambiguous_id, blocked (project has write-blocking errors), io; exit codes unchanged
- [ ] #2 Conflict errors also list the IDs whose preconditions failed (an ids array) when known
- [ ] #3 README machine contract and skill document the kinds; tests assert kind for each class
<!-- AC:END -->
