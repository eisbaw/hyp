---
id: HYPO-0027
title: Make hyp apply preconditions discoverable without the README
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 01:34'
updated_date: '2026-09-30 13:57'
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
- [x] #1 An agent with only the installed skill and hyp --help output can write a valid hyp apply assessment batch
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- hyp apply --help (after_long_help) shows all four change shapes, a create+archive example, an assessment with expected.hypotheses review_token, and experiment/run expected.revisions. Every shape was applied successfully against a demo project with a throwaway script. SKILL.md now points to apply --help instead of the README.
- Test: apply_help_shows_every_change_and_an_assessment_statement.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Committed in human CLI batch 1 (see commit message). Review: QA GO and architect GO; small cleanup round (wrap_help dropped, web relation spelling, --project help, multi-line-title repair note); confirmation QA GO, architect GO. Gate: 90 Rust tests + DOM, nix flake check.
<!-- SECTION:FINAL_SUMMARY:END -->
