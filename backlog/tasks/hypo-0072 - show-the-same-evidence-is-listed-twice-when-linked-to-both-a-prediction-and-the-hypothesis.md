---
id: HYPO-0072
title: >-
  show: the same evidence is listed twice when linked to both a prediction and
  the hypothesis
status: To Do
assignee: []
created_date: '2026-09-30 17:08'
labels:
  - agents
  - ux
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Dogfood v2, 2026-09-30 (Claude Code and Codex resuming yesterday's notebook on a timezone bug; both succeeded; friction reported by the agents). Plain `hyp show H` listed one observation twice (via its prediction and via a direct link), each with the full body; easy to misread as two observations.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Each evidence record appears once, with all its links/relations listed under it
- [ ] #2 Test
<!-- AC:END -->
