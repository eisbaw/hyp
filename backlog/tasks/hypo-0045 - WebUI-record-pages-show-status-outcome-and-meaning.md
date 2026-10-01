---
id: HYPO-0045
title: 'WebUI: record pages show status, outcome and meaning'
status: In Progress
assignee:
  - '@implementer-A'
created_date: '2026-09-30 11:27'
updated_date: '2026-10-01 09:04'
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
- [x] #1 Experiment and run pages show status/outcome and label referenced records by role
- [x] #2 Link pages show relation and direction; evidence cards show their interpretations
- [x] #3 Editing an interpretation cannot change its endpoints
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Record pages: status (experiment), outcome (run), judgment (assessment) and relation (link) as badges under the title.
- 'Referenced records' replaced by role sections (roles()): Hypothesis, Hypothesis under test, Plan: the experiment it ran, Cited evidence, Assessed hypothesis, Evidence considered, Falsification criterion, Resolved by. A run page shows its frozen plan (text, not escaped YAML inside JSON) and whether the experiment changed since; the structured JSON leaves out the frozen copies. Runs and assessments say they are history (no Edit/Archive, as the CLI refuses to archive them).
- Link pages: a FROM -> relation -> TO panel. Evidence cards list each hypothesis they bear on with stance and meaning, or 'unexplained'. Experiment cards say which hypothesis they test.
- Editing a link: no From/To fields, the title says 'Edit interpretation' for links from evidence, and an advanced-JSON edit that changes from/to is rejected before sending. This is a WebUI rule only: the server still accepts endpoint changes through hyp apply update/patch; hyp set has no endpoint flags.
- DOM test recordPages(): each part red when reverted.

- After review: assessments list what they supersede; a reference to a missing record shows its ID marked missing instead of being hidden; link pages show the relation once (in the direction panel).
<!-- SECTION:NOTES:END -->
