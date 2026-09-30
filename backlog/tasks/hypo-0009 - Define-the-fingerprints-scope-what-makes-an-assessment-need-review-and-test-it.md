---
id: HYPO-0009
title: >-
  Define the fingerprint's scope (what makes an assessment need review) and test
  it
status: To Do
assignee: []
created_date: '2026-09-29 22:16'
updated_date: '2026-09-30 02:26'
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
- [ ] #1 Decide which changes should invalidate an assessment (e.g. claim/scope/assumptions, criteria, predictions, evidence and interpretations, runs) and which should not (tags, gap resolution, experiment status, linked hypotheses' metadata)
- [ ] #2 Fingerprint implemented accordingly with tests for both a triggering and a non-triggering change
- [ ] #3 No-op updates do not change the file (and therefore the revision)
- [ ] #4 Root cause fixed: the fingerprint hashes a canonical projection of the meaningful fields, not file-byte revisions (which include `updated_at`); object revisions stay raw-byte hashes, which is right for detecting external edits
- [ ] #5 Dogfood regression: `hyp set H --lifecycle closed` (the skill's finishing step), tag or title edits do not set needs_review on H or on hypotheses linked to it. In the 2026-09-30 test-drive both the Claude Code and the Codex session ended with every hypothesis 'needs review' purely because they closed them
<!-- AC:END -->

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
<!-- SECTION:NOTES:END -->
