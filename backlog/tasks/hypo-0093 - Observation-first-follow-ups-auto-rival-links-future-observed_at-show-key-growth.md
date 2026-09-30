---
id: HYPO-0093
title: >-
  Observation-first follow-ups: auto rival links, future observed_at, show key
  growth
status: To Do
assignee: []
created_date: '2026-09-30 21:54'
updated_date: '2026-09-30 22:09'
labels:
  - agents
  - ux
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
From the HYPO-0091 reviews. (1) Explanations of the same observation are only linked competes-with when named (--competes-with); consider offering the existing explanations of that observation as rivals (e.g. --competes-with-all or a hint listing them). (2) observe --observed-at accepts future dates; observed_at defaults to now even when the locator holds a date (coordinate with HYPO-0043). (3) 'Bears on' is sorted by ID, not by when each explanation was added. (4) hyp show --json keeps growing kind-specific top-level keys (bears_on, unexplained, runs, evidence, basis); consider a per-kind object before data records add more. (5) The status header counts falsified hypotheses as open.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Decide and implement rival linking help for --explains
- [ ] #2 observed_at rejects future dates beyond a small tolerance
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From the HYPO-0091 confirmation: (6) evidence recorded against a criterion (it meets it, so it counts against the hypothesis) is listed as an unexplained observation from the moment it is recorded, until a live hypothesis accounts for it. Kept deliberately (no live explanation exists yet), but it can crowd the list during an investigation; revisit if users find it noisy. (7) Evidence that does not meet a criterion counts For and so explains the observation (the README calls it mild corroboration): confirm deliberately. (8) JSON bears_on lacks lifecycle while plain show prints it.
<!-- SECTION:NOTES:END -->
