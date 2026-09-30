---
id: HYPO-0032
title: 'CLI help and small polish: undocumented subcommands, plurals, init next steps'
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 02:26'
updated_date: '2026-09-30 13:56'
labels:
  - cli
  - ux
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Test-drive 2026-09-30. `hyp --help` lists predict, falsify-if, gap, evidence, experiment, run, link, show, list, search, edit, archive, restore, check, web, graph and export with no description, and `--project` / `--json` have no help. `hyp check` prints 'Checked 1 objects'. `hyp init` prints only 'Initialized <path>' with no next step (e.g. `hyp add`, `hyp agents install`).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Every subcommand and global flag has a one-line help text (agents read --help)
- [x] #2 Singular/plural messages are correct
- [x] #3 `hyp init` prints one line with the next steps
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Browser test-drive 2026-09-30:  says 'notes' but the flag is --body.

Correction to the previous note (a shell quoting slip dropped the command): the help text of "hyp set" describes the flag as "notes", but the flag is --body.

Interactive terminal test-drive 2026-09-30 (xterm+tmux, first-time user, release build of fda0faa). Help findings: 15 of 23 subcommands have no description while agents/apply have long wrapping ones; flags almost everywhere lack help (evidence add --source/--locator/--against/--qualifies/--reason, experiment add --targets needs 'comma-separated F-/P- IDs', list --archived, check --strict, graph --focus: say it prints Mermaid, run --evidence); `hyp help add` shows <TITLE> but says '-' reads stdin, and --body - also works; set --help uses clap's long layout unlike other commands; help does not wrap at narrow widths (enable clap wrap_help); add --tags vs list --tag.

- One-line about for every subcommand, help for every argument (test iterates Cli::command(): every_subcommand_and_argument_has_help). --tags/--tag aliases both ways; set --help says --body; clap wrap_help enabled (adds terminal_size 0.4.4 to Cargo.lock; nix flake check passes). Empty "" defaults hidden.
- "Checked 1 object"; init prints one Next: line. Test: check_counts_in_the_singular_and_init_says_what_to_do_next.

- Review round: clap wrap_help dropped again (decision-0002: fixed-width, deterministic help); Cargo.toml/Cargo.lock equal HEAD. --project help is a neutral one-liner.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Committed in human CLI batch 1 (see commit message). Review: QA GO and architect GO; small cleanup round (wrap_help dropped, web relation spelling, --project help, multi-line-title repair note); confirmation QA GO, architect GO. Gate: 90 Rust tests + DOM, nix flake check.
<!-- SECTION:FINAL_SUMMARY:END -->
