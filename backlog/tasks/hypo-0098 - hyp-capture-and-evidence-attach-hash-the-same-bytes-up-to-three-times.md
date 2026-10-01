---
id: HYPO-0098
title: hyp capture and evidence attach hash the same bytes up to three times
status: To Do
assignee: []
created_date: '2026-10-01 00:35'
labels:
  - performance
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while implementing HYPO-0004. A capture hashes its bytes in memory (store_blob), hashes an existing stored copy again to decide whether to keep it (Store::blob in store_blob), and the commit hashes the stored file once more when it creates the data record (Data::Captured in commit_planned). For a 32 MiB capture that is about 3 x 32 MiB of SHA-256 per write (~0.15 s release, more in debug). Writes are rare, so this is low priority; reads no longer hash at all.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A capture of new bytes hashes them at most twice (once in memory, once as stored before the record is committed), and a capture of bytes already stored hashes the stored copy once
- [ ] #2 A stored copy changed in place is still replaced or refused, as now (tests/data.rs)
<!-- AC:END -->
