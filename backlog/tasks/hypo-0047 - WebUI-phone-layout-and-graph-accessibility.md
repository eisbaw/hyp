---
id: HYPO-0047
title: 'WebUI: phone layout and graph accessibility'
status: To Do
assignee: []
created_date: '2026-09-30 11:27'
labels:
  - webui
  - ux
  - a11y
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Browser test-drive 2026-09-30. At 390px the hypothesis page puts 'Current assessment' and 'Review hypothesis' at the very bottom (/tmp/claude-1000/-home-mpedersen-topics-hyp/b93cfa51-31aa-4941-a678-5890abf7545d/scratchpad/webtd/14-hyp-390.jpg); the graph is a 750px SVG in a 339px box with no scroll hint and the focus node cut off (/tmp/claude-1000/-home-mpedersen-topics-hyp/b93cfa51-31aa-4941-a678-5890abf7545d/scratchpad/webtd/15-graph-390.jpg); the active tab in the nav strip can be off-screen. The graph SVG is one image to assistive tech (nodes are SVG <a> not reachable by keyboard); the legend colours 'competes with' the same green as 'supports'.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 At 390px the current assessment appears near the top of the hypothesis page and the graph fits or scrolls with a visible affordance
- [ ] #2 Graph nodes are keyboard-reachable with accessible names; relation colours are distinct
<!-- AC:END -->
