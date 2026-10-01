---
id: HYPO-0103
title: 'Store: a link''s ends cannot change on update or patch'
status: To Do
assignee: []
created_date: '2026-10-01 18:58'
labels:
  - contract
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The WebUI refuses to change a link's from or to (ENDPOINTS_FIXED in web/app.js: no fields, advanced JSON rejected; HYPO-0045), but the store does not: check_update in src/store.rs checks kind, immutable kinds, created_at and experiment targets only, so hyp apply update or patch can still re-point a link. The README says link ends are fixed in the WebUI only. A rule that one client enforces is advice, not a tool rule (decision-0002), and re-pointing a link silently changes what evidence bears on which hypothesis.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 hyp apply update or patch that changes a link's from or to is refused (nothing written) with an error naming the link and saying to link anew and archive
- [ ] #2 The WebUI-only check is removed; the WebUI shows the server's refusal
- [ ] #3 Tests cover update and patch; the DOM test still passes
- [ ] #4 README states that link ends are immutable for every writer
<!-- AC:END -->
