---
id: HYPO-0065
title: 'Diagnostics: report every rule violation per record with a per-rule code'
status: To Do
assignee: []
created_date: '2026-09-30 12:55'
updated_date: '2026-09-30 13:15'
labels:
  - validation
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found in HYPO-0003. `Snapshot::validate` stops at the first violation of a record, and the non-blocking code `inconsistent` groups several cross-record rules (owner kind, link endpoint kinds, experiment target ownership, investigating without criterion). The commit gate identifies errors by (path, code), so a write can swap one `inconsistent` reason for another on an already-broken record, or hide a second violation behind the first, and still be accepted.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A record breaking two rules reports both in hyp check
- [ ] #2 A write that replaces one cross-record violation of a record with a different one is rejected
- [x] #3 --json diagnostic output stays additive
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From the HYPO-0003 architect review: 'stored references must use full IDs' is classed inconsistent, but whether a short ID dangles depends on which records exist, so an unrelated create whose ID starts with that prefix flips the identity from dangling_reference to inconsistent and the write is rejected (hand edits only). find() misses during the supersession walk come out as inconsistent rather than dangling_reference (cosmetic).

Done within HYPO-0003 round 2 (uncommitted):
- validate collects every violation per record (one diagnostic each); commit rejects a write if any record gains a (path, code) it did not have. QA repro (dangling link turned into depends_on from evidence) is now rejected.
- References are typed by ID prefix (enforced on every record), so kind rules and the full-ID rule are record-local `invalid` (blocking). The short-ID prefix flip is gone: a stored short ID is `invalid` whether or not a record matches it. Supersession-walk misses are skipped (reported once as dangling_reference on the holder).
- AC#2 left unchecked: a swap to a violation of another code is rejected (test replacing_a_known_violation_with_another_is_rejected), but a swap within the same code on the same record (e.g. a link dangling to E1 re-pointed at a missing E2) keeps the identity and is accepted. Closing that needs a subject (the missing ID, the rule) in the identity, beyond the (path, code) contract.

From the HYPO-0003 confirmation (architect): identity is only (path, code), so on a record that already has one missing reference an apply update can add a second one unrejected (same for an experiment with a wrong-hypothesis target gaining another). Include the referenced ID in the identity. Also: an uppercase UUID counts as a full ID and then reports as dangling_reference instead of invalid (hand edits only).

From the HYPO-0003 confirmation (QA): confirmed by repro that a second dangling reference with the same code is accepted (the (path, code, referenced id) identity would close it). Also an experiment whose hypothesis is missing reports the same dangling_reference twice (from hypothesis: and targets[].id); deduplicate.
<!-- SECTION:NOTES:END -->
