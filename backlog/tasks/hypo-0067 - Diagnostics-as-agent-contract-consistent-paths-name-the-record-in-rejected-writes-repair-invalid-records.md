---
id: HYPO-0067
title: >-
  Diagnostics as agent contract: consistent paths, name the record in rejected
  writes, repair invalid records
status: In Progress
assignee:
  - '@implementer-B'
created_date: '2026-09-30 12:59'
updated_date: '2026-10-01 09:31'
labels:
  - agents
  - validation
dependencies:
  - HYPO-0003
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
From the HYPO-0003 architect review. (1) Diagnostic identity is (path, code), but paths are inconsistent: no_criterion uses a bare ID, attachment uses assets/... relative to hyp/, the rest hyp/.... (2) A rejected write reports only the violation message, not which record gained the error; now a write can add an error on another record (archiving a criterion trips 'investigating requires a criterion' on its hypothesis). (3) invalid (record-local) errors block all writes and cannot be repaired through the tool, e.g. a hand edit that empties a title.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 All diagnostics use one path form (hyp/<dir>/<id>.md or the asset path under hyp/); documented
- [x] #2 Rejected-write errors name the record and code of each new error
- [x] #3 An invalid record can be repaired through the tool (e.g. hyp set/edit on that record is allowed if it removes the error)
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From the HYPO-0003 confirmation (architect): wrong-kind superseded assessments or criteria still say 'belongs to another hypothesis' (say 'is not an assessment/criterion'); attachment errors block every write, although a sync still delivering assets/ is the same 'only late' case; conflict copies from sync tools (Syncthing, Dropbox) probably block every write (unverified); Repair.note is always set, so it could be a plain String.

From the agent-UX batch 2 reviews: a gap citing missing evidence is a dangling_reference with a note but empty repair.commands; offer 'hyp set G --resolved false'. A failing 'hyp --json check' ends with {"error":"validation failed","kind":"invalid_input"} (consider a dedicated kind or none). The WebUI's plain-text 4xx rejections (axum) and Host/Origin 403s never carry kind.

2026-10-01 implementation (implementer B): (1) Paths: no_criterion now names hyp/hypotheses/<id>.md; legacy attachment diagnostics hyp/assets/<sha>. Every diagnostic path is relative to the project root under hyp/ (README, Diagnostic doc).

(2) Rejected writes: commit collects every new (path, code); message 'nothing was written: the write would add N errors: <path> (<code>): <message>; ...'; the --json error body gains 'diagnostics' in the hyp check form. Blocked errors carry the blocking diagnostics too (error::Classified.diagnostics).

(3) Repair: while only invalid records block, a write that changes only invalid records (config.toml excluded) and leaves each valid is accepted (Store::assert_repair_scope, assert_repaired); one that leaves a changed record invalid is blocked, naming what is still wrong. Malformed and attachment still block everything. Invalid diagnostics get a default repair note (hyp edit/set/patch; immutable kinds: fix the file).

(4) Notes items: a gap with a dangling resolved_by gets commands (hyp set G --by <remaining>, or --resolved false); a failing hyp check is kind check_failed; wrong-kind messages now say '<id> is not a criterion' and 'superseded <id> is not an assessment'. Not done: WebUI plain-text 4xx/403 without kind; late-sync attachments; sync conflict copies; Repair.note as plain String.

Review follow-up (mped-architect): repair scope is now decided from the changes right after planning (Store::repair_scope), so a blocked write is reported as blocked before preconditions or other checks; a no-op or still-invalid repair is blocked naming what is wrong; deleting an archived invalid record counts as a repair; migration-converted records are not limited. Diagnostics of a rejected write carry repair null (nothing stored). The blocked hint about repair appears only when a blocking record is changeable.

Review round 2: rejected/blocked write diagnostics about a record a change names carry change (its index) and ref (batch-local reference); delete while blocked says to repair instead of archive; assert_writable runs on one path (commit_planned before planning; repair_scope assumes only invalid records block).
<!-- SECTION:NOTES:END -->
