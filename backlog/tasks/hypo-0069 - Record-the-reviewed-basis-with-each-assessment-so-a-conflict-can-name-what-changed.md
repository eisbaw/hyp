---
id: HYPO-0069
title: >-
  Record the reviewed basis with each assessment so a conflict can name what
  changed
status: To Do
assignee: []
created_date: '2026-09-30 16:18'
labels:
  - cli
  - agents
  - review
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
From HYPO-0059 (human CLI batch 2). An assessment conflict (exit 3) cannot name the records that changed since the review: the reviewer's basis is not stored, only its fingerprint (based_on) and the review token. The message now says to compare `hyp show H` with what was reviewed. Storing the basis snapshot (or the IDs and revisions of its entries) with each assessment would let `show` and conflicts say 'changed since the current assessment: E-..., L-...'. Decide what to store (size vs. precision), whether it goes in the assessment file or is derived, and how it interacts with the fingerprint (decision-0003).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 An assessment conflict names the records added, removed or changed since the current assessment
- [ ] #2 Plain and JSON show of a needs-review hypothesis name what changed since its current assessment
- [ ] #3 The storage choice and its size cost are recorded as a decision
<!-- AC:END -->
