---
id: HYPO-0114
title: 'WebUI Bears on: the stance is said twice for a qualifying link'
status: To Do
assignee: []
created_date: '2026-10-01 19:00'
labels:
  - webui
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
On an evidence page, each Bears on row (bearsOnSection in web/app.js) shows a stance badge and then each link's meaning text. For a qualifying link the row reads "qualifies" (badge) next to "qualifies H…" (meaning), which says the same thing twice and pushes the judgment and lifecycle badges apart.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A Bears on row states each stance once, for every stance (for, against, qualifies, mixed)
- [ ] #2 DOM test asserts it for a qualifying link
<!-- AC:END -->
