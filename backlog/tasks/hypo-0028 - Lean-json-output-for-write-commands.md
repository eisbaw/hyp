---
id: HYPO-0028
title: Lean --json output for write commands
status: To Do
assignee: []
created_date: '2026-09-30 01:46'
labels:
  - cli
  - agents
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
From the HYPO-0016 architect review. `hyp --json add` (and every write command) prints {"ids": [...], "snapshot": {...}}: the whole project, including every frozen experiment plan. On a real notebook an agent that follows 'use --json for what you parse' fills its context on each write. The skill now tells agents to read plain output for writes; the JSON contract itself should be fixed. Related: HYPO-0024 (hyp apply output size).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Write commands with --json print only the affected IDs (with kinds) and the new project revision; no snapshot
- [ ] #2 hyp apply --json output is equally compact (coordinate with HYPO-0024)
- [ ] #3 Skill and README updated; CLI tests assert the output shape
<!-- AC:END -->
