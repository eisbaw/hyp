---
id: HYPO-0050
title: >-
  Decide whether evidence source, locator and attachments belong in the review
  basis
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 11:31'
updated_date: '2026-09-30 11:59'
labels:
  - design
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
decision-0003 lists title and body for evidence, so Snapshot::basis projects evidence as title, body, archived. Correcting an evidence source or locator, or attaching the raw capture (hyp evidence attach), does not flag assessments that cite it. Attachments are content-addressed and arguably part of the observation.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Decision recorded (amend decision-0003 or a new one)
- [x] #2 If included: basis projection updated, with a test that a new attachment flags the assessment
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Resolved 2026-09-30 in the review basis v2 change (with HYPO-0009), per the coordinator after the Codex review (P1): evidence provenance is observation content, which decision-0003 counts ("content only"). The evidence projection in Snapshot::basis is now title, body, archived, source, locator and attachment hashes (sha256 list, in stored order). observed_at stays out, not listed in the ruling (HYPO-0043 is about validating it). Tests: changes_to_the_basis_flag_an_assessment ("correcting the evidence source", "attaching the raw capture" via hyp evidence attach) and the pinned canonical form in the_fingerprint_canonical_form_is_pinned. AC #1 (decision recorded by amending decision-0003 or a new decision) is not done: decisions are the user's, so this ruling is recorded here and in HYPO-0009's notes only; decision-0003 still says "title and body" for evidence. Left unchecked for the orchestrator.

Orchestrator: decision-0003 was amended (marked as proposed, pending user confirmation) to include evidence source, locator and attachment hashes, and the hypothesis's own archived state, in the basis. The earlier note that decision-0003 'still says title and body' is superseded.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Evidence provenance (source, locator, attachment hashes) is part of the review basis; implemented in the review basis v2 batch; decision-0003 amended (pending user confirmation).
<!-- SECTION:FINAL_SUMMARY:END -->
