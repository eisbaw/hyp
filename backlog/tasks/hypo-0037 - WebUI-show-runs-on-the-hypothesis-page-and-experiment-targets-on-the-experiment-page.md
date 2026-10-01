---
id: HYPO-0037
title: >-
  WebUI: show runs on the hypothesis page and experiment targets on the
  experiment page
status: In Progress
assignee:
  - '@implementer-A'
created_date: '2026-09-30 02:26'
updated_date: '2026-10-01 08:34'
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
- [x] #1 Hypothesis page lists each experiment's runs with outcome and cited evidence
- [x] #2 Experiment page lists all frozen targets and marks targets whose current revision differs from the frozen one
- [x] #3 DOM test covers both
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Hypothesis page: each experiment shows its status and its runs (outcome badge, link, cited evidence) (experimentItem).
- Experiment page: 'Frozen targets' lists every target with its kind, 'as frozen' or 'changed since frozen' (current revision differs) or 'missing', the frozen title when renamed, and the frozen content in a details element when changed (targetsSection).
- DOM test recordPages(): the demo run with outcome and evidence on the hypothesis page; after hyp set of the demo prediction, exactly that target is data-changed=true. Red without each.
<!-- SECTION:NOTES:END -->
