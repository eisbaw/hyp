---
id: HYPO-0035
title: Meaningful auto-generated titles for links and assessments
status: To Do
assignee: []
created_date: '2026-09-30 02:26'
updated_date: '2026-09-30 12:37'
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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Interactive terminal test-drive 2026-09-30 (xterm+tmux, first-time user, release build of fda0faa). evidence add titles links 'Evidence for H-...' with no relation even for --against, while hyp link titles 'E-2675 supports H-87d6'; assessment titles are 'Assessment: weakened' without the hypothesis. --reason is required on link but optional (default empty) on evidence add.
<!-- SECTION:NOTES:END -->
