---
id: HYPO-0054
title: CLI ergonomics follow-ups
status: To Do
assignee: []
created_date: '2026-09-30 12:18'
labels:
  - cli
  - ux
  - hardening
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Non-blocking findings from the CLI ergonomics review (2026-09-30): Status::parse would silently prefer Judgment if value sets ever overlap (add a disjointness test); Kind is the single source only for kind(), while directory()/prefix() and JSON show still match on strings (kind() -> Kind plus as_str()); a list row shows 'untested' for conflicting assessment heads without saying why; serde_json::to_value(...).unwrap_or_default() in show.rs swallows errors (use expect); hints still say 'hyp --json show' for the review token though plain show prints it (a deliberate message change); hyp list --kind experiment has no status column; list rows use one space before the title after needs-review; 'Referenced by' in plain show uses auto link titles and lists superseded assessments; plain show of an assessment prints the based_on hash; no-op writes still appear in written (consider a changed flag).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Kind value sets tested disjoint; kind() returns Kind
- [ ] #2 Experiment rows show status; list rows explain conflicting heads
<!-- AC:END -->
