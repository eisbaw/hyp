---
id: HYPO-0113
title: 'locate_changes: skip creates instead of matching them by an empty ID'
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
store::locate_changes marks each diagnostic of a refused write with the change that names its record, by comparing the diagnostic path's file stem with each change's full ID. In the blocked-write path of Store::repair_scope a create has no ID yet and is passed as "" (unwrap_or_default), so a diagnostic whose path has an empty stem (a path ending in "/" or "/.md") would be attributed to the last create. Nothing produces such a path today; the match should not depend on that.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Creates are left out of the ID match (they are located by batch-local reference or not at all), so no diagnostic can match an empty ID
- [ ] #2 Unit test: a diagnostic with an empty file stem gets no change index when the write contains a create
<!-- AC:END -->
