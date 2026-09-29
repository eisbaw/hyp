---
id: HYPO-0006
title: '`hyp search` matches serialized field names'
status: To Do
assignee: []
created_date: '2026-09-29 22:15'
updated_date: '2026-09-29 22:30'
labels:
  - bug
  - cli
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found in the initial review. `Search` lowercases `serde_json::to_string(&record)` and substring-matches it, so field names and enum values also match. `hyp search kind` and `hyp search title` return every record. `hyp search lifecycle` returns every hypothesis, because only hypotheses have that field. Queries containing quotes or newlines never match because of JSON escaping.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Search matches only user-visible values (title, body, tags, and text fields such as scope, source, locator, conditions)
- [ ] #2 Searching for a field name like `kind` returns no records unless a value contains it
- [ ] #3 Test for both behaviours
<!-- AC:END -->
