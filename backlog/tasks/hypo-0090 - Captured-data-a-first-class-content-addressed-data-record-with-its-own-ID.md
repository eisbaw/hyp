---
id: HYPO-0090
title: 'Captured data: a first-class, content-addressed data record with its own ID'
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 20:09'
updated_date: '2026-10-01 00:01'
labels:
  - feature
  - agents
  - storage
dependencies:
  - HYPO-0085
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
User request (2026-09-30): the tangible data behind an observation (a log excerpt, a capture file, command output, a measurement table, a screenshot) must be deep-copied with its metadata and stored as its own record, with its own ID, so that many observations, hypotheses and future investigations can reference the same data.

Today `hyp evidence attach E FILE` copies a file into hyp/assets/<sha256> and records {path, sha256} on that one evidence record. The data has no ID, no metadata (origin, when captured, media type, size, how it was produced), cannot be referenced from anything else, and cannot exist before an observation does.

Proposal (to confirm in a short decision record before implementing):
- New record kind for captured data, e.g. `data` with prefix `D-` (name to decide). Metadata: title, origin (path, URL, host or the command that produced it, as text; hyp never runs commands), captured_at, media type, size, sha256, optional note. The bytes are stored content-addressed under hyp/assets/ (deduplicated by hash), and the record is immutable once captured, like runs.
- `hyp capture FILE|- --origin "..." [--title ...]` deep-copies the bytes (stdin supports `cmd | hyp capture -`) and prints the D- ID. Size limit as for attachments (32 MiB), with a clear error.
- Observations (evidence), and possibly runs and gaps, reference data by ID (`--data D-...`, repeatable). One D- record can be referenced from many places; delete is refused while it is referenced.
- The review basis includes each referenced data record's hash, so changed data flags review (consistent with evidence provenance in decision-0003).
- `hyp show D-...` shows the metadata and every record that references it. `hyp data get D-... > file` (or `hyp show --raw`) writes the bytes back out. The WebUI lists data records and offers text previews, served safely (no executable content, as for attachments today).
- Migration: existing evidence attachments become data records, or keep working as they are. Decide and test.
- This is a new kind, so a schema bump per decision-0004 (depends on HYPO-0085).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A decision record fixes the name, the metadata fields, immutability and the migration of existing attachments
- [x] #2 hyp capture deep-copies a file or stdin into a content-addressed data record with metadata and prints its ID; identical bytes are stored once
- [x] #3 Evidence (and any other kinds agreed in the decision) can reference data records by ID; one data record can be referenced from many records, and deleting a referenced data record is refused
- [x] #4 Referenced data hashes are part of the review basis; changed or missing data bytes are reported by hyp check
- [x] #5 hyp show D-... shows metadata and referrers; the bytes can be written back out; the WebUI lists data records with safe previews
- [x] #6 Schema bump and tests per decision-0004; skill and README document capture
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. model: Kind::Data (D-, hyp/data/), Data::Captured {origin, captured_at, media_type, size, sha256}; Record-level `data: [D-...]` refs (skip when empty); Record::references/references_mut/schema; validation; basis adds data hashes (evidence keeps key `attachments` so migration leaves fingerprints unchanged; other kinds get `data` only when non-empty).
2. store: SCHEMAS row 3 (0.3.0); data dir; blob verification for D- (code attachment + repair note); D- immutable (update/patch refused, archive ok); create path sets captured_at/size/media type from the blob; migration of evidence attachments inside the journaled commit when the result is schema >= 3; capture() and attach() as capture+reference; read_config parameterised for a schema test.
3. cli: hyp capture, hyp data get, --data on observe/evidence add/add/predict/falsify-if/gap/run/assess/set; show D- with referrers; apply help.
4. web: data view with metadata and escaped text/* preview (derived at read, never raw bytes); detail shows data refs.
5. docs: README (Files, schema table, model, contract), skill paragraph, version 0.3.0.
6. tests: red/green for each listed behaviour; DOM test for the data list.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- AC1 is decision-0005 (accepted before this work); choices left open there, decided here: capture reuses an existing D- only when bytes, title, origin, media type and note are all equal (a retried capture writes nothing); attach reuses a referenced or any non-archived D- with the same hash; migration makes one D- per distinct hash and reuses an existing one.
- data refs live in the Record header (`data`, Rust field `data_refs`), skipped while empty, so schema-1/2 files are unchanged.
- Basis: evidence keeps its key `attachments` (legacy hashes, then referenced data hashes); other kinds get `data` only when non-empty. Result: no existing fingerprint changes, and the migration changes none either (pinned in the golden test).
- Migration runs inside commit_written whenever the resulting schema is >= 3 and some evidence still has attachments (also after a merge from an older branch), in the same journal as the raise.
- Every test was shown red by a mutation (scratchpad mutate.py): 17 Rust mutations and 2 app.js mutations each fail their test.
- Follow-ups filed: HYPO-0094 (unreferenced stored bytes), HYPO-0095 (WebUI forms for data refs), HYPO-0096 (clear data refs via hyp set); notes added to HYPO-0004 (per-read hashing) and HYPO-0089 (VALIDATION.md for 0.3.0).

Fix round after deep review (architect NO-GO):
- Deterministic migration: D- ID = UUIDv5(hyp namespace, sha256) via uuid feature v5 (adds sha1_smol, a tiny zero-dependency crate, to Cargo.lock); title "Migrated attachment <sha8>"; times = earliest created_at of the evidence holding the bytes; evidence updated_at kept. attach of bytes a legacy attachment holds uses the derived ID. Two copies migrate to identical files (test).
- Capture reuse and attach reuse-by-hash decided under the write lock (Store::commit_planned); parallel identical captures and attaches make one record (test).
- check: size mismatch with intact bytes is `invalid` with a record-file note; legacy attachment symlinks and any symlink in hyp/assets are `attachment`.
- Duplicate IDs in `data` are invalid; `data get --output` refuses hyp/ and writes atomically; empty capture needs --allow-empty; --json writes list converted records as `migrated`, and the stderr notice names converted evidence, saying "raised" only on a raise.

Confirmation round: data get --output also refuses .hyp/; the hyp/assets symlink scan covers only SHA-256-named entries and skips blobs a data record already reported; attach of bytes the evidence already holds (data ref or legacy attachment) writes nothing and keeps updated_at (legacy case prints only the evidence).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Captured data records per decision-0005 in hyp 0.3.0 (schema 3): hyp capture, D- records, data refs on every kind, review basis includes referenced data, hyp check on bytes, hyp data get, WebUI data view, deterministic migration of evidence attachments. Deep review: QA GO, adversarial scout (no data loss), architect NO-GO (non-deterministic migration) -> fixed with UUIDv5 IDs and notebook-derived timestamps, plus capture idempotence under the lock, check classification, duplicate refs, data get guards (hyp/ and .hyp/), empty capture refusal, migration in --json; confirmation QA GO, architect GO (two migrated copies merge in Git without conflicts); final QA gate GO. Codex not available. Gate: 156 Rust tests + DOM, nix flake check.
<!-- SECTION:FINAL_SUMMARY:END -->
