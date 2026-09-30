---
id: HYPO-0040
title: Require evidence for every judgment except untested
status: To Do
assignee: []
created_date: '2026-09-30 05:54'
labels:
  - agents
  - validation
  - mvp2
dependencies:
  - HYPO-0009
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Per decision-0003 (item 4): every assessment whose judgment is not 'untested' must cite at least one evidence ID, for all writers (CLI, apply, WebUI). falsified additionally needs a criterion (already enforced). Replaces the skill's prose rule ('for other judgments citing evidence is your rule'). With linked-only citations (decision-0003 item 2), the cited evidence must also be linked to the hypothesis.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 validate() rejects an assessment with judgment other than untested and no evidence (ordinary error, exit 1 / 422), with a message naming the rule
- [ ] #2 The WebUI assessment form requires evidence and says why
- [ ] #3 Skill and README state the rule as enforced; the skill's 'your rule' wording is removed
- [ ] #4 Tests: CLI assess --status supported without --evidence exits 1; API 422; DOM form
<!-- AC:END -->
