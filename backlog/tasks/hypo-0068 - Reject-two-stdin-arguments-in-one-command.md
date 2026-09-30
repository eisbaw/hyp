---
id: HYPO-0068
title: Reject two '-' (stdin) arguments in one command
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 13:31'
updated_date: '2026-09-30 16:36'
labels:
  - cli
  - bug
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while doing HYPO-0055. `hyp add - --body -` (or set --title - --body -) reads stdin twice: the second argument silently gets an empty string. Fail fast with an argument error instead.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Test with two '-' arguments
- [x] #2 A command given '-' for more than one text argument exits 1 (an ordinary error, per the orchestrator's decision) naming the arguments, before reading stdin
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. stdin_arguments(&Command): the text arguments given as "-", by name (TITLE, --title, --body, --reason).
2. At the start of cli::run, two or more is an error naming them (exit 1), before anything is read.
3. Test.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- AC changed from exit 2 to exit 1 per the orchestrator's decision (an ordinary error from cli::run, not a clap usage error).
- Message: "TITLE and --body are each '-', but stdin can be read only once: give all but one as text".
- Test: stdin_can_be_given_to_one_argument_only (add - --body -, set --title - --body -; one - still works and prints nothing to a non-terminal stderr). Fails on HEAD (exit 0, body silently empty).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Committed in human CLI batch 2 (see commit message). Review: QA GO; architect NO-GO (edit reopen loop with a non-interactive editor; abort message lacked the error) -> fixed; confirmation QA GO, architect GO. Gate: 99 Rust tests + DOM, nix flake check.
<!-- SECTION:FINAL_SUMMARY:END -->
