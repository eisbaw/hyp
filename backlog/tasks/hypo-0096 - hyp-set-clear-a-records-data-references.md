---
id: HYPO-0096
title: 'hyp set: clear a record''s data references'
status: To Do
assignee: []
created_date: '2026-09-30 22:50'
labels:
  - cli
  - agents
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found in HYPO-0090. hyp set ID --data D-... replaces the list, but there is no way to empty it from the CLI; only a hyp apply patch with "set": {"data": []} does. Same shape as --by on gaps.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A documented hyp set flag or value empties a record's data references, and a test covers it
<!-- AC:END -->
