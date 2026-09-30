---
id: HYPO-0051
title: hyp check reports assessments that predate the create-time assessment rules
status: To Do
assignee: []
created_date: '2026-09-30 11:31'
labels:
  - validation
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Evidence-required and linked-only citations (decision-0003) are checked when an assessment is created, not on stored ones, because assessments are immutable history and a later link archive must not invalidate them. Assessments written by an older hyp (or by hand) may therefore be judgments without evidence or cite unlinked evidence, and nothing tells an agent so.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 hyp check warns (not errors) about stored assessments that break the create-time rules, naming the rule
- [ ] #2 hyp check --strict fails on them
<!-- AC:END -->
