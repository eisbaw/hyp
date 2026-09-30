---
id: HYPO-0064
title: 'hyp delete: correct advice for referenced records'
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 12:36'
updated_date: '2026-09-30 13:56'
labels:
  - bug
  - cli
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Interactive terminal test-drive 2026-09-30 (xterm+tmux, first-time user, release build of fda0faa). `hyp delete` on an archived hypothesis that is still referenced says 'object is referenced; archive it instead', although it is already archived, and does not say what refers to it. On an unarchived record it says only 'archive before deleting'.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The referenced error lists the referencing IDs (kind and title) and says those must be deleted first, or that archiving is enough
- [x] #2 The not-archived error suggests the archive command
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Delete checks: historical, then referenced (lists referrers via model::listing; advice depends on archived state), then not-archived ("archive ID before deleting it: hyp archive ID").
- Test: deleting_a_referenced_record_lists_what_refers_to_it; workflow archived_references_still_prevent_deletion updated to the new message.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Committed in human CLI batch 1 (see commit message). Review: QA GO and architect GO; small cleanup round (wrap_help dropped, web relation spelling, --project help, multi-line-title repair note); confirmation QA GO, architect GO. Gate: 90 Rust tests + DOM, nix flake check.
<!-- SECTION:FINAL_SUMMARY:END -->
