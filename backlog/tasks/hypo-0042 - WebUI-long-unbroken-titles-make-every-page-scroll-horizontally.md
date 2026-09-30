---
id: HYPO-0042
title: 'WebUI: long unbroken titles make every page scroll horizontally'
status: To Do
assignee: []
created_date: '2026-09-30 11:26'
labels:
  - webui
  - bug
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Browser test-drive 2026-09-30. A hypothesis title with a ~95-character path (no spaces) makes the document 1741px wide at 1440px and 1237px at 390px; cards reach 798px on a phone and push Edit/Archive off-screen. Agents often put paths and identifiers in titles. Screenshot: /tmp/claude-1000/-home-mpedersen-topics-hyp/b93cfa51-31aa-4941-a678-5890abf7545d/scratchpad/webtd/26-longtitle-390.jpg
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Titles, IDs and sources wrap (e.g. overflow-wrap: anywhere) so no view scrolls horizontally at 390px or 1440px
- [ ] #2 Checked with a long-path title in the DOM or browser test
<!-- AC:END -->
