---
id: HYPO-0008
title: Replace unmaintained dependencies serde_yaml and fs2
status: To Do
assignee: []
created_date: '2026-09-29 22:15'
updated_date: '2026-10-01 00:35'
labels:
  - dependencies
dependencies:
  - HYPO-0009
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found in the initial review. `serde_yaml` 0.9 is deprecated and archived upstream. `fs2` has not been released since 2018, and `std::fs::File::lock` has been stable since Rust 1.89 (the flake already pins 1.95; `rust-version` says 1.85).

Front matter is the durable on-disk format, so a replacement YAML crate must produce byte-identical output for existing records, or the migration must be explicit (every revision hash changes and every assessment gets flagged needs-review).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 File locking uses std `File::lock`; `rust-version` bumped accordingly
- [ ] #2 YAML crate replaced with a maintained one, or the decision to keep serde_yaml is recorded with rationale
- [ ] #3 A golden-file test proves existing record files round-trip byte-identically, so revisions and assessment fingerprints do not change
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Do this together with HYPO-0009 (and HYPO-0005) so users see a single needs-review wave. If HYPO-0009 makes the fingerprint semantic, the byte-identical requirement in AC#3 can be relaxed. Check RustSec before choosing a YAML crate (serde_yml reportedly has an advisory; verify).

HYPO-0004: reads take fs2's shared lock (FileExt::lock_shared, called by path because std's inherent File::lock_shared, stable since Rust 1.89, shadows it and is newer than the MSRV 1.85). Replacing fs2 with std file locks needs the MSRV raised to 1.89.
<!-- SECTION:NOTES:END -->
