---
id: HYPO-0117
title: >-
  hyp status: take unexplained observations from the snapshot, not a second
  computation
status: To Do
assignee: []
created_date: '2026-10-01 19:00'
labels:
  - hardening
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Snapshot::unexplained_observations (IDs, derived once in derive(), 5c49f6c) is what /api/snapshot, the transaction answer and hyp export carry. src/status.rs calls s.unexplained() again to build its own list. Both come from the same function today, but two computations of one set invite drift (e.g. a filter added to one).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 hyp status builds its unexplained list from Snapshot::unexplained_observations
- [ ] #2 A test checks that hyp --json status and the snapshot list the same IDs in the same order
<!-- AC:END -->
