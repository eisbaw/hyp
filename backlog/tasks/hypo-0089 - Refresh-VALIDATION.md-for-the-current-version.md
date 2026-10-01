---
id: HYPO-0089
title: Refresh VALIDATION.md for the current version
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 19:54'
updated_date: '2026-10-01 19:02'
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

Closing review 2026-10-01: AC #1 is met as written: VALIDATION.md names what it validates (9a62a67, crate version 0.3.0) and the checks run for it and the commits before it. It is not current for 0.4.0 (a38c06c), which was released after this task's work; regenerating it for 0.4.0, with the browser suite, property tests and CI, is HYPO-0107. Closed here rather than reopened, so the 0.3.0 refresh and the 0.4.0 one are tracked separately.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
VALIDATION.md was rewritten for 0.3.0 and scoped to commit 9a62a67: what was checked there and at the 0.2.0 and 0.3.0 commits before it, and what was not.

Changes (193f872, a6d5c1c; merged in 865934e):
- Title and first line scoped to 9a62a67; by-hand checks and reviews attributed to the commits they ran on (25fde81, ad113d5, 18c2a3f, 9a62a67).
- States what was not checked: no cross-model review since 0.2.0, no recorded aarch64-linux build, the browser suite never completed (true at the time), NFS, mixed versions; and when and on which binary the jsdom recovery flake was seen.
- Records that 9a62a67 changed read locking without a version bump, so two builds reported 0.3.0 (fixed by the 0.4.0 release).

Gate: nix flake check -L passed (aarch64-linux omitted).

Follow-up: VALIDATION.md is not current for 0.4.0 and still says the browser suite never completed; HYPO-0107 regenerates it (HYPO-0017 AC #3 waits on that).
<!-- SECTION:FINAL_SUMMARY:END -->
