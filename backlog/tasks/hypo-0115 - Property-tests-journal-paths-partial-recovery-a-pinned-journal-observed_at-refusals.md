---
id: HYPO-0115
title: >-
  Property tests: journal paths, partial recovery, a pinned journal, observed_at
  refusals
status: To Do
assignee: []
created_date: '2026-10-01 19:00'
labels:
  - testing
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Gaps left by the HYPO-0013 property tests (tests/properties):
- The journal property only generates valid journals; invalid and symlinked journal entry paths are not generated.
- Recovery that meets an invalid entry stops partway, after applying the entries before it. Nobody has decided whether that is correct or whether recovery should validate the whole journal first.
- No journal saved from a real crashed commit is checked in, so a change to the on-disk journal format would go unnoticed.
- The commit property never generates the observed_at values writers refuse (8bd8ba2: unreadable or more than a day ahead).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The journal property generates invalid and symlinked entry paths; recovery refuses them and leaves hyp/ unchanged
- [ ] #2 The behaviour of recovery that meets an invalid entry after valid ones is decided, documented in the code, and pinned by a test
- [ ] #3 A journal from a real crashed commit is checked in and a test recovers it to the expected state
- [ ] #4 The commit property generates refused observed_at values and checks the refusal: invalid_input with a bad_observed_at diagnostic, nothing written
<!-- AC:END -->
