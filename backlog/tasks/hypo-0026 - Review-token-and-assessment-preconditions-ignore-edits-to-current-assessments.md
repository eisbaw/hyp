---
id: HYPO-0026
title: Review token and assessment preconditions ignore edits to current assessments
status: To Do
assignee: []
created_date: '2026-09-30 01:18'
labels:
  - concurrency
  - agents
dependencies:
  - HYPO-0002
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
From the HYPO-0002 Codex round-3 review (finding 3). Assessments are immutable through the tool, but a hand edit, sync or merge can change a current assessment's judgment or rationale under the same ID. review_token and expected.hypotheses[H].assessment_ids compare IDs only, so a reviewer holding an old token can supersede a changed assessment without having seen it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 review_token and the server-side check cover the revisions of the current assessments, not only their IDs
- [ ] #2 Test: an on-disk edit of the current assessment makes an old token exit 3
<!-- AC:END -->
