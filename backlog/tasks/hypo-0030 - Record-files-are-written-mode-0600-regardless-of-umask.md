---
id: HYPO-0030
title: Record files are written mode 0600 regardless of umask
status: To Do
assignee: []
created_date: '2026-09-30 01:51'
labels:
  - storage
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
store::atomic writes through tempfile::NamedTempFile, which creates the file 0600, and persist() keeps that mode. Every file under hyp/ (records, config.toml) therefore ends up 0600 whatever the umask, so a group-shared project directory, or another user's read-only tool, cannot read records a teammate wrote; files that arrive by copy or clone get the usual 0644. Found in HYPO-0016 review: the agent skill files had the same problem and now use store::atomic_readable (0644 less the umask). Decide whether records should follow the umask too (probably yes: hyp is not a secrets store), then switch atomic to it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Files hyp writes under hyp/ get 0644 less the umask, or the choice to keep 0600 is documented with its reason
<!-- AC:END -->
