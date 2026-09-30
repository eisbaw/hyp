---
id: HYPO-0022
title: >-
  Paper cuts from cycle-2 QA: unnamed search path, dangling .gitignore symlink,
  CLI object-conflict test
status: In Progress
assignee:
  - '@claude'
created_date: '2026-09-29 23:15'
updated_date: '2026-09-30 12:37'
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

From the HYPO-0016 QA: `hyp evidence attach E FILE` prints 'Attached ./FILE' instead of an ID, unlike the other write commands (the skill says write commands print IDs). Print the evidence ID (and asset hash).

Filed from the review basis v2 deep gate (2026-09-30): vague errors for agents. "falsified requires a criterion and evidence" does not say which is missing; an unknown ID gives "ID \"X\" matches 0 objects; use a longer prefix", which suggests the ID is too short rather than unknown. Say "no object with ID X" for zero matches.

2026-09-30: taking only the 'evidence attach prints the evidence ID' note as part of the HYPO-0028 batch.

2026-09-30: the `evidence attach` note is done in the HYPO-0028 batch: it prints the evidence full ID (plain) or the lean write JSON. The asset hash is not printed (it is in `hyp --json show E`, attachments[].sha256); the ACs above are untouched.

Interactive terminal test-drive 2026-09-30 (xterm+tmux, first-time user, release build of fda0faa). hyp check warns 'no active falsification criterion' for an archived hypothesis.
<!-- SECTION:NOTES:END -->
