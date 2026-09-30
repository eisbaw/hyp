---
id: HYPO-0037
title: >-
  WebUI: show runs on the hypothesis page and experiment targets on the
  experiment page
status: To Do
assignee: []
created_date: '2026-09-30 02:26'
labels:
  - webui
  - ux
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Test-drive 2026-09-30 (demo notebook, real browser). The hypothesis page lists its experiment but not the experiment's runs or their outcomes; you must click through. The experiment page's 'Referenced records' shows only the hypothesis: web/app.js builds it from hypothesis/experiment/from/to/evidence and ignores the frozen `targets` (the demo experiment targets H, a prediction and a criterion), and nothing shows the frozen content next to the current record when they differ.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Hypothesis page lists each experiment's runs with outcome and cited evidence
- [ ] #2 Experiment page lists all frozen targets and marks targets whose current revision differs from the frozen one
- [ ] #3 DOM test covers both
<!-- AC:END -->
