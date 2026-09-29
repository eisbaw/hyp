---
id: HYPO-0003
title: >-
  Invariant violations from external edits or merges block the writes that would
  repair them
status: To Do
assignee: []
created_date: '2026-09-29 22:15'
updated_date: '2026-09-29 22:30'
labels:
  - bug
  - validation
  - storage
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found in the initial review. Reproduced.

`commit` calls `assert_healthy()` first and rejects every write while any error diagnostic exists. Files that parse but break cross-record rules can arrive from outside hyp: a hand edit, a file sync from another machine, or a Git merge where two branches each add one half of a `depends_on` cycle. Then even `hyp archive <link>` or `hyp delete`, which would fix the problem, fails with `project has 2 validation errors ... depends_on cycle detected`. The only way out is hand-editing YAML front matter.

Malformed/unparseable files should keep blocking writes. Records that parse but violate cross-record invariants should be repairable through the tool.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A transaction that does not introduce new errors and removes or reduces existing ones is accepted while the project has semantic errors
- [ ] #2 Unparseable/malformed files still block all writes
- [ ] #3 `hyp check` output suggests the repair command for cycle and dangling-reference errors
- [ ] #4 Test: a depends_on cycle introduced by dropping a second link file into `hyp/links/` (as a merge or sync would) is repaired with `hyp archive` on one link
- [ ] #5 Both gates are addressed: `assert_healthy()` and the post-change `validate` loop over all objects
- [ ] #6 Pre-existing errors are identified by a stable diagnostic identity (path + kind), not message text or counts, when deciding whether a change introduces new errors
- [ ] #7 Repair works from the CLI; the WebUI either allows repair writes or clearly says to use the CLI (today `refresh()` treats any error as 'writes blocked')
<!-- AC:END -->
