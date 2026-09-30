---
id: HYPO-0092
title: 'WebUI: surface unexplained observations and what an observation bears on'
status: To Do
assignee: []
created_date: '2026-09-30 21:25'
labels:
  - webui
  - agents
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
HYPO-0091 added observation-first work to the CLI: hyp observe, hyp add --explains/--competes-with, unexplained observations in hyp status (Snapshot::unexplained) and a Bears on section in hyp show E-… (Snapshot::bears_on). The WebUI shows none of it: its overview does not list unexplained observations, an evidence page does not say which hypotheses it bears on and how, and there is no way to create a hypothesis that explains an existing observation in one step.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The WebUI overview lists unexplained observations (the same set as hyp status)
- [ ] #2 An evidence page lists the hypotheses it bears on with stance and links, and says when none explains it
- [ ] #3 A hypothesis can be created from an observation page, linked to it (supports) in the same write
<!-- AC:END -->
