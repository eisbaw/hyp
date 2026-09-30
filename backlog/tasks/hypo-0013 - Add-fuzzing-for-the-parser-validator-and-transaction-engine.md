---
id: HYPO-0013
title: 'Add fuzzing for the parser, validator and transaction engine'
status: To Do
assignee: []
created_date: '2026-09-29 22:16'
updated_date: '2026-09-30 05:33'
labels:
  - testing
dependencies:
  - HYPO-0011
  - HYPO-0009
  - HYPO-0008
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The on-disk format and `hyp apply` accept untrusted input: hand-edited Markdown, Git merges, and JSON from agents. The core invariants are well suited to fuzzing and property testing:
- `decode(encode(r)) == r`, and `decode` never panics on arbitrary bytes.
- `Snapshot::validate` / `read_unlocked` never panic on arbitrary record sets (dangling IDs, cycles, duplicate IDs, bad UTF-8, huge fields).
- `Store::commit` over arbitrary `Change` sequences is all-or-nothing: after an error the files on disk are unchanged, and after success `hyp check` reports no errors.
- Journal recovery after a simulated crash at any point is idempotent.

cargo-fuzz needs nightly Rust, which the pinned nixpkgs toolchain does not provide. Choose between a nightly toolchain in the flake (fenix/rust-overlay) and stable-compatible property testing (proptest/bolero).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Fuzz or property targets for decode/encode round-trip, snapshot validation and commit atomicity, each checked into the repo
- [ ] #2 Toolchain choice documented; runs inside `nix develop`
- [ ] #3 `just fuzz` runs a time-bounded session; a short smoke run is suitable for CI
- [ ] #4 Any crashes found are fixed with regression tests, or filed as separate tasks
- [ ] #5 A short fuzz/property smoke run is part of CI (moved from HYPO-0012)
<!-- AC:END -->
