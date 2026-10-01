---
id: HYPO-0042
title: 'WebUI: long unbroken titles make every page scroll horizontally'
status: In Progress
assignee:
  - '@implementer-A'
created_date: '2026-09-30 11:26'
updated_date: '2026-10-01 10:30'
labels:
  - webui
  - bug
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Browser test-drive 2026-09-30. A hypothesis title with a ~95-character path (no spaces) makes the document 1741px wide at 1440px and 1237px at 390px; cards reach 798px on a phone and push Edit/Archive off-screen. Agents often put paths and identifiers in titles. Screenshot: /tmp/claude-1000/-home-mpedersen-topics-hyp/b93cfa51-31aa-4941-a678-5890abf7545d/scratchpad/webtd/26-longtitle-390.jpg
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Titles, IDs and sources wrap (e.g. overflow-wrap: anywhere) so no view scrolls horizontally at 390px or 1440px
- [x] #2 Checked with a long-path title in the DOM or browser test
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- web/style.css: body { overflow-wrap: anywhere } (inherited), so titles, IDs and sources break inside long unbroken strings; anywhere (not break-word) also lowers min-content, so flex/grid items shrink.
- DOM test longTitles(): a 115-character path title; asserts the computed overflow-wrap of the record h1, the overview card title and the card ID (jsdom has no layout); red without the rule.
- Real browser (headless Brave on the exported report, a 390 px srcdoc iframe since headless clamps windows to 500 px): with the rule, scrollWidth equals clientWidth on overview, evidence, hypothesis and evidence pages, matrix, graph, all, experiments, data at 390 and 1440 px; without it 669-745 px at 390. The export hides edit buttons, so the live page's action buttons were not in that measurement.
- Follow-up for the Playwright suite (scripts/browser-test.cjs, implementer C): assert no horizontal scroll with a long-path title at 390 and 1440.

- Review fix: the body-wide overflow-wrap: anywhere was inherited by buttons and badges, so at 390 px 'Edit record'/'Archive' broke one letter per line beside a long title, and 'Edit' broke at 1440. Buttons, badges, tags and meta-line labels now use overflow-wrap: normal; .section-head wraps. Live Brave via the DevTools MCP, all views at 390 and 1440: with the old rule re-inserted via CSSOM, buttons broke on 7 views at 390 and 1 at 1440 (Edit button 39x190 px); with the fix none, and scrollWidth equals clientWidth on every view. DOM test asserts the computed overflow-wrap of a button, a badge and a meta-line label.
<!-- SECTION:NOTES:END -->
