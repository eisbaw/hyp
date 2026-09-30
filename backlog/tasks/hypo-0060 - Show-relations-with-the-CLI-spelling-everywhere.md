---
id: HYPO-0060
title: Show relations with the CLI spelling everywhere
status: To Do
assignee: []
created_date: '2026-09-30 12:36'
labels:
  - cli
  - ux
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Interactive terminal test-drive 2026-09-30 (xterm+tmux, first-time user, release build of fda0faa). The CLI requires `competes-with`, but `list --all`, `show L-...`, `graph` and `export` print `competes_with`. Users copy what they see. Files keep underscores (format unchanged).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every human-facing output uses the hyphenated CLI spelling
- [ ] #2 JSON keeps the serialized form (document)
<!-- AC:END -->
