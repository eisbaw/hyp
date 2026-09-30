---
id: HYPO-0064
title: 'hyp delete: correct advice for referenced records'
status: To Do
assignee: []
created_date: '2026-09-30 12:36'
labels:
  - bug
  - cli
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Interactive terminal test-drive 2026-09-30 (xterm+tmux, first-time user, release build of fda0faa). `hyp delete` on an archived hypothesis that is still referenced says 'object is referenced; archive it instead', although it is already archived, and does not say what refers to it. On an unarchived record it says only 'archive before deleting'.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The referenced error lists the referencing IDs (kind and title) and says those must be deleted first, or that archiving is enough
- [ ] #2 The not-archived error suggests the archive command
<!-- AC:END -->
