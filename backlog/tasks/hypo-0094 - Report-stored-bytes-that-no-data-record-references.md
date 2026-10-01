---
id: HYPO-0094
title: Report stored bytes that no data record references
status: To Do
assignee: []
created_date: '2026-09-30 22:50'
updated_date: '2026-10-01 00:57'
labels:
  - storage
  - agents
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found in HYPO-0090. hyp capture stores the bytes in hyp/assets/<sha256> before it commits the record (content-addressed, outside the lock), so a capture whose commit fails (blocked project, invalid title or media type) leaves an unreferenced file, and deleting a data record keeps its bytes by design. Nothing reports these files; the README says they are retained rather than garbage-collected. An agent or human cannot tell kept history from debris.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 hyp check reports files in hyp/assets/ that no data record (and no legacy attachment) names, as a warning with a repair note
- [ ] #2 A documented, explicit way removes them; nothing is removed implicitly
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From the HYPO-0004 scout: after 50 injected writer kills, 23 .tmp* files were left under hyp/ and .hyp/ (reads ignore them; they accumulate and git add -A would commit those under hyp/). Report and offer cleanup in hyp check.
<!-- SECTION:NOTES:END -->
