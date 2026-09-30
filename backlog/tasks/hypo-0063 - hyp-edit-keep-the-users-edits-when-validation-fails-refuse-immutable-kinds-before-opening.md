---
id: HYPO-0063
title: >-
  hyp edit: keep the user's edits when validation fails; refuse immutable kinds
  before opening
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 12:36'
updated_date: '2026-09-30 16:36'
labels:
  - cli
  - ux
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Interactive terminal test-drive 2026-09-30 (xterm+tmux, first-time user, release build of fda0faa). When `hyp edit` fails validation the temp file is deleted and all edits are lost; YAML errors report line numbers relative to the front matter, not the file. `hyp edit R-...`/`A-...` opens the editor and only afterwards says assessments and runs are immutable. Changing `kind:` gives a misleading 'unknown field scope' error.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 On a validation error the editor reopens with the error as a comment at the top (like git commit), or the temp file is kept and its path printed
- [x] #2 Line numbers refer to the file the user edited
- [x] #3 Immutable kinds are refused before launching the editor
- [x] #4 A changed kind gives 'cannot change the kind of a record'
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. New src/edit.rs: refuse assessments and runs before launching the editor.
2. Loop: write the text, run the editor, strip "# hyp:" notes, check kind (from the YAML before full decode), decode, commit. On an ordinary error reopen with the error as "# hyp:" comment lines after the opening ---; on a conflict keep the text and fail with exit 3.
3. Abort when the reopened file is saved unchanged, emptied, or the editor fails: keep a copy (tempfile keep) and name it; exit 1.
4. store::decode reports YAML line numbers counted in the file (FrontMatter error, shift_lines); edit shifts them again by the note lines so they count lines of the reopened file.
5. Tests with EDITOR set to a scripted editor in the test temp dir.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Implemented as planned. An unchanged first open is a no-op update ("no changes", exit 0, HYPO-0058); only an unchanged *reopened* file aborts.
- Kept copy: the edited temp file in $TMPDIR (hyp-edit-<short ID>-XXXX.md), kept only when it holds edits (not empty, not the record as it was); an abort with nothing to keep says "nothing was written" without a path. A conflict (exit 3) also keeps the edits: "conflict: ...; your text is kept in PATH" (message still starts with conflict:).
- decode() now reports front-matter YAML errors with file line numbers (opening --- is line 1), which also fixes `hyp check` diagnostics for malformed files (previously off by one).
- Changing kind: "cannot change the kind of a record (hypothesis to prediction); create a new record instead". Changing id: "cannot change the ID of a record".
- Tests: edit_reopens_with_the_error_and_keeps_the_users_text (asserts the reported line number points at the broken line of the reopened file), edit_aborts_keeping_the_text_when_the_reopened_file_is_saved_unchanged (also kind change, and emptying), edit_refuses_assessments_and_runs_before_opening_the_editor (the editor logs each open); unrelated_write_between_read_and_commit_does_not_fail_a_cli_write now also asserts the kept copy on conflict. All new ones fail on HEAD.

Review fix round (architect NO-GO):
- Infinite reopen loop fixed: a non-interactive editor that rewrote the file each round (cp of an invalid file) dropped the # hyp: notes, so text never equalled the presented text. Now the editor reopens only when stdin and stderr are both terminals; otherwise the first error fails (exit 1): "<error>; nothing was written; your text is kept in PATH". On a terminal, "unchanged" compares the note-stripped text with the previous round's note-stripped text.
- Abort messages carry the last error: "edit aborted (the file was saved unchanged); nothing was written. Last error: <error>; your text is kept in PATH" (same text in --json {"error"}).
- Tests: edit_with_a_copying_editor_stops_instead_of_reopening_forever (non-TTY: 1 open; pty: 2 opens; --json error; runs under a 20 s kill timeout; on the previous code it was killed at 20 s). The reopen/abort tests now run hyp on a pty (tests/cli.rs on_terminal), since reopening needs a terminal.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Committed in human CLI batch 2 (see commit message). Review: QA GO; architect NO-GO (edit reopen loop with a non-interactive editor; abort message lacked the error) -> fixed; confirmation QA GO, architect GO. Gate: 99 Rust tests + DOM, nix flake check.
<!-- SECTION:FINAL_SUMMARY:END -->
