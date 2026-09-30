---
id: HYPO-0080
title: 'WebUI graph and hypothesis cards: show evidence by stance, not link relation'
status: To Do
assignee: []
created_date: '2026-09-30 17:20'
labels:
  - webui
  - ux
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while implementing HYPO-0071. The relationship graph colours an evidence edge by its link relation, so evidence that supports (meets) a falsification criterion is drawn green like support for the hypothesis. The overview cards count only links whose from/to is the hypothesis itself, missing evidence linked via its criteria or predictions. Both should use the server-derived bearings (snapshot.bearings, Snapshot::evidence_bearings) that the detail view and matrix now use.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Graph edges from evidence to a criterion are coloured by stance (a met criterion counts against the hypothesis)
- [ ] #2 Hypothesis cards count linked evidence as Snapshot::linked_evidence does, including evidence via criteria and predictions
- [ ] #3 DOM test covers both
<!-- AC:END -->
