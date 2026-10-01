---
id: HYPO-0111
title: Test a repair write that also triggers the legacy-attachment migration
status: To Do
assignee: []
created_date: '2026-10-01 18:59'
labels:
  - testing
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Since HYPO-0067 a write that only repairs invalid records is accepted while they block (Store::repair_scope), and records converted by the migration are not limited by the repair scope. A write to a notebook with legacy attachments also migrates them to data records (schema 3, HYPO-0090). No test covers a repair and the migration in one write, so their interaction (repair scope, diagnostics, atomicity) is unverified.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Test: on a notebook with legacy attachments, a write that repairs an invalid record is accepted, migrates the attachments, and hyp check then reports no errors
- [ ] #2 Test: on the same notebook, a repair that leaves the record invalid is blocked and writes nothing, the migration included (hyp/ byte-identical)
<!-- AC:END -->
