---
id: HYPO-0055
title: Multi-line stdin title breaks one-record-per-line output
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 12:36'
updated_date: '2026-09-30 13:56'
labels:
  - bug
  - cli
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Interactive terminal test-drive 2026-09-30 (xterm+tmux, first-time user, release build of fda0faa). `hyp add -` with multi-line stdin stores a title containing a newline; `hyp list` and `list | head/cut` then break across lines and show prints 'title: Cron starts two backups\n  second line body?'. Applies to every command taking a title via '-'.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Titles cannot contain newlines: either an ordinary error, or the first line becomes the title and the rest the body (decide, document)
- [x] #2 validate() rejects stored titles with newlines so hand-edited files are caught by hyp check
- [x] #3 Test with multi-line stdin
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Decided: with '-' the first stdin line is the title, the rest is appended to --body (blank line between if both). A given title with a newline fails validation (Code::Invalid "title must be a single line; put the rest in the body"), so hyp check reports hand edits. Documented in README and each TITLE help.
- Test: a_title_is_one_line_and_stdin_lines_after_it_go_to_the_body (red on old source).

- Review round: the multi-line-title diagnostic carries a repair note (edit the file by hand; hyp check confirms). Still blocks writes.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Committed in human CLI batch 1 (see commit message). Review: QA GO and architect GO; small cleanup round (wrap_help dropped, web relation spelling, --project help, multi-line-title repair note); confirmation QA GO, architect GO. Gate: 90 Rust tests + DOM, nix flake check.
<!-- SECTION:FINAL_SUMMARY:END -->
