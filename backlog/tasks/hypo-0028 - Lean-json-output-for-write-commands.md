---
id: HYPO-0028
title: Lean --json output for write commands
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 01:46'
updated_date: '2026-09-30 12:28'
labels:
  - cli
  - agents
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
From the HYPO-0016 architect review. `hyp --json add` (and every write command) prints {"ids": [...], "snapshot": {...}}: the whole project, including every frozen experiment plan. On a real notebook an agent that follows 'use --json for what you parse' fills its context on each write. The skill now tells agents to read plain output for writes; the JSON contract itself should be fixed. Related: HYPO-0024 (hyp apply output size).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Write commands with --json print only the affected IDs (with kinds) and the new project revision; no snapshot
- [x] #2 hyp apply --json output is equally compact (coordinate with HYPO-0024)
- [x] #3 Skill and README updated; CLI tests assert the output shape
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Store::commit reports the full ID and kind of each change it applied (creates included, whose IDs the server may assign)
2. Every write command, apply and evidence attach print {"ids":[{"id","kind"}],"revision"} with --json, one ID per line otherwise
3. README machine contract, SKILL.md; CLI tests assert the exact keys
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-09-30 implementation (uncommitted, awaiting review):
- Shape: `{"ids":[{"id":"H-…","kind":"hypothesis"}],"revision":"<project revision after the write>"}` for every write command, `apply` and `evidence attach`. Plain output: one full ID per line, in change order.
- Root cause of the old shape: only `Store::commit` knows the IDs it assigned (an apply create may omit `id`), so the CLI echoed the whole snapshot. New `Store::commit_ids` returns the named objects (`store::Affected`, full ID + kind) in change order; `commit` wraps it. `attach` returns `(Vec<Affected>, Snapshot)`.
- A no-op update/archive is still listed (it named the object). Delete lists the deleted ID.
- Behaviour change: plain `hyp apply` (without --json) used to print the snapshot JSON; it now prints IDs, one per line.
- `hyp --json init` keeps printing the whole snapshot (not a hot path; documented in README).
- `revision` is the value `apply --expected-revision` accepts (the test chains them).
- Test: tests/cli.rs json_writes_print_the_affected_ids_and_the_new_revision (exact keys for add, evidence add, assess, evidence attach, apply; red on the old binary).

2026-09-30 review fix round (contract decided by the orchestrator):
- Shape is now `{"written":[{"id","kind","revision"}],"revision"}`: `ids` renamed to `written`; each entry carries the record's revision after the write (null after a delete), so an agent can create and then update/archive without a `show`. Same for every write command, `apply`, `evidence attach` and now `hyp --json init` (lists every record of the new project; no snapshot any more).
- `Store::commit_written` returns `store::Committed { written: Vec<Written>, snapshot }` (replaces the round-1 tuple and `commit_ids`); `attach` returns `Committed`.
- Test json_writes_print_what_they_wrote_and_the_new_revisions chains create -> archive -> delete through the printed revisions and checks init --json; red on the round-1 build.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Committed in the CLI output ergonomics batch (see commit message). Review: QA GO and architect GO, one fix round (contract: 'written' with per-object revisions, init --json same shape, exit 2 for bad filter combinations), then confirmation QA GO and architect GO. Gate: 75 Rust tests plus DOM, nix flake check.
<!-- SECTION:FINAL_SUMMARY:END -->
