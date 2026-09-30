---
id: HYPO-0024
title: >-
  Precondition contract polish: prefix IDs in values, clones, duplicated lists,
  apply output size
status: To Do
assignee: []
created_date: '2026-09-30 00:28'
updated_date: '2026-09-30 11:49'
labels:
  - hardening
  - cli
  - agents
dependencies:
  - HYPO-0002
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Non-blocking findings from the HYPO-0002 round-2 deep gate:
- A prefix ID inside expected.hypotheses[H].assessment_ids values causes a permanent conflict; a prefix target or experiment ID in JSON gives 'does not exist'. Require full IDs everywhere in create inputs, with an ordinary error.
- Snapshot::relevant_with clones the whole snapshot (bodies included) to push one dummy entry, about 3 times per assessment create. Take the extra cited evidence as a parameter instead.
- In commit, 'let relevant = after.relevant_with(&record)' is computed before the match but used only in the assessment arm.
- The 'assessment, experiment or run need expected' list is duplicated (needs_expected in create_seen and in check_create); use one helper.
- hyp run and seed_demo build plan: e.frozen() only for create_seen to discard it; use FrozenRef::default().
- hyp --json apply prints the whole project snapshot on success, which is large for agents; consider printing affected IDs plus the new project revision.
- Omitting a closure record on a first attempt is exit 3 (inherent: the server cannot know what the caller read); the message lists the IDs. Consider an error-kind field in --json errors so agents can tell 'stale' from 'incomplete statement'.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Full IDs required in all create inputs, with ordinary errors
- [ ] #2 relevant_with no longer clones the snapshot; the duplicated needs_expected list is unified
- [ ] #3 hyp --json apply output is compact (decide the shape, document it)
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From the HYPO-0002 round-3 QA: the same assessment race yields two messages depending on where it is caught (CLI pre-check: 'changed since you reviewed it ... review token differs'; commit: 'since you read the project: the assessments of H changed (assessment_ids)'); unify the wording for CLI users. --reviewed accepts upper-case hex; say so or normalise.

2026-09-30, review basis v2 (HYPO-0009/0031/0026/0040, uncommitted): obsolete bullets: relevant_with (deleted, so its clone is gone), 'let relevant = after.relevant_with' in commit (deleted), the prefix ID in expected.hypotheses[H].assessment_ids (the field is gone; it is now {review_token}), and 'omitting a closure record is exit 3' (no closure). AC #2's relevant_with half is obsolete; the needs_expected duplication (create_seen vs check_create) still stands. The two conflict wordings for the same race are now 'changed since you reviewed it ... the review token differs' (CLI pre-check) and 'hypothesis H changed: its basis or its current assessments (review_token)' (commit): closer, still two.

Filed from the review basis v2 deep gate (2026-09-30): the README fingerprint recipe ("SHA-256 of basis as compact JSON with sorted keys") should say the bytes are UTF-8 with serde_json's escaping (non-ASCII kept literal, no spaces), so an agent recomputing it outside Rust (e.g. Python json.dumps needs separators=(",", ":"), ensure_ascii=False, sort_keys=True) gets the same hash.
<!-- SECTION:NOTES:END -->
