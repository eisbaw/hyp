---
id: HYPO-0025
title: >-
  CLI: let agents state what they reviewed for runs, experiments and cited
  evidence
status: To Do
assignee: []
created_date: '2026-09-30 01:18'
updated_date: '2026-09-30 11:32'
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
- [ ] #1 hyp run and hyp experiment add accept the revisions the agent reviewed (e.g. --reviewed <revision> for the experiment; per-target revisions), with a mismatch exiting 3
- [ ] #2 hyp assess accepts revisions for cited evidence reviewed outside the fingerprint (or a combined token covering them)
- [ ] #3 Decide per flag whether it is required (agent-first) or optional; README and skill updated
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Per decision-0003 (linked-only citations), AC #2 (cited evidence reviewed outside the fingerprint) is obsolete once HYPO-0009 lands: cited evidence is always inside the basis. What remains is --reviewed for hyp run and hyp experiment add.

2026-09-30: confirmed by the review basis v2 change: assessments may cite only evidence already linked when read (checked against the pre-batch state), so AC #2 is obsolete; cited evidence is always inside the review token.
<!-- SECTION:NOTES:END -->
