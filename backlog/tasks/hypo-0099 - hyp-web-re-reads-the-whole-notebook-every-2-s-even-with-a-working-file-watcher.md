---
id: HYPO-0099
title: hyp web re-reads the whole notebook every 2 s even with a working file watcher
status: To Do
assignee: []
created_date: '2026-10-01 00:35'
updated_date: '2026-10-01 00:57'
labels:
  - performance
  - web
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while implementing HYPO-0004. The monitor in src/web.rs reads a snapshot on every file event and also on a 2 s tick, as a fallback for watcher errors. After HYPO-0004 a read no longer hashes stored bytes and file opens no longer count as events, so idle CPU is ~0.2% with 200 records (release), but the tick still parses every record file every 2 s, which grows with the notebook. A tick only while the watcher has reported an error (or a much slower one) would make idle cost independent of notebook size.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 An idle hyp web with a healthy watcher reads no record file for at least 30 s
- [ ] #2 Changes are still noticed within 2 s after a watcher error, and the existing SSE/DOM tests pass
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From the HYPO-0004 reviews: hyp check hashes every stored byte while holding the shared apply lock, so writers wait for the whole hash; stored bytes are immutable and named by hash, so hashing can happen after releasing the lock. ensure_layout runs on open and every read (a dozen mkdir per read). The blobs scan keyed by (sha, size) is O(assets x blobs). On NFS, flock degrades to per-process POSIX locks, so threads inside hyp web do not exclude each other (pre-existing).
<!-- SECTION:NOTES:END -->
