---
id: HYPO-0006
title: '`hyp search` matches serialized field names'
status: To Do
assignee: []
created_date: '2026-09-29 22:15'
updated_date: '2026-09-30 12:37'
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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Interactive terminal test-drive 2026-09-30 (xterm+tmux, first-time user, release build of fda0faa). `hyp search logrotate` returns runs whose titles do not contain the word, because run files embed the frozen experiment/targets snapshot (plan:) and search matches it. No match context; 'no results' prints nothing (exit 0). Exclude frozen snapshots, show the matching field, print 'no matches' to stderr.
<!-- SECTION:NOTES:END -->
