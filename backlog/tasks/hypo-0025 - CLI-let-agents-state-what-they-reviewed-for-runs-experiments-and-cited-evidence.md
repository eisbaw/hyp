---
id: HYPO-0025
title: >-
  CLI: let agents state what they reviewed for runs, experiments and cited
  evidence
status: In Progress
assignee:
  - '@implementer-B'
created_date: '2026-09-30 01:18'
updated_date: '2026-10-01 08:06'
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
- [ ] #2 hyp assess accepts revisions for cited evidence reviewed outside the fingerprint (or a combined token covering them)
- [x] #3 Decide per flag whether it is required (agent-first) or optional; README and skill updated
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Per decision-0003 (linked-only citations), AC #2 (cited evidence reviewed outside the fingerprint) is obsolete once HYPO-0009 lands: cited evidence is always inside the basis. What remains is --reviewed for hyp run and hyp experiment add.

2026-09-30: confirmed by the review basis v2 change: assessments may cite only evidence already linked when read (checked against the pre-batch state), so AC #2 is obsolete; cited evidence is always inside the review token.

2026-10-01 implementation (implementer B): hyp experiment add --reviewed TOKEN takes the hypothesis review token (content-only: covers the claim, criteria and predictions it freezes; a lifecycle change does not conflict). Chosen over per-target raw revisions, which would conflict on hyp set H --lifecycle investigating, the usual step before planning.

hyp run --reviewed REVISION takes the experiment revision; plain hyp show line 1 now ends with 'revision <12 hex>' for every non-hypothesis record. Cited evidence is named, not frozen, so it is not covered.

AC #3 decision: both optional. The frozen content is stored in the record itself, so nothing is lost (unlike a judgment); requiring it would break every existing caller. Mismatch exits 3 (conflict, ids the record); a malformed value exits 1. README Preconditions and the skill updated.
<!-- SECTION:NOTES:END -->
