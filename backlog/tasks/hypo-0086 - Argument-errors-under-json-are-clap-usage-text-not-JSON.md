---
id: HYPO-0086
title: 'Argument errors under --json are clap usage text, not JSON'
status: To Do
assignee: []
created_date: '2026-09-30 17:54'
labels:
  - agents
  - cli
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while implementing HYPO-0078. With --json, errors from the command-line parser (unknown flag, invalid value, list filter conflicts; exit 2) are printed as clap's plain usage text, so an agent parsing stderr as {"error","kind"} fails on them. Exit code 2 still distinguishes them.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 With --json, an argument error prints the JSON error shape with a documented kind on stderr and still exits 2
- [ ] #2 README machine contract documents it
<!-- AC:END -->
