---
id: HYPO-0052
title: 'Review basis v2: follow-ups from the confirmation review'
status: To Do
assignee: []
created_date: '2026-09-30 11:59'
labels:
  - hardening
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Non-blocking findings from the review basis v2 confirmation (architect): (1) the basis_growth rejection message always says '(a new or restored link or run) ... link pre-existing evidence in an earlier write', which is wrong advice for a moved experiment or a new run citing old evidence: say 'make that change in an earlier write'. (2) A batch that assesses and then archives one of its cited links is accepted and flags the assessment immediately; consider rejecting it. (3) observed_at is provenance but not in the basis; decide with HYPO-0043. (4) Attachment hashes are hashed in stored order, so reordering attachments flags the assessment; sort them. (5) The review_token format (ID:revision lines) has no golden test. (6) Round-1 notes in HYPO-0009 and HYPO-0040 were superseded by round 2.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The basis_growth message gives correct advice for every kind of growth
- [ ] #2 Attachment hashes are sorted in the projection; a golden test pins review_token
<!-- AC:END -->
