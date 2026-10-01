---
id: HYPO-0095
title: 'WebUI forms: reference data records without the JSON editor'
status: To Do
assignee: []
created_date: '2026-09-30 22:50'
labels:
  - webui
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found in HYPO-0090. The WebUI lists data records and shows the data a record references, but its create and edit forms cannot set a record's data references; only the Advanced JSON editor can. The CLI has --data on every create command and hyp set.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The create and edit forms of every mutable kind offer the data records to reference, and saving sets the record's data list
- [ ] #2 The DOM test covers setting and clearing a data reference from a form
<!-- AC:END -->
