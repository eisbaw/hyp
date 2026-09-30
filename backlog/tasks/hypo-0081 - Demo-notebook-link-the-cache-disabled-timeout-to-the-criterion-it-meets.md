---
id: HYPO-0081
title: 'Demo notebook: link the cache-disabled timeout to the criterion it meets'
status: To Do
assignee: []
created_date: '2026-09-30 17:20'
labels:
  - demo
  - docs
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while implementing HYPO-0071. In hyp init --demo, the observation 'Timeout reproduced at transfer 8,142' (with D-cache disabled) is exactly what criterion 'Timeout reproduces with D-cache disabled' describes, but it is linked 'contradicts' to the hypothesis, and the demo assessment is 'weakened' while naming that criterion (a criterion is meant for falsified). The demo is what people and agents first read, so it should model the intended pattern: evidence that meets a criterion is linked to the criterion, and a judgment names a criterion only when it is falsified (or say why weakened with a met criterion is intended).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The demo's evidence is linked to the criterion it meets, and plain show lists it as 'meets criterion ... (counts against H)'
- [ ] #2 The demo assessment's judgment and criterion are consistent
- [ ] #3 Tests that rely on the demo's shape still pass or are updated
<!-- AC:END -->
