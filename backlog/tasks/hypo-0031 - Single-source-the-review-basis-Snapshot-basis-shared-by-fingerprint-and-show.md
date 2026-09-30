---
id: HYPO-0031
title: 'Single-source the review basis: Snapshot::basis shared by fingerprint and show'
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 02:08'
updated_date: '2026-09-30 11:59'
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
- [x] #1 A test asserts that .basis hashes to .state.fingerprint
- [x] #2 One helper for 'has an active criterion'
- [x] #3 Snapshot::basis(id) returns the canonical content projection of decision-0003 (record ID -> counted fields); the fingerprint is the SHA-256 of its compact JSON with sorted keys; hyp --json show prints it as .basis
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Part of the "review basis v2" change (with HYPO-0009, 0026, 0040).
1. Snapshot::basis(id): canonical JSON projection (id -> content fields) of the records an assessment of H is based on; fingerprint = sha256 of its compact JSON.
2. hyp --json show prints .basis from it; .runs/.evidence filtered by its keys.
3. One helper for "has an active criterion" (validate and the hyp check warning).
4. Test: sha256(compact JSON of .basis) == .state.fingerprint.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented 2026-09-30 with HYPO-0009 (uncommitted, awaiting the deep gate).
- Snapshot::basis(H) is the one source: fingerprint() hashes it (sha256 of compact JSON, sorted keys) and `hyp --json show` prints it as .basis; .runs and .evidence are the run/evidence entries whose IDs are basis keys, so runs enter only through H's own experiments.
- AC #1 as written (an (id, revision) map hashed byte-identically, stored based_on stays valid) is superseded by decision-0003 items 1 and 3: basis is the content projection, and existing based_on values show needs-review once. Left unchecked for the orchestrator to reword.
- AC #2: cli show_prints_the_basis_the_fingerprint_hashes asserts hash(.basis) == .state.fingerprint (RED on HEAD, where .basis held revisions).
- AC #3: Snapshot::has_active_criterion, used by validate() and the hyp check warning.

Round 2 (2026-09-30): AC changed because decision-0003 (items 1 and 3) supersedes it. Old text of AC #1: "Snapshot::basis(id) returns the (id, revision) map; fingerprint_of hashes it byte-identically (stored based_on stays valid); show prints it". Removed and re-added as AC #3 (the backlog CLI cannot edit an AC in place, so the numbering shifted): basis is the content projection, and existing based_on values show needs-review once. Verified: cli show_prints_the_basis_the_fingerprint_hashes and workflow the_fingerprint_canonical_form_is_pinned (literal JSON and hash; red with serde_json/preserve_order enabled and with a projection field renamed). fingerprint_of(basis) is now the one hashing function; commit hashes the same basis it checks for growth.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Committed with the review basis v2 batch. Deep review over two rounds plus a confirmation: round 1 NO-GO (architect M1 batch-growth regression, Codex provenance and archive gaps), all fixed in round 2; confirmation QA GO (70 Rust tests + DOM, x2; flake check), architect GO, Codex GO.
<!-- SECTION:FINAL_SUMMARY:END -->
