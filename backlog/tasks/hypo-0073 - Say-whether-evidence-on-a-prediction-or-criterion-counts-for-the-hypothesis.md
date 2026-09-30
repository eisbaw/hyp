---
id: HYPO-0073
title: Say whether evidence on a prediction or criterion counts for the hypothesis
status: To Do
assignee: []
created_date: '2026-09-30 17:08'
labels:
  - agents
  - docs
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Dogfood v2, 2026-09-30 (Claude Code and Codex resuming yesterday's notebook on a timezone bug; both succeeded; friction reported by the agents). Agents could not tell whether `hyp evidence add P-...` counts for H, so they added a second link to H 'just in case'. It does count (linked_evidence includes evidence linked to active criteria/predictions), but neither --help nor the skill says so.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 evidence add --help and the skill state that evidence linked to a criterion or prediction is part of the hypothesis's basis and citable
- [ ] #2 hyp show makes the path visible (see the 'meets criterion' task)
<!-- AC:END -->
