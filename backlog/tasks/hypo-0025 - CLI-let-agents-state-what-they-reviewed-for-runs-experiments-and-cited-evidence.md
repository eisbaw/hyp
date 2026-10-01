---
id: HYPO-0025
title: >-
  CLI: let agents state what they reviewed for runs, experiments and cited
  evidence
status: Done
assignee:
  - '@implementer-B'
created_date: '2026-09-30 01:18'
updated_date: '2026-10-01 19:01'
labels:
  - cli
  - agents
  - concurrency
dependencies:
  - HYPO-0002
  - HYPO-0009
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
From the HYPO-0002 Codex round-3 review (finding 1, documented as a known limit). hyp run, hyp experiment add and the cited evidence of hyp assess take their preconditions from the command's own read. An agent that reviewed experiment X (or evidence E) earlier and then runs the command after X/E changed gets the new content frozen or cited without a conflict. hyp assess --reviewed covers only the hypothesis fingerprint and current assessments. Per decision-0002, agents should be able to state what they reviewed.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 hyp run and hyp experiment add accept the revisions the agent reviewed (e.g. --reviewed <revision> for the experiment; per-target revisions), with a mismatch exiting 3
- [x] #2 hyp assess accepts revisions for cited evidence reviewed outside the fingerprint (or a combined token covering them)
- [x] #3 Decide per flag whether it is required (agent-first) or optional; README and skill updated
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Per decision-0003 (linked-only citations), AC #2 (cited evidence reviewed outside the fingerprint) is obsolete once HYPO-0009 lands: cited evidence is always inside the basis. What remains is --reviewed for hyp run and hyp experiment add.

2026-09-30: confirmed by the review basis v2 change: assessments may cite only evidence already linked when read (checked against the pre-batch state), so AC #2 is obsolete; cited evidence is always inside the review token.

2026-10-01 implementation (implementer B): hyp experiment add --reviewed TOKEN takes the hypothesis review token (content-only: covers the claim, criteria and predictions it freezes; a lifecycle change does not conflict). Chosen over per-target raw revisions, which would conflict on hyp set H --lifecycle investigating, the usual step before planning.

hyp run --reviewed REVISION takes the experiment revision; plain hyp show line 1 now ends with 'revision <12 hex>' for every non-hypothesis record. Cited evidence is named, not frozen, so it is not covered.

AC #3 decision: both optional. The frozen content is stored in the record itself, so nothing is lost (unlike a judgment); requiring it would break every existing caller. Mismatch exits 3 (conflict, ids the record); a malformed value exits 1. README Preconditions and the skill updated.

Review round 2 (P2.3): kept the hypothesis review token for experiment add --reviewed and documented precisely (README Preconditions, --help) that any basis or assessment change since the review conflicts, also one the experiment would not freeze; per-target precision stays available via expected.revisions in hyp apply.

Closing review 2026-10-01: AC #2 checked as met through its "combined token" alternative, not by a new flag. Since decision-0003 an assessment may cite only evidence already linked to the hypothesis or its criteria or predictions (enforced in model.rs: "<id> is not linked to <hypothesis>…"), so every citable evidence record is inside the review basis and hyp assess --reviewed covers it.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Agents can state what they reviewed before freezing content: hyp experiment add --reviewed and hyp run --reviewed, both optional, a mismatch exiting 3.

Changes (335ae01, 8bd8ba2):
- hyp experiment add --reviewed TOKEN takes the hypothesis review token (content-only, so a lifecycle change does not conflict); any change to the basis or the current assessments since the review conflicts, including ones the experiment does not freeze (README and --help say so). Per-target precision stays available through expected.revisions in hyp apply.
- hyp run --reviewed REVISION takes the experiment revision; plain hyp show prints "revision <12 hex>" on line 1 for every non-hypothesis record.
- Cited evidence in hyp assess needs no new flag: under decision-0003 only linked evidence can be cited, so it is always inside the review token (AC #2).
- Both flags are optional: the frozen content is stored in the record, and requiring them would break every caller. Malformed values exit 1. README Preconditions and the skill updated.

Tests: tests/cli.rs covers match and mismatch (exit 3) for both flags. just e2e and nix flake check green at a38c06c.
<!-- SECTION:FINAL_SUMMARY:END -->
