---
id: HYPO-0076
title: Resolve an open question (gap) with evidence
status: To Do
assignee: []
created_date: '2026-09-30 17:08'
labels:
  - agents
  - ux
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Dogfood v2, 2026-09-30 (Claude Code and Codex resuming yesterday's notebook on a timezone bug; both succeeded; friction reported by the agents). To close yesterday's gap an agent wanted to cite the evidence that answered it; `hyp link E G` fails ('expected a hypothesis, prediction or criterion, got gap'), so the answer lived only in free text.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A gap can be resolved with evidence (e.g. hyp set G --resolved true --by E-..., or links from evidence to gaps), shown in hyp show
- [ ] #2 Decide whether gap answers enter the basis (decision-0003 says gaps do not; keep it that way unless decided otherwise)
<!-- AC:END -->
