---
id: HYPO-0033
title: 'hyp list: validate filters and show status'
status: To Do
assignee: []
created_date: '2026-09-30 02:26'
labels:
  - cli
  - ux
  - agents
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Test-drive 2026-09-30. `hyp list --kind hypotheses` (a plural typo) or an unknown --status silently prints nothing and exits 0, so an agent concludes there are no hypotheses. The default output interleaves every record kind sorted by ID (links, criteria, assessments), and hypothesis rows show neither the judgment, the lifecycle nor needs-review, so the list cannot answer 'what is the state of my investigation'.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 --kind and --status are validated (clap value enums); an unknown value exits 2 and lists valid values
- [ ] #2 Hypothesis rows show judgment, lifecycle and a needs-review marker
- [ ] #3 Decide whether plain `hyp list` defaults to hypotheses only (with --all for everything); document it
<!-- AC:END -->
