---
id: HYPO-0116
title: 'WebUI: show a run''s frozen plan as fields'
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
A run freezes its plan as the experiment's encoded Markdown record (Store::commit), not as JSON like frozen targets. frozenFields in web/app.js falls back to <pre> for it, so the run page shows the raw front matter and body. b328f29 corrected the earlier claim (HYPO-0045) that the plan rendered as fields.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A run page shows its frozen plan's fields (title, hypothesis, targets, status, body) the way a frozen target is shown
- [ ] #2 The frozen plan is decoded in one place, by the server's record decoder; the WebUI does not parse front matter
- [ ] #3 DOM test covers a run's frozen plan
<!-- AC:END -->
