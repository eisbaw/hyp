---
id: HYPO-0088
title: hyp check reports records that need a newer schema than config.toml states
status: To Do
assignee: []
created_date: '2026-09-30 19:54'
labels:
  - schema
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found in HYPO-0085. A write raises hyp/config.toml to the schema its records need, but only on the next write. A merge or sync that brings a gap with resolved_by while config.toml still says 1 (or a hand edit lowering it) leaves a notebook that claims schema 1: hyp 0.1.0 would open it and report the gap as malformed instead of refusing up front, which decision-0004 wants to avoid.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 hyp check reports a record whose Data::schema exceeds the notebook's schema_version, with a repair that raises it
- [ ] #2 Test: a schema-1 config with a resolved_by gap is reported
<!-- AC:END -->
