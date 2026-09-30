---
id: HYPO-0062
title: hyp export --format markdown is not a readable report
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
Interactive terminal test-drive 2026-09-30 (xterm+tmux, first-time user, release build of fda0faa). The markdown export is flat and sorted by ID prefix (assessments first), not grouped by hypothesis; the heading is the generic 'hyp — research notebook' and ignores config.toml name; it dumps raw YAML incl. the nested frozen plan snapshots (about 350 lines for 2 hypotheses) and prints 'needs review: false'.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Grouped by hypothesis, reusing the plain show rendering
- [ ] #2 Uses the project name; omits frozen snapshots and internal fields
<!-- AC:END -->
