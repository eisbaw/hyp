---
id: HYPO-0090
title: 'Captured data: a first-class, content-addressed data record with its own ID'
status: To Do
assignee: []
created_date: '2026-09-30 20:09'
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
- [ ] #1 A decision record fixes the name, the metadata fields, immutability and the migration of existing attachments
- [ ] #2 hyp capture deep-copies a file or stdin into a content-addressed data record with metadata and prints its ID; identical bytes are stored once
- [ ] #3 Evidence (and any other kinds agreed in the decision) can reference data records by ID; one data record can be referenced from many records, and deleting a referenced data record is refused
- [ ] #4 Referenced data hashes are part of the review basis; changed or missing data bytes are reported by hyp check
- [ ] #5 hyp show D-... shows metadata and referrers; the bytes can be written back out; the WebUI lists data records with safe previews
- [ ] #6 Schema bump and tests per decision-0004; skill and README document capture
<!-- AC:END -->
