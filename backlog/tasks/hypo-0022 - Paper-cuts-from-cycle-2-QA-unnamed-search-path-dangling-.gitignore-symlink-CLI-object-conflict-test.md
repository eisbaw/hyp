---
id: HYPO-0022
title: >-
  Paper cuts from cycle-2 QA: unnamed search path, dangling .gitignore symlink,
  CLI object-conflict test
status: To Do
assignee: []
created_date: '2026-09-29 23:15'
updated_date: '2026-09-29 23:39'
labels:
  - cli
  - ux
  - hardening
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
From the cycle-2 QA gate (non-blocking). (1) `hyp --project <empty dir> list` says 'no hyp project found; run hyp init' without naming where it searched. (2) A dangling symlink at `.hyp/.gitignore` is silently accepted, while symlinks elsewhere in the layout are rejected (safe, but inconsistent). (3) tests/cli.rs covers exit code 3 only for a stale whole-project revision; a stale per-object revision via `hyp apply` was verified by hand only.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 'no hyp project found' names the starting directory it searched upward from
- [ ] #2 A symlink at .hyp/.gitignore is rejected like the other layout entries
- [ ] #3 CLI test: a stale per-object expected_revision exits 3
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
HYPO-0002 added tests/cli.rs unrelated_write_between_read_and_commit_does_not_fail_a_cli_write, which also checks exit 3 for a stale per-object revision through 'hyp edit' (a concurrent 'hyp set' of the same record). AC #3 asks for it through 'hyp apply'; still open if that path should be covered separately.
<!-- SECTION:NOTES:END -->
