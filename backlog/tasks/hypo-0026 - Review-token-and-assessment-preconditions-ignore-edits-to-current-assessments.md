---
id: HYPO-0026
title: Review token and assessment preconditions ignore edits to current assessments
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 01:18'
updated_date: '2026-09-30 11:59'
labels:
  - concurrency
  - agents
dependencies:
  - HYPO-0002
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
From the HYPO-0002 Codex round-3 review (finding 3). Assessments are immutable through the tool, but a hand edit, sync or merge can change a current assessment's judgment or rationale under the same ID. review_token and expected.hypotheses[H].assessment_ids compare IDs only, so a reviewer holding an old token can supersede a changed assessment without having seen it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 review_token and the server-side check cover the revisions of the current assessments, not only their IDs
- [x] #2 Test: an on-disk edit of the current assessment makes an old token exit 3
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. review_token hashes the fingerprint plus id:revision of each current assessment.
2. Server-side assessment precondition becomes expected.hypotheses[H] = {review_token}, compared against the current token (covers fingerprint, assessment IDs and their revisions).
3. Test: hand-edit the current assessment body -> old token exits 3.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented 2026-09-30 with HYPO-0009 (uncommitted).
- review_token = sha256(fingerprint + sorted "ID:revision" of the current assessment heads).
- The server-side assessment precondition is now expected.hypotheses[H] = {review_token} (replacing {fingerprint, assessment_ids}), compared case-insensitively with the current token, so it covers the fingerprint, the head IDs and their revisions in one value. Old-shape statements are an ordinary error (unknown field `fingerprint`, expected `review_token`).
- Test: cli a_hand_edited_current_assessment_invalidates_an_old_token rewrites the current assessment's body on disk; the old token exits 3 (RED on HEAD: exit 0).

Round 2 (2026-09-30, Codex P3): a malformed expected.hypotheses[H].review_token (not 64 hex digits) is an ordinary error (exit 1 / 422, "must be the 64-hex-digit .state.review_token"), not a conflict; case in incomplete_statements_are_clear_errors_not_conflicts, red before (exit 3).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Committed with the review basis v2 batch. Deep review over two rounds plus a confirmation: round 1 NO-GO (architect M1 batch-growth regression, Codex provenance and archive gaps), all fixed in round 2; confirmation QA GO (70 Rust tests + DOM, x2; flake check), architect GO, Codex GO.
<!-- SECTION:FINAL_SUMMARY:END -->
