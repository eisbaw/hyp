---
id: HYPO-0102
title: 'WebUI: repair an invalid record while writes are blocked'
status: To Do
assignee: []
created_date: '2026-10-01 18:58'
labels:
  - webui
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Since HYPO-0067 an invalid record no longer blocks a write that only repairs it, through the CLI and hyp apply. The WebUI cannot use this: while blocking diagnostics exist, refresh() in web/app.js keeps the last readable state ("Showing the last readable state; writes are blocked"), so the invalid record's current content is neither shown nor editable, and an edit would start from stale content. A human steering through the WebUI has to drop to the CLI to repair a hand edit or merge.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 While only invalid records block writes, the WebUI shows each invalid record's current content and offers an edit that is saved as a repair
- [ ] #2 A repair saved from the WebUI is accepted and the page returns to Live; a save that leaves the record invalid shows the server's diagnostics
- [ ] #3 Edits to other records stay blocked, with the reason shown
- [ ] #4 DOM test: a record whose title was emptied by hand is repaired from the WebUI
<!-- AC:END -->
