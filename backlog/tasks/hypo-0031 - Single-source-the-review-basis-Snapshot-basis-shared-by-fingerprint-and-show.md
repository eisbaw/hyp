---
id: HYPO-0031
title: 'Single-source the review basis: Snapshot::basis shared by fingerprint and show'
status: To Do
assignee: []
created_date: '2026-09-30 02:08'
labels:
  - hardening
  - refactor
dependencies:
  - HYPO-0016
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
From the HYPO-0016 final architect check. The Show arm in src/cli.rs repeats the 'filter objects by relevant(id), pair ID with revision' logic of Snapshot::fingerprint_of (src/model.rs). If the fingerprint changes (e.g. skipping archived records), .basis drifts silently. Also, the 'has an active criterion' check is written twice (validate() and the hyp check warning in store.rs). And if a link can ever point at a run of another hypothesis's experiment, .runs (filtered by kind from relevant) would include it, while the docs say 'runs of its experiments'.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Snapshot::basis(id) returns the (id, revision) map; fingerprint_of hashes it byte-identically (stored based_on stays valid); show prints it
- [ ] #2 A test asserts that .basis hashes to .state.fingerprint
- [ ] #3 One helper for 'has an active criterion'
<!-- AC:END -->
