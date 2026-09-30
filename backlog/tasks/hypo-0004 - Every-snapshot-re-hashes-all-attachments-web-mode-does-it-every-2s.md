---
id: HYPO-0004
title: Every snapshot re-hashes all attachments (web mode does it every 2s)
status: To Do
assignee: []
created_date: '2026-09-29 22:15'
updated_date: '2026-09-30 23:15'
labels:
  - performance
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found in the initial review. Measured.

`read_unlocked` reads and SHA-256 hashes every attachment on every snapshot. `commit` does a full read three times, plus another hash pass during validation. The web server polls a snapshot every 2s and on each file event, and every browser tab requests one on each SSE change. Each read also takes the exclusive write lock.

Measured `hyp list` with six 30 MB attachments: 0.28-0.35s release (4.2s debug), vs 0.01s without. This scales linearly with attachment volume, and web mode burns CPU continuously. Attachments are content-addressed and immutable, so full re-hashing on every read is unnecessary.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `hyp check` still detects a modified or missing attachment
- [ ] #2 A single commit performs at most two full reads of the notebook
- [ ] #3 Benchmark note in the commit message with before/after timings
- [ ] #4 Ordinary snapshots only check attachment existence and containment (no bytes read); hashing happens in `hyp check` and when committing evidence that references the attachment. No stored hash cache
- [ ] #5 Reads do not take the exclusive write lock (shared lock or lock-free read with recovery only under the write lock), so the 2s web poll, open tabs and writers do not queue behind each other; a notebook on read-only media can be read
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
HYPO-0090: data records' stored bytes (hyp/assets/<sha256>, up to 32 MiB each) are now verified the same way on every read, once per distinct hash, so this cost grows with captured data.

Raised to high after the HYPO-0090 review: with one 32 MiB capture a debug /api/snapshot takes 4.5-9.6 s and idle hyp web uses ~84% of a core; every read hashes all stored bytes, a write reads the notebook three times. Cheap fixes: per-process hash cache keyed by (inode, len, mtime); skip byte verification in the pre-write re-read.
<!-- SECTION:NOTES:END -->
