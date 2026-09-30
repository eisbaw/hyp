---
id: HYPO-0035
title: Meaningful auto-generated titles for links and assessments
status: To Do
assignee: []
created_date: '2026-09-30 02:26'
labels:
  - cli
  - ux
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Test-drive 2026-09-30. `hyp evidence add --against` creates a link titled 'Evidence for H-xxxx' even though it contradicts; two links from one session had identical titles. Assessments get 'Assessment: falsified'. `hyp link` titles use the prefixes the user typed. These titles show up in `hyp list`, the graph and the WebUI.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Auto link titles include the relation and both ends' titles (e.g. 'contradicts: <hypothesis title>')
- [ ] #2 Assessment titles include the hypothesis title
- [ ] #3 Existing files are not rewritten
<!-- AC:END -->
