---
id: HYPO-0066
title: hyp check repair for dangling references on referenced or historical records
status: To Do
assignee: []
created_date: '2026-09-30 12:55'
updated_date: '2026-09-30 13:13'
labels:
  - validation
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found in HYPO-0003. For a dangling reference, `hyp check` suggests `hyp archive ID && hyp delete ID`. That fails when other records reference ID (e.g. a prediction whose hypothesis was deleted, still targeted by an experiment), and assessments and runs get no suggestion because they cannot be archived. The remaining repair is restoring the missing file, which the output does not say.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 hyp check suggests a repair that works, or says to restore the missing record, for every dangling-reference diagnostic
- [ ] #2 Test: a referenced record with a dangling owner
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
HYPO-0003 round 2: dangling-reference diagnostics now always carry a restore note; only links get commands (archive, delete: links are never referenced, so delete works). Predictions, criteria, experiments, gaps, assessments and runs get the note only. What remains here is the assessments/runs case the orchestrator scoped.

From the HYPO-0003 confirmation (architect): for a prediction or criterion whose owner is gone for good, the note only says 'Restore'; archiving does not clear the error (dangling references are validated on archived records), so there is no tool path. Add guidance or a repair.
<!-- SECTION:NOTES:END -->
