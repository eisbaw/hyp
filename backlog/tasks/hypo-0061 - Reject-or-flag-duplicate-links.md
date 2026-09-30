---
id: HYPO-0061
title: Reject or flag duplicate links
status: To Do
assignee: []
created_date: '2026-09-30 12:36'
labels:
  - cli
  - validation
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Interactive terminal test-drive 2026-09-30 (xterm+tmux, first-time user, release build of fda0faa). The same link twice, and a reverse competes-with pair, are accepted silently; `hyp check` does not flag them; plain show lists the evidence twice.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Creating a link identical (from, to, relation) to an active link is an ordinary error naming the existing link
- [ ] #2 competes-with is treated as symmetric for duplicate detection
- [ ] #3 hyp check warns on existing duplicates
<!-- AC:END -->
