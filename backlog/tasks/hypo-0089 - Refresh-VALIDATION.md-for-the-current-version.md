---
id: HYPO-0089
title: Refresh VALIDATION.md for the current version
status: In Progress
assignee:
  - '@claude'
created_date: '2026-09-30 19:54'
updated_date: '2026-10-01 08:13'
labels:
  - docs
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found in HYPO-0085: the crate is now 0.2.0 (schema 2 needs hyp >= 0.2.0), while VALIDATION.md is titled and written for 0.1.0.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 VALIDATION.md names the version it validates and the checks run for it
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
HYPO-0090 bumped the crate to 0.3.0 (schema 3: data records); refresh VALIDATION.md for that version instead.

- VALIDATION.md rewritten for 0.3.0 at 9a62a67 from the verification notes: what was checked (fmt, clippy, e2e, flake check, hands-on scenarios, review rounds) and what was not (Codex cross-model review unavailable since 0.2.0, aarch64 never built, browser suite not run since 0.1.0 with a TODO for HYPO-0017, NFS, mixed versions).
- Found: 9a62a67 changed the read locking without a version bump, so two different builds report 0.3.0 while the README warns that hyp 0.3.0 and earlier must not share a notebook with this version. Stated in VALIDATION.md; a version bump is a code change outside this task.

- Review round: scoped to 9a62a67 (title and first line), by-hand checks attributed to 25fde81, ad113d5 and 18c2a3f, no ARM build recorded, browser suite never completed, jsdom recovery flake dated and attributed to the 9a62a67 binary under parallel load; the future observed_at limit dropped (fixed on another branch). To be regenerated against the merged HEAD at the end.
<!-- SECTION:NOTES:END -->
