---
id: HYPO-0063
title: >-
  hyp edit: keep the user's edits when validation fails; refuse immutable kinds
  before opening
status: To Do
assignee: []
created_date: '2026-09-30 12:36'
labels:
  - cli
  - ux
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Interactive terminal test-drive 2026-09-30 (xterm+tmux, first-time user, release build of fda0faa). When `hyp edit` fails validation the temp file is deleted and all edits are lost; YAML errors report line numbers relative to the front matter, not the file. `hyp edit R-...`/`A-...` opens the editor and only afterwards says assessments and runs are immutable. Changing `kind:` gives a misleading 'unknown field scope' error.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 On a validation error the editor reopens with the error as a comment at the top (like git commit), or the temp file is kept and its path printed
- [ ] #2 Line numbers refer to the file the user edited
- [ ] #3 Immutable kinds are refused before launching the editor
- [ ] #4 A changed kind gives 'cannot change the kind of a record'
<!-- AC:END -->
