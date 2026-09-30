---
id: HYPO-0055
title: Multi-line stdin title breaks one-record-per-line output
status: To Do
assignee: []
created_date: '2026-09-30 12:36'
labels:
  - bug
  - cli
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Interactive terminal test-drive 2026-09-30 (xterm+tmux, first-time user, release build of fda0faa). `hyp add -` with multi-line stdin stores a title containing a newline; `hyp list` and `list | head/cut` then break across lines and show prints 'title: Cron starts two backups\n  second line body?'. Applies to every command taking a title via '-'.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Titles cannot contain newlines: either an ordinary error, or the first line becomes the title and the rest the body (decide, document)
- [ ] #2 validate() rejects stored titles with newlines so hand-edited files are caught by hyp check
- [ ] #3 Test with multi-line stdin
<!-- AC:END -->
