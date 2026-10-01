---
id: HYPO-0105
title: 'Timing-bound tests: a time-budget multiplier for loaded machines and CI'
status: To Do
assignee: []
created_date: '2026-10-01 18:58'
labels:
  - testing
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Some tests bound wall-clock time and flake under heavy load (seen locally with about 20 parallel jobs on 14 cores; CI runners are smaller and shared):
- tests/reads.rs a_write_is_not_starved_by_reads_that_never_stop
- tests/reads.rs a_check_hashing_stored_bytes_does_not_hold_up_reads_behind_a_writer (asserts list < check / 2)
- the recovery waits in scripts/dom-test.cjs

Blanket retries would hide real regressions. A multiplier from one environment variable keeps each assertion but scales its budget on CI and loaded machines (noted in HYPO-0012 and the HYPO-0004 QA).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 One environment variable (default 1) scales every wall-clock budget in the listed tests, read in one place per language; the README development section documents it
- [ ] #2 The ratio test (list < check / 2) is either made load-independent or scaled by the same variable, with the choice stated in its comment
- [ ] #3 No retry loops are added: each listed test still fails against a deliberate mutant (e.g. a reader that starves the writer) with the multiplier set
- [ ] #4 With the multiplier set, just e2e passes repeatedly under heavy parallel load (e.g. 20 concurrent runs)
<!-- AC:END -->
