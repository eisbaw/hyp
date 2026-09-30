---
id: HYPO-0057
title: >-
  ID errors: explain zero matches, list ambiguous candidates, name the wrong
  kind
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
Interactive terminal test-drive 2026-09-30 (xterm+tmux, first-time user, release build of fda0faa). `hyp falsify-if bd43 ...` (no H- prefix) says 'ID "bd43" matches 0 objects; use a longer prefix', which can never help; typos get the same advice. An ambiguous prefix ('hyp show H-') says 'matches 2 objects' without listing them. 'hyp predict E-4f2f x' says 'expected a hypothesis' without naming what it got. Related note already on HYPO-0022.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Zero matches: 'no record with ID prefix X' plus a hint about the kind letters when X lacks one
- [ ] #2 Ambiguous: list the candidate IDs with kind and title
- [ ] #3 Wrong kind: name the argument, the expected kind and the kind found
<!-- AC:END -->
