---
id: HYPO-0032
title: 'CLI help and small polish: undocumented subcommands, plurals, init next steps'
status: To Do
assignee: []
created_date: '2026-09-30 02:26'
updated_date: '2026-09-30 11:27'
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
- [ ] #1 Every subcommand and global flag has a one-line help text (agents read --help)
- [ ] #2 Singular/plural messages are correct
- [ ] #3 `hyp init` prints one line with the next steps
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Browser test-drive 2026-09-30:  says 'notes' but the flag is --body.

Correction to the previous note (a shell quoting slip dropped the command): the help text of "hyp set" describes the flag as "notes", but the flag is --body.
<!-- SECTION:NOTES:END -->
