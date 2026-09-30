---
id: decision-0005
title: >-
  Captured data records: kind 'data' (D-), attachments migrate, any record may
  reference data
date: '2026-09-30 21:43'
status: accepted
---
## Context

The tangible data behind an observation (a log excerpt, a capture file, command output, a measurement table, a screenshot) needs to be deep-copied with its metadata and stored so that many records, now and in later investigations, can reference the same bytes (HYPO-0090). Today `hyp evidence attach` copies a file into hyp/assets/<sha256> and records {path, sha256} on one evidence record only: no ID, no metadata, not referenceable elsewhere.

## Decision

- New record kind `data`, ID prefix `D-`, created by `hyp capture FILE|-` (stdin supports `cmd | hyp capture -`; hyp never runs commands itself). Metadata: title, origin (free text: path, URL, host or the command that produced it), captured_at, media type, size and sha256. The bytes live content-addressed under hyp/assets/ and are stored once per hash. A data record is immutable once captured.
- Any record kind may reference data records (evidence, runs, hypotheses, gaps and the others) through a common `data: [D-...]` reference list. One data record can be referenced from many records. Deleting a referenced data record is refused.
- Existing evidence attachments become data records: `hyp evidence attach` creates (or reuses, by hash) a data record and references it. Existing attachments are converted when the notebook is raised to the schema that introduces data records (decision-0004).
- Referenced data hashes are part of the review basis of the hypotheses whose basis includes the referencing records. Changed or missing bytes are reported by `hyp check`.

## Consequences

- A schema bump (decision-0004) with a migration of existing attachments, tested both ways (an old-schema notebook with attachments is read, then raised and converted on the first write that needs it).
- One mechanism for stored bytes; `attachments` on evidence is retired after migration.
- `hyp show D-...` shows metadata and every referrer; the bytes can be written back out; the WebUI lists data records with safe previews (no executable content served).
