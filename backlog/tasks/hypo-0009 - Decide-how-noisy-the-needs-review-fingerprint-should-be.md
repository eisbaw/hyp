---
id: HYPO-0009
title: Decide how noisy the needs-review fingerprint should be
status: To Do
assignee: []
created_date: '2026-09-29 22:16'
updated_date: '2026-09-29 22:30'
labels:
  - design
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found in the initial review; design question, not a bug.

`Snapshot::fingerprint` includes the revision of every owned record, linked record and run. So cosmetic edits flag an assessment as needs-review: retagging the hypothesis, fixing a typo in a gap, moving an experiment from planned to running, or editing a *competing* hypothesis linked via `competes_with`. `updated_at` also changes on every save, even a no-op `hyp set`.

If most review flags are noise, users learn to ignore them, and the flag's value is lost.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Decide which changes should invalidate an assessment (e.g. claim/scope/assumptions, criteria, predictions, evidence and interpretations, runs) and which should not (tags, gap resolution, experiment status, linked hypotheses' metadata)
- [ ] #2 Fingerprint implemented accordingly with tests for both a triggering and a non-triggering change
- [ ] #3 No-op updates do not change the file (and therefore the revision)
- [ ] #4 Root cause fixed: the fingerprint hashes a canonical projection of the meaningful fields, not file-byte revisions (which include `updated_at`); object revisions stay raw-byte hashes, which is right for detecting external edits
<!-- AC:END -->
