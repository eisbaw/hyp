---
id: HYPO-0048
title: 'WebUI nits: invisible plus glyph, nav highlight, contrast, plurals'
status: To Do
assignee: []
created_date: '2026-09-30 11:27'
labels:
  - webui
  - ux
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Browser test-drive 2026-09-30. The fullwidth plus U+FF0B on New/Add buttons renders nothing in the page's font stack, so on All records the create buttons look like filter chips (/tmp/claude-1000/-home-mpedersen-topics-hyp/b93cfa51-31aa-4941-a678-5890abf7545d/scratchpad/webtd/12-all-1440.jpg). The sidebar highlights 'Hypotheses' on every record page and the breadcrumb always says 'Record'. Much 9-12px grey text is 3.7-4.4:1 contrast (IDs, eyebrows, SUPPORTS/CONTRADICTS, stat labels, the 'weakened' badge). '1 experiments'; tag sort is case-sensitive; 'Export notebook ↗' suggests an external link but downloads.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 ASCII + or an inline SVG icon
- [ ] #2 Sidebar and breadcrumb reflect the record's kind
- [ ] #3 Text meets 4.5:1 contrast
- [ ] #4 Plurals and case-insensitive tag sort
<!-- AC:END -->
