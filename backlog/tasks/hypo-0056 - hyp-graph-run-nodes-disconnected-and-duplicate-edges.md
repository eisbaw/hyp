---
id: HYPO-0056
title: 'hyp graph: run nodes disconnected and duplicate edges'
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
Interactive terminal test-drive 2026-09-30 (xterm+tmux, first-time user, release build of fda0faa). `hyp graph` emits run nodes with no experiment->run edge, and duplicate links produce duplicate edges.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every run has an edge from its experiment
- [ ] #2 Identical edges are emitted once
- [ ] #3 Test on the demo notebook
<!-- AC:END -->
