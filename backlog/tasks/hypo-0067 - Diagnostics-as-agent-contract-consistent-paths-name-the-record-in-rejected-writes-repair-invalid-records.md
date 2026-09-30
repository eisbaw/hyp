---
id: HYPO-0067
title: >-
  Diagnostics as agent contract: consistent paths, name the record in rejected
  writes, repair invalid records
status: To Do
assignee: []
created_date: '2026-09-30 12:59'
updated_date: '2026-09-30 18:20'
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
- [ ] #1 All diagnostics use one path form (hyp/<dir>/<id>.md or the asset path under hyp/); documented
- [ ] #2 Rejected-write errors name the record and code of each new error
- [ ] #3 An invalid record can be repaired through the tool (e.g. hyp set/edit on that record is allowed if it removes the error)
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From the HYPO-0003 confirmation (architect): wrong-kind superseded assessments or criteria still say 'belongs to another hypothesis' (say 'is not an assessment/criterion'); attachment errors block every write, although a sync still delivering assets/ is the same 'only late' case; conflict copies from sync tools (Syncthing, Dropbox) probably block every write (unverified); Repair.note is always set, so it could be a plain String.

From the agent-UX batch 2 reviews: a gap citing missing evidence is a dangling_reference with a note but empty repair.commands; offer 'hyp set G --resolved false'. A failing 'hyp --json check' ends with {"error":"validation failed","kind":"invalid_input"} (consider a dedicated kind or none). The WebUI's plain-text 4xx rejections (axum) and Host/Origin 403s never carry kind.
<!-- SECTION:NOTES:END -->
