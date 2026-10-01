---
id: HYPO-0110
title: Late-arriving stored bytes and sync conflict copies block every write
status: To Do
assignee: []
created_date: '2026-10-01 18:59'
labels:
  - hardening
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
From the HYPO-0003 and HYPO-0067 reviews, not done there. hyp is meant to live in synced or shared folders (decision-0001).
- A sync tool still delivering hyp/assets/ (a record that arrives before its stored bytes) gives an attachment diagnostic (missing or wrong-length bytes), which blocks every write, although it is the same "only late" case that cross-record rules treat as non-blocking.
- Conflict copies written by Syncthing (*.sync-conflict-*) or Dropbox ("… (conflicted copy …)") in a record directory or in hyp/assets/ probably make the project malformed and block every write (unverified).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Tests pin what a Syncthing and a Dropbox conflict copy in each record directory and in hyp/assets/ does today
- [ ] #2 A conflict copy does not block writes; hyp check reports it with its own code and a repair note (compare, then delete)
- [ ] #3 Missing stored bytes block only writes that cite, hash or migrate them; other writes go on, and hyp check still reports the missing bytes (or the decision to keep blocking is recorded with its reason)
- [ ] #4 README diagnostics section documents both cases
<!-- AC:END -->
