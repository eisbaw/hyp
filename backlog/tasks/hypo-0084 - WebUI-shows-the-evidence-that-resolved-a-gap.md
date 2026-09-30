---
id: HYPO-0084
title: WebUI shows the evidence that resolved a gap
status: To Do
assignee: []
created_date: '2026-09-30 17:53'
labels:
  - webui
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while implementing HYPO-0076. Gaps can carry resolved_by (evidence IDs), shown by hyp show and hyp status, but the WebUI lists only open gaps and its gap form neither shows nor sets resolved_by (reopening clears it).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Hypothesis detail lists resolved gaps with links to the evidence that resolved them
- [ ] #2 The gap form can set resolved_by to evidence of the project
<!-- AC:END -->
