---
id: HYPO-0104
title: 'Snapshot: each record''s references with role names'
status: To Do
assignee: []
created_date: '2026-10-01 18:58'
labels:
  - webui
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
roles() in web/app.js keeps its own list of which field of which kind references another record and under what role (hypothesis under test, plan, cited evidence, evidence considered, falsification criterion, supersedes, resolved by; HYPO-0045). The server already knows each kind's reference fields (Record::references). The two lists drift when a kind or reference field is added, as data references were (HYPO-0090), and the HTML export, which renders offline from the snapshot, inherits the JS copy.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The snapshot (/api/snapshot, the transaction answer, the HTML export) gives each record's references with a role name, from one server-side definition
- [ ] #2 The WebUI renders its role sections from that data and keeps no per-kind list of reference fields
- [ ] #3 A test fails when a kind gains a reference field without a role
- [ ] #4 README documents the field as an additive snapshot change
<!-- AC:END -->
