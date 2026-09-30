---
id: HYPO-0003
title: >-
  Invariant violations from external edits or merges block the writes that would
  repair them
status: Done
assignee:
  - '@claude'
created_date: '2026-09-29 22:15'
updated_date: '2026-09-30 13:15'
labels:
  - bug
  - validation
  - storage
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found in the initial review. Reproduced.

`commit` calls `assert_healthy()` first and rejects every write while any error diagnostic exists. Files that parse but break cross-record rules can arrive from outside hyp: a hand edit, a file sync from another machine, or a Git merge where two branches each add one half of a `depends_on` cycle. Then even `hyp archive <link>` or `hyp delete`, which would fix the problem, fails with `project has 2 validation errors ... depends_on cycle detected`. The only way out is hand-editing YAML front matter.

Malformed/unparseable files should keep blocking writes. Records that parse but violate cross-record invariants should be repairable through the tool.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A transaction that does not introduce new errors and removes or reduces existing ones is accepted while the project has semantic errors
- [x] #2 Unparseable/malformed files still block all writes
- [x] #3 `hyp check` output suggests the repair command for cycle and dangling-reference errors
- [x] #4 Test: a depends_on cycle introduced by dropping a second link file into `hyp/links/` (as a merge or sync would) is repaired with `hyp archive` on one link
- [x] #5 Both gates are addressed: `assert_healthy()` and the post-change `validate` loop over all objects
- [x] #6 Pre-existing errors are identified by a stable diagnostic identity (path + kind), not message text or counts, when deciding whether a change introduces new errors
- [x] #7 Repair works from the CLI; the WebUI either allows repair writes or clearly says to use the CLI (today `refresh()` treats any error as 'writes blocked')
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add a stable DiagnosticCode (kind) to model Diagnostic; split structural (decode/filename/kind-dir/duplicate/attachment) from semantic codes.
2. Store::commit: block only on structural errors up front; after applying changes, compute error identities (path+code) and reject if any identity is new vs before.
3. hyp check: suggest repair command for cycle and dangling-reference errors.
4. WebUI refresh(): only structural errors mark writes blocked; semantic errors show a banner.
5. Tests in tests/cli.rs and tests/workflow.rs (cycle via dropped link file, dangling reference, new error rejected, unparseable blocks, check suggestion).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
COMPASS 2026-09-30: cornerstone of the Git-friendly promise (decision-0001): a merge or sync that introduces e.g. a dangling reference blocks all writes with no tool-level recovery. Independent of HYPO-0009; can run in parallel with the ergonomics tasks.

Implemented (uncommitted, awaiting review):
- Rule: diagnostics get a stable `code`; identity = (path, code). Codes malformed/attachment/invalid block every write (assert_healthy renamed assert_writable, counts only those). dangling_reference/cycle/inconsistent do not block; commit rejects a write only if post-change validate yields an error identity absent before. Writes that leave errors unchanged are allowed (unblocks unrelated work; archive-then-delete repair of a dangling link has an intermediate step that does not reduce errors).
- validate split into validate_fields (record-local -> invalid, blocks) and validate_relations (cross-record); returns typed model::Violation.
- Archived depends_on/supersedes links no longer report a cycle (they are already excluded from the walk); without this, `hyp archive` on one cycle link left that link in error.
- Diagnostic JSON additive: code, blocks_writes, repair. `hyp check` prints `(repair: ...)`.
- WebUI: only blocks_writes diagnostics mark the project stale/blocked; others render with a notice and Repair lines in the checks panel; saving works.
Gotchas:
- Dangling-reference message changed from 'ID "E-..." matches 0 objects; use a longer prefix' to 'references E-..., which does not exist'.
- cargo test with several --test flags stops at the first failing binary; use --no-fail-fast when comparing.
- app.js is include_str!-embedded: rebuild before running dom-test.cjs directly.

Follow-ups filed: HYPO-0065 (first-violation-only and coarse inconsistent code let a write swap one cross-record violation for another on a broken record), HYPO-0066 (dangling-reference repair suggestion fails on referenced records; history records get none).

Round 2 (review fixes, uncommitted):
- validate returns all violations; commit compares per-record sets of (path, code) (QA A fixed; HYPO-0065).
- Kinds come from ID prefixes: link endpoint kinds, owner kind, evidence/criterion/assessment/experiment reference kinds and the full-ID rule are `invalid` (blocking). Consequence for QA E: a link can now only carry dangling_reference or cycle, both with repairs; archived links still skip the cycle rule. Chosen over skipping relational rules on archived links because those rules are decidable from the link alone, so they belong with its field rules, and it also closes the short-ID flip.
- Repair is {note, commands: [[argv]...]}; commands run in the project directory. Delete is offered only for links, after a restore note; records with a missing owner or other reference get the note only (architect blocker).
- Rejection message: "N error(s) block(s) writes (M more do/does not); run hyp check ...: path: message".
- Gotcha: round-1 src is what the index held before round 2 was staged; used `git checkout -- src` from the index for the red run.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Cross-record errors from merges, syncs or hand edits no longer block the writes that repair them. Diagnostics carry a stable code (malformed/attachment/invalid block all writes; cycle/dangling_reference/inconsistent do not), validation reports every violation per record, and a commit is rejected only if a record gains a (path, code) it did not have. hyp check prints repairs as {note, commands: [argv]}: restoring the missing record comes first, delete is offered only for links, never for records whose owner may still be arriving. Link endpoint-kind and short-ID checks became record-local (invalid). WebUI allows repair writes. Review: round 1 QA GO (gap), architect NO-GO (delete suggestion); fixed; confirmation QA GO, architect GO. Gate: 83 Rust tests + DOM, flake check. Follow-ups: HYPO-0065 (identity incl. referenced id), 0066, 0067.
<!-- SECTION:FINAL_SUMMARY:END -->
