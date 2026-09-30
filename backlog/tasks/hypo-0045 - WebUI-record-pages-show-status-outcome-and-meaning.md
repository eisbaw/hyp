---
id: HYPO-0045
title: 'WebUI: record pages show status, outcome and meaning'
status: To Do
assignee: []
created_date: '2026-09-30 11:27'
labels:
  - webui
  - ux
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Browser test-drive 2026-09-30. Experiment detail hides its status; run detail hides its outcome and has no Edit/Archive; referenced records are not labelled by role (plan vs cited evidence); 'Structured record' shows the frozen plan as an escaped YAML string inside JSON; experiment list cards do not show the hypothesis under test. Link detail omits relation and direction. Evidence list cards show no interpretations despite the subtitle; 'Edit interpretation' opens a generic link form where From/To can be changed. Related: HYPO-0037.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Experiment and run pages show status/outcome and label referenced records by role
- [ ] #2 Link pages show relation and direction; evidence cards show their interpretations
- [ ] #3 Editing an interpretation cannot change its endpoints
<!-- AC:END -->
