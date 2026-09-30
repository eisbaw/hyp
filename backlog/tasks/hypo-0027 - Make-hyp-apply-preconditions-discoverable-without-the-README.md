---
id: HYPO-0027
title: Make hyp apply preconditions discoverable without the README
status: To Do
assignee: []
created_date: '2026-09-30 01:34'
labels:
  - agents
  - docs
  - cli
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The agent skill (agents/hyp/SKILL.md) teaches the single-command workflow and points to `hyp apply --help` and the README for batches. The README is not installed with the skill, and `hyp apply --help` only says 'see the README'. An agent that needs `hyp apply` (batches, longer precondition windows) cannot learn the `expected` object from what it has. Options: a fuller `hyp apply --help` (long_about), or a second skill reference file (e.g. agents/hyp/apply.md installed next to SKILL.md, which both Claude Code and Codex load on demand). Found while writing the skill for HYPO-0016.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 An agent with only the installed skill and hyp --help output can write a valid hyp apply assessment batch
<!-- AC:END -->
