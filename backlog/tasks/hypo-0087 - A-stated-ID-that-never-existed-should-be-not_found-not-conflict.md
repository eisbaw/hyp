---
id: HYPO-0087
title: 'A stated ID that never existed should be not_found, not conflict'
status: To Do
assignee: []
created_date: '2026-09-30 18:30'
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
- [ ] #1 A stated ID with no match in the pre-batch state gives kind not_found (exit 1); a record that existed and was deleted is still a conflict when detectable (decide how; document the limit)
- [ ] #2 Test: a typo'd ID in apply returns not_found
<!-- AC:END -->
