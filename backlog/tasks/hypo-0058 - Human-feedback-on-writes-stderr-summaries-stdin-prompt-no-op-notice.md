---
id: HYPO-0058
title: 'Human feedback on writes: stderr summaries, stdin prompt, no-op notice'
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
Interactive terminal test-drive 2026-09-30 (xterm+tmux, first-time user, release build of fda0faa). Plain write commands print only bare IDs: `evidence add` prints two unlabelled lines (E-, then an unexplained L-); delete/archive/restore, a flagless `hyp set H` and an unchanged `hyp edit` all print just the ID, like a real change. `hyp restore` of a non-archived record succeeds silently. `hyp add -` shows no prompt and just waits. Stdout IDs are the machine contract (keep them).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 When stderr is a TTY, write commands print a one-line human summary to stderr (e.g. 'created evidence E-... (+ link L-... supports H-...)'); stdout is unchanged
- [ ] #2 No-op writes say 'no changes' (stderr) and exit 0; restore of a non-archived record says so
- [ ] #3 Reading a text argument from a TTY stdin prints a one-line hint to stderr
<!-- AC:END -->
