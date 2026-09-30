---
id: HYPO-0074
title: Review tokens that are easy to take from the reviewed output
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
Dogfood v2, 2026-09-30 (Claude Code and Codex resuming yesterday's notebook on a timezone bug; both succeeded; friction reported by the agents). Both agents found copying 64-hex review tokens the most tedious step; with two hypotheses in one output they had to pick the right token out of a long listing. Codex asked for 'a command that safely uses the reviewed state directly'. Constraint: the token must come from the output the agent reviewed (do not add a token-only command that tempts assessing without reading).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Plain show prints the 12-hex short token prominently in its header line (e.g. 'H-02c7fa80  review 46d8d8f5c79b'); --json keeps the full token
- [ ] #2 The skill's example uses the short token
- [ ] #3 Test that the short token printed by show is accepted by assess
<!-- AC:END -->
