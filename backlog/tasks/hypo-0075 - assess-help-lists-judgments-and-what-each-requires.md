---
id: HYPO-0075
title: assess --help lists judgments and what each requires
status: To Do
assignee: []
created_date: '2026-09-30 17:08'
labels:
  - agents
  - docs
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Dogfood v2, 2026-09-30 (Claude Code and Codex resuming yesterday's notebook on a timezone bug; both succeeded; friction reported by the agents). An agent only learned from the skill that 'falsified' needs --criterion and that other judgments need linked evidence; hyp assess --help lists the values but not their requirements, and a bad value's error does not explain them.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 assess --help: one line per judgment with its requirements (untested: none; inconclusive/supported/weakened: linked evidence; falsified: linked evidence + --criterion)
- [ ] #2 The error for a missing requirement names the judgment's rule
<!-- AC:END -->
