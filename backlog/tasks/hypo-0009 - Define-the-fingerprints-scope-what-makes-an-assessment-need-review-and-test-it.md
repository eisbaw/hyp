---
id: HYPO-0009
title: >-
  Define the fingerprint's scope (what makes an assessment need review) and test
  it
status: Done
assignee:
  - '@claude'
created_date: '2026-09-29 22:16'
updated_date: '2026-09-30 11:59'
labels:
  - design
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found in the initial review; design question, not a bug.

`Snapshot::fingerprint` includes the revision of every owned record, linked record and run. So cosmetic edits flag an assessment as needs-review: retagging the hypothesis, fixing a typo in a gap, moving an experiment from planned to running, or editing a *competing* hypothesis linked via `competes_with`. `updated_at` also changes on every save, even a no-op `hyp set`.

If most review flags are noise, users learn to ignore them, and the flag's value is lost.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Decide which changes should invalidate an assessment (e.g. claim/scope/assumptions, criteria, predictions, evidence and interpretations, runs) and which should not (tags, gap resolution, experiment status, linked hypotheses' metadata)
- [x] #2 Fingerprint implemented accordingly with tests for both a triggering and a non-triggering change
- [x] #3 No-op updates do not change the file (and therefore the revision)
- [x] #4 Root cause fixed: the fingerprint hashes a canonical projection of the meaningful fields, not file-byte revisions (which include `updated_at`); object revisions stay raw-byte hashes, which is right for detecting external edits
- [x] #5 Implements decision-0003 items 1-3: the fingerprint hashes a canonical projection of the listed content fields; lifecycle, tags, experiment status, gap resolution and updated_at are excluded; linked hypotheses count only via the link; assessments may cite only linked evidence; the citation-closure machinery is removed
- [x] #6 Dogfood regression: hyp set H --lifecycle closed (the skill's finishing step), retagging H, and any edit of a linked or competing hypothesis (title, lifecycle) do not set needs_review on H or on hypotheses linked to it; editing H's own title (the claim, per decision-0003) still does
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Implements decision-0003 items 1-3 in one change with HYPO-0031/0026/0040.
1. Projection per kind in Snapshot::basis: hypothesis title/body/scope/assumptions; criteria, predictions (+conditions) and linked evidence title/body/archived; links touching H or its criteria/predictions: from/to/relation/body/archived; runs of H's experiments: title/body/outcome/evidence (and the cited evidence's content). Excluded: lifecycle, tags, untestable_reason, experiment records, gaps, updated_at, other hypotheses' content.
2. Linked-only citations: create-time rule in commit; delete relevant_with, assessment_additions, JS expectedFrom closure, README recipe; expected.revisions no longer needed for assessments.
3. Tests: RED->GREEN for close/tag/experiment status/gap/competing hypothesis; must-flag cases; projection bites (temporarily drop criteria).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Found in HYPO-0002: citing evidence in an assessment pulls links from that evidence and their endpoints (other hypotheses, their predictions/criteria) into the assessment's fingerprint. Consequences: (1) edits to such a linked other hypothesis flag this assessment needs_review; (2) HYPO-0002's preconditions cover the read fingerprint plus the cited evidence revisions, but not those extra records, so a concurrent change to them between read and commit is absorbed. Narrowing the fingerprint here would close (2) too. Also: an assessment's fingerprint is taken at its position in a batch, so records created later in the same batch flag it immediately.

From the HYPO-0002 architect review (L4): because revisions are raw-byte hashes (including updated_at), a no-op touch of H or its experiments still makes an assessment conflict, and the whole-project re-read check in commit turns an external editor's save to an unrelated file during the commit window into a conflict. A semantic fingerprint would reduce both.

Update from HYPO-0002 cycle 2: point (2) above is closed; an assessment create must now state (or it conflicts) every record its cited evidence brings into the fingerprint. Point (1), the scope of the fingerprint itself, stands.

Payoff noted by the HYPO-0002 architect review: the link closure over cited evidence is the root cause of the precondition machinery (relevant_with, assessment_additions, the JS closure walk in web/app.js). If the fingerprint is narrowed so that citing evidence adds only that evidence, most of it goes away. The JS copy of the closure is also a drift risk: any change to relevant() or the link rules must update web/app.js, and only one DOM scenario guards it.

Raised to high after the HYPO-0002 Codex round-3 review. The precondition machinery is sound; what remains is WHICH records count as relevant. relevant() today is one hop along links from a direct set. Concrete cases the design must decide, each with a test:
(a) E is linked to H's criterion C (E enters via the link scan); a later link E->H2 (a second interpretation of E) does not change H's fingerprint.
(b) E is linked to H2; a run of H1's experiment citing E does not enter relevant(H2).
(c) Links on run-cited evidence now count (changed in HYPO-0002 cycle 3) and pull in other hypotheses, which increases noise (the original concern of this task).
Decide: a principled closure (e.g. evidence reachable within the hypothesis's own records, plus every interpretation of that evidence) vs narrowing. Keep web/app.js expectedFrom in sync, or better, have the server expose what to state.

Test-drive 2026-09-30: concrete, high-visibility symptom. Both dogfood agents (Claude Code, Codex) followed the skill, assessed, then ran `hyp set H --lifecycle closed`; that changed H's revision, hence its fingerprint, so needs_review became true for H and for the competing hypothesis linked to it. Every finished investigation therefore looks stale.

COMPASS 2026-09-30: settle the scope as a user decision first (proposed decision-0003 'review basis'), then implement HYPO-0031 -> 0009 -> 0026 as one deep-gated change. Open questions for the user: per-kind which field edits invalidate (title/body yes? lifecycle/tags/status no? archiving?); whether linked/competing hypotheses' content counts or only the link; whether an assessment may cite evidence outside the basis (if not, relevant_with/assessment_additions/expectedFrom mostly disappear); migration: accept a one-time needs-review wave or version the scheme in based_on. Watch: the title IS the claim for hypotheses/criteria/predictions/evidence, so 'title edits do not invalidate' must not apply to those kinds.

DECIDED by the user 2026-09-30: see decision-0003 (content-only fingerprint; linked-only citations; accept one needs-review wave; evidence required for judgments, tracked separately). Implement HYPO-0031 -> 0009 -> 0026 as one deep-gated change and remove the citation-closure machinery (relevant_with, assessment_additions, expectedFrom, README recipe).

Implemented 2026-09-30 (review basis v2, with HYPO-0031/0026/0040; uncommitted, awaiting the deep gate).
- Snapshot::basis(H) projects content only (record ID -> fields): H title/body/scope/assumptions; its criteria (title/body/archived) and predictions (title/body/conditions/archived); links with an end at H or its criteria/predictions (from/to/relation/body/archived); evidence those links start at (title/body/archived); runs of H's experiments (title/body/outcome/evidence) and the evidence they cite. fingerprint = sha256 of its compact JSON (sorted keys).
- Decisions left open by decision-0003: prediction conditions IN (part of what the prediction claims). Experiments OUT (a plan is not an observation; runs freeze the plan they used and count). Gaps OUT entirely, titles included (open questions/work items, not claims or observations; the task named gap typo fixes as noise). untestable_reason OUT (not in the decision's list; about testability, not the claim). Evidence source/locator/observed_at/attachments OUT per the decision's title+body wording (follow-up HYPO-0050).
- Linked-only citations (decision-0003 item 2): checked when an assessment is created, against the state before the batch (what the stated review token vouches for), not the batch so far. Reason: a batch [link existing E to H, assess citing E] would otherwise base the assessment on E content its author never reviewed, the race the old closure machinery guarded. So linking must be an earlier write. Evidence created in the same batch cannot be cited either.
- Removed: Snapshot::relevant, relevant_with, fingerprint_of; store::assessment_additions and the unstated-closure conflict; the closure walk in web/app.js expectedFrom (the function remains, for experiment/run revisions and the assessment's review_token); the README recipe.
- AC #3: commit skips an update/archive that changes nothing but updated_at (file and revision unchanged).
- AC #5 nuance: closing and retagging H, and any edit of a linked/competing hypothesis, no longer flag H. Editing H's own title does flag it: decision-0003 counts the title as the claim, so the AC's 'title edits' applies only to linked hypotheses. Left unchecked for the orchestrator to reword.
- Migration: none. Every existing based_on (a hash of revisions) differs from the new fingerprint, so every assessed hypothesis shows needs_review once (decision-0003 item 3).
- Tests: changes_outside_the_basis_do_not_flag_an_assessment (all 10 edits RED on HEAD b6ca61d, probed one by one: close, retag, untestable reason, experiment status, experiment procedure, gap resolved, gap title, evidence tags, close competing, rename competing); changes_to_the_basis_flag_an_assessment (claim, scope, criterion title, prediction conditions, observation, link reason, contradicting evidence, new run, link archived, criterion added, criterion archived); run_evidence_counts_but_its_interpretations_elsewhere_do_not; cli closing_after_assessing_leaves_nothing_needing_review (dogfood replay, RED on HEAD); an_update_that_changes_nothing_is_not_written (RED on HEAD). Bite check: with criteria dropped from the projection, changes_to_the_basis_flag_an_assessment fails ('editing a criterion's title went unnoticed'); restored.
- Tests removed/rewritten because they asserted the deleted closure: unlinked_assessment_evidence_changes_trigger_review (unlinked citations are now rejected; replaced by assessment_may_cite_only_evidence_already_linked); assessment_is_a_conflict_when_cited_unlinked_evidence_changed_after_it_was_read, assessment_is_a_conflict_when_records_its_evidence_brings_in_changed_after_it_was_read, batch_linking_existing_evidence_must_state_what_it_brings_in (deleted: the closure and its statements no longer exist; the batch case is now an ordinary error inside assessment_may_cite_only_evidence_already_linked); links_on_run_evidence_are_part_of_the_fingerprint (inverted by the decision, now run_evidence_counts_but_its_interpretations_elsewhere_do_not); cli show_reveals_every_change_the_review_token_covers (asserted the competing hypothesis is covered; now show_prints_the_basis_the_fingerprint_hashes); DOM 'Cites shared evidence / unseen link' step (replaced by evidence-required and linked-only steps). stated_records_deleted_after_the_read_are_conflicts lost its assessment part (it cited unlinked evidence); own_earlier_changes_in_the_batch_are_not_conflicts and records_created_earlier_in_the_same_batch... now cite linked evidence.

Round 2 (2026-09-30, final fix round after the deep gate: QA GO, architect NO-GO, Codex NO-GO):
- AC changed because decision-0003 supersedes it. Old text of AC #5: "Dogfood regression: `hyp set H --lifecycle closed` (the skill's finishing step), tag or title edits do not set needs_review on H or on hypotheses linked to it. In the 2026-09-30 test-drive both the Claude Code and the Codex session ended with every hypothesis 'needs review' purely because they closed them". Removed and re-added as AC #6 (no in-place AC edit in the backlog CLI): decision-0003 counts H's own title as the claim. Verified by changes_outside_the_basis_do_not_flag_an_assessment, changes_to_the_basis_flag_an_assessment ('editing the claim') and cli closing_after_assessing_leaves_nothing_needing_review.
- Architect M1 fixed: linked-only is checked at the create's position against the batch state (after), and the basis may grow within a batch only by records the batch created: after.basis(H) keys minus before.basis(H) keys must be a subset of the batch's created IDs (store::basis_growth), else an ordinary error naming the IDs ("link pre-existing evidence in an earlier write, re-read, then assess"). Test a_batch_may_not_bring_unreviewed_records_into_the_basis: [link edited pre-existing O->H, assess citing linked E] rejected (was accepted), [create E, link E->H, assess citing E] accepted (was rejected); both red before the fix. Conservative side effect: a batch that creates a run citing pre-existing unlinked evidence and then assesses is also rejected; split it.
- Codex P1 (HYPO-0050): evidence projects source, locator and attachment hashes too. Codex P2: the hypothesis projects its own archived flag (test archiving_the_assessed_hypothesis_needs_review: needs_review true and an old token conflicts).
- Architect file-later #1/#2: Snapshot::linked_evidence(H) is the single source for citable evidence and for the evidence part of the basis: evidence with an ACTIVE evidence link to H or an ACTIVE criterion/prediction. Archived links and archived criteria/predictions themselves stay in the basis (archiving them flags). Test evidence_reached_only_through_archived_records_is_not_in_the_basis (red before).
- Architect M2: the_fingerprint_canonical_form_is_pinned asserts literal canonical JSON and hash for fixed records; Python's sha256 of the same JSON gives the same hash. Red with --features serde_json/preserve_order and with a projection field renamed.
- Accepted per decision-0003: experiment text (title, procedure, targets) is not in the basis; runs freeze the plan they used and count (reviewer note).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Committed with the review basis v2 batch. Deep review over two rounds plus a confirmation: round 1 NO-GO (architect M1 batch-growth regression, Codex provenance and archive gaps), all fixed in round 2; confirmation QA GO (70 Rust tests + DOM, x2; flake check), architect GO, Codex GO.
<!-- SECTION:FINAL_SUMMARY:END -->
