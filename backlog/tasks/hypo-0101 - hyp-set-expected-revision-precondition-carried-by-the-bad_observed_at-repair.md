---
id: HYPO-0101
title: 'hyp set: expected-revision precondition, carried by the bad_observed_at repair'
status: To Do
assignee: []
created_date: '2026-10-01 18:58'
labels:
  - contract
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The bad_observed_at repair that hyp check prints is hyp set E --body=<body + "Observed at (as recorded): ..."> --observed-at= (8bd8ba2). It carries the body as hyp check read it, and hyp set has no expected-revision precondition, so a body edited between hyp check and running the repair is silently overwritten. Today the repair note only tells the agent to run hyp check again right before running it (a15f73a). Per decision-0002 the tool should enforce this rather than say it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 hyp set accepts the expected revision of the record it changes; a mismatch exits 3 (conflict) and writes nothing
- [ ] #2 The bad_observed_at repair (plain hyp check, --json argv, WebUI and HTML export) includes the record's revision
- [ ] #3 Test: a body edited between hyp check and running its repair makes the repair exit 3, and the edit is kept
- [ ] #4 README machine contract and the agent skill document the flag
<!-- AC:END -->
