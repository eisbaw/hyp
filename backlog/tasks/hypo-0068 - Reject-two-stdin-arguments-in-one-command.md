---
id: HYPO-0068
title: Reject two '-' (stdin) arguments in one command
status: To Do
assignee: []
created_date: '2026-09-30 13:31'
labels:
  - cli
  - bug
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while doing HYPO-0055. `hyp add - --body -` (or set --title - --body -) reads stdin twice: the second argument silently gets an empty string. Fail fast with an argument error instead.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A command given '-' for more than one text argument exits 2 naming the arguments
- [ ] #2 Test with two '-' arguments
<!-- AC:END -->
