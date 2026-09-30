---
id: HYPO-0070
title: Skill drift guard for the Commands block and JSON paths
status: To Do
assignee: []
created_date: '2026-09-30 16:53'
labels:
  - agents
  - testing
  - hardening
dependencies:
  - HYPO-0036
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
From the HYPO-0036 architect review. Only the skill's Example is executed by the drift test; the Commands block and the JSON paths the prose names are reviewed by hand, and they are the parts most likely to drift. The drift test also inherits the environment (BASH_ENV would be sourced; an exported function named hyp would shadow the binary), has no timeout, and bash_blocks silently skips sh/shell fences inside ## Example.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A test parses each 'hyp <sub> ... --flag' line in SKILL.md and checks the flag appears in 'hyp <sub> --help'
- [ ] #2 A test checks the JSON paths named in SKILL.md (.state.review_token, .basis, .written[].revision, .repair.note/commands, blocks_writes) against real output
- [ ] #3 The Example test runs with a cleaned environment and a timeout; non-bash fences inside ## Example fail the test
<!-- AC:END -->
