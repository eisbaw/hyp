---
id: HYPO-0058
title: 'Human feedback on writes: stderr summaries, stdin prompt, no-op notice'
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
Interactive terminal test-drive 2026-09-30 (xterm+tmux, first-time user, release build of fda0faa). Plain write commands print only bare IDs: `evidence add` prints two unlabelled lines (E-, then an unexplained L-); delete/archive/restore, a flagless `hyp set H` and an unchanged `hyp edit` all print just the ID, like a real change. `hyp restore` of a non-archived record succeeds silently. `hyp add -` shows no prompt and just waits. Stdout IDs are the machine contract (keep them).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 When stderr is a TTY, write commands print a one-line human summary to stderr (e.g. 'created evidence E-... (+ link L-... supports H-...)'); stdout is unchanged
- [x] #2 No-op writes say 'no changes' (stderr) and exit 0; restore of a non-archived record says so
- [x] #3 Reading a text argument from a TTY stdin prints a one-line hint to stderr
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. store::Written gets `changed` (serde-skipped, so --json is unchanged): false for an update/archive the store skipped as a no-op.
2. cli: Action per change; summary() groups by verb; report() prints IDs to stdout (unchanged) and on stderr the summary when stderr is a terminal, or "no changes" whenever nothing changed (not with --json).
3. input(arg, s): with a TTY stdin and stderr, "reading ARG from stdin; end with Ctrl-D".
4. Tests: pty (libc posix_openpt) for the summary and the hint; no-op set/restore/archive/edit; README.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Summary format: "created evidence E-xxxxxxxx (+ link L-xxxxxxxx: E-xxxxxxxx supports H-xxxxxxxx)"; IDs shortened to 8 hex digits (a prefix hyp accepts; stdout keeps full IDs). Groups by verb joined with "; ", plus "N unchanged" for mixed apply batches.
- "no changes" is printed whenever nothing changed, TTY or not (a notice, useful to agents in plain mode too), but never with --json (stderr there is only for {"error"}; the unchanged revision says it). Restore of a non-archived record: "no changes: H-... is not archived"; archive of an archived one: "... is already archived".
- TTY detection is tested for real: tests/cli.rs opens a pty with libc (already a dependency) and runs hyp with stderr (and stdin) on it; it also passes inside the nix flake check sandbox. Manually checked in tmux as well (summary, no-op notice, stdin hint; no summary with `2>&1 | cat`).
- Tests: writes_summarise_on_a_terminal_and_hint_at_stdin, a_write_that_changes_nothing_says_so. Both fail on HEAD.

- Review fix: "no changes: H-xxxxxxxx is not archived / is already archived" use the short ID like the other summaries. The pty test helper opens /dev/ptmx through std (O_CLOEXEC) instead of posix_openpt.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Committed in human CLI batch 2 (see commit message). Review: QA GO; architect NO-GO (edit reopen loop with a non-interactive editor; abort message lacked the error) -> fixed; confirmation QA GO, architect GO. Gate: 99 Rust tests + DOM, nix flake check.
<!-- SECTION:FINAL_SUMMARY:END -->
