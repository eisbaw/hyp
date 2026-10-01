---
id: HYPO-0013
title: 'Add fuzzing for the parser, validator and transaction engine'
status: In Progress
assignee:
  - '@implementer-c'
created_date: '2026-09-29 22:16'
updated_date: '2026-10-01 19:02'
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
- [x] #1 Fuzz or property targets for decode/encode round-trip, snapshot validation and commit atomicity, each checked into the repo
- [x] #2 Toolchain choice documented; runs inside `nix develop`
- [x] #3 `just fuzz` runs a time-bounded session; a short smoke run is suitable for CI
- [ ] #4 Any crashes found are fixed with regression tests, or filed as separate tasks
- [x] #5 A short fuzz/property smoke run is part of CI (moved from HYPO-0012)
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Stable-Rust proptest (dev-dependency, default features off) instead of cargo-fuzz: no nightly in the pinned nixpkgs.
2. tests/properties/ (one test target): generators (YAML-hostile text, records of every kind with references from a small ID pool), then codec, snapshot, commit, journal and fingerprint properties.
3. Case counts capped per property (PROPTEST_CASES overrides); just fuzz SECONDS CASES runs rounds with fresh seeds; the flake runs them with a fixed PROPTEST_RNG_SEED.
4. Show each property bites with a deliberate mutant in src/, then revert.
5. Fix tiny bugs found with regression tests; report others with an ignored failing test.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Toolchain: proptest 1.11 on stable Rust (default-features = false, features = [std]), not cargo-fuzz, which needs nightly; the pinned nixpkgs has none, and fenix/rust-overlay would add a second toolchain to the flake. Runs inside nix develop; Cargo.lock updated, --locked builds offline in the flake.
- Targets in tests/properties/: codec (decode(encode(r)) == r compared by Debug; decode never panics on damaged or random bytes, and whatever it accepts round-trips), snapshot (no query panics on arbitrary record sets incl. duplicates, dangling references, cycles, huge fields; reads from disk with stray, non-UTF-8 and copied files succeed with diagnostics, deterministically; records written are read exactly or reported), commit (arbitrary batches over a seeded notebook: a failed commit leaves hyp/ byte-identical and no journal; a successful one matches a fresh read and leaves no error; the real hyp check passes at the end), journal (crash after any prefix of applied files, recovery by a read or a write, ends in the full-journal state; a second recovery changes nothing), fingerprint (cosmetic edits keep every fingerprint; content edits of any basis member change it; end to end through the store, needs_review stays false after cosmetic patches and turns true after a source change).
- Default run: about 7 s wall for the properties binary on an idle machine. just fuzz SECONDS CASES for longer sessions. The flake package check runs them with PROPTEST_RNG_SEED fixed (reproducible); just test and just fuzz use fresh seeds and persist failures to tests/proptest-regressions/.
- Each property was shown to bite: it failed against a deliberate mutant in src/ (decode slicing without a check, a slice in validation, a read that fails on duplicate IDs, a commit that writes before validating, a commit that accepts invalid records, a recovery that appends, tags counted in the basis, evidence source or data hashes left out of the basis); src/ was restored after each.
- Bug found and fixed (tiny, isolated): decode passed the front matter without its last line break, so a YAML block scalar ending the front matter lost its final newline (prediction conditions ending in a newline read back without it). Fixed in store::decode; regression test codec::a_trailing_newline_in_the_last_front_matter_field_survives plus a persisted seed.
- Bug found, NOT fixed (needs a format decision): a multi-line value ending in U+2028 or U+2029 in the last front-matter field (prediction conditions, untestable reason, observed_at) makes serde_yaml end the header without a line feed, so hyp writes a record file it then reports as malformed, which blocks all writes. Pinned by the ignored test codec::a_last_field_ending_in_a_unicode_separator_is_readable; the round-trip property skips exactly that input class until it is fixed. Needs its own task (AC 4).

- Review follow-ups (mped-architect): encode now refuses a record whose YAML header would not end in a line feed (the U+2028/U+2029 case), so a write fails with a message and leaves the notebook as it was, instead of writing a file hyp cannot read (test codec::a_value_hyp_cannot_store_is_refused_not_written). Storing such values is still the open bug (ignored test). The commit property now also checks that each change was applied (created, updated, patched, archived and deleted records are in the returned snapshot as asked; a commit that reports changes wrote files; mutant: a commit that applies nothing fails it). The read-back property allows Malformed only for files not named after their ID. Cosmetic edits target records of a kind they fit. A single predicate (generate::unicode_separator_bug) marks the known-bug input.
- Seeds: tests/properties/main.rs fixes the seed (1013) unless PROPTEST_RNG_SEED is set, so just test, just e2e and the flake check are reproducible; just fuzz draws a fresh seed per round and prints it.
- CI smoke: the flake package check (nix flake check) runs the properties binary with the default case counts (12 passed, 1 ignored in the sandbox).

Correction to the earlier note that just test and just fuzz use fresh seeds: tests/properties/main.rs fixes the seed (1013) unless PROPTEST_RNG_SEED is set, so just test, just e2e and the flake check are reproducible with capped case counts (8 to 256 per property); only just fuzz draws a fresh seed per round and prints it. The flake no longer sets PROPTEST_RNG_SEED.

Coordinator follow-ups: eval-time Playwright pin assert; encode refusal is InvalidInput and names the field; text() yields long word lines for YAML folding; fingerprint::the_rich_basis_holds_every_record_built_for_it.

Closing review 2026-10-01: AC #1, #2, #3 and #5 are met (tests/properties; proptest on stable documented in the README; just fuzz SECONDS CASES; the flake package check runs the properties binary and GitHub CI runs nix flake check). AC #4 stays unchecked and the task In Progress: the trailing-newline bug is fixed with a regression test, but the U+2028/U+2029 storage bug is only refused, not fixed; it is filed as HYPO-0100, which this AC waits on. (Read literally, "or filed as separate tasks" is now satisfied; it is left open because the coordinator treats HYPO-0100 as blocking.)
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Property tests (proptest, stable Rust) cover the record format, validation, commits, crash recovery and the review fingerprint; they run in just test, just e2e, the flake check and CI, and just fuzz runs longer sessions with fresh seeds.

Changes (43b1130, 9dad9fc, 6f390ca; merged in d1e2fe9):
- tests/properties: codec (decode(encode(r)) == r; decode never panics on damaged or random bytes), snapshot (no query panics on arbitrary record sets; reads from disk report problems as diagnostics), commit (a failed batch leaves hyp/ byte-identical; a successful one is applied as asked and leaves no hyp check error), journal (recovery after a crash at any point is idempotent), fingerprint (cosmetic edits keep it, basis content edits change it).
- Toolchain: proptest instead of cargo-fuzz (nightly-only; the pinned nixpkgs has none). Fixed seed 1013 unless PROPTEST_RNG_SEED is set, capped case counts per property; just fuzz SECONDS CASES draws a fresh seed per round and prints it; failures persist to tests/proptest-regressions/.
- Each property was shown to fail against a deliberate mutant of src/.
- Bug fixed: decode dropped the final newline of a YAML block scalar ending the front matter (regression test). Bug refused, not fixed: a multi-line value ending in U+2028/U+2029 in the last front-matter field; encode now refuses it (InvalidInput naming the field) instead of writing an unreadable file; pinned by an ignored test.

Tests: the properties binary passes in just e2e and the flake check, with the known bug ignored. Gate at a38c06c green.

Open: AC #4 waits on HYPO-0100 (store the U+2028/U+2029 values). Follow-ups: HYPO-0115 (journal paths, partial recovery, a pinned journal, observed_at refusals).
<!-- SECTION:FINAL_SUMMARY:END -->
