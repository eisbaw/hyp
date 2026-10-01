---
id: HYPO-0041
title: 'WebUI: relationship card names the page''s own hypothesis on the ''to'' side'
status: Done
assignee:
  - '@implementer-A'
created_date: '2026-09-30 11:26'
updated_date: '2026-10-01 19:01'
labels:
  - webui
  - bug
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Browser test-drive 2026-09-30 (headless Brave, demo notebook). On the page of the link's 'to' hypothesis (bus contention), the 'Alternative explanation' card reads 'competes with DMA timeout is caused by bus contention' and links to itself; it should name the 'from' record. The 'from' side is correct. Screenshot: /tmp/claude-1000/-home-mpedersen-topics-hyp/b93cfa51-31aa-4941-a678-5890abf7545d/scratchpad/webtd/09-hyp2-1440.jpg
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Relationship cards name the other end of the link on both the from and the to page
- [x] #2 DOM test covers both sides
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- web/app.js detail(): the Relationships card names the other end: 'This hypothesis <rel> <to>' on the from page, '<from> <rel> this hypothesis' on the to page.
- DOM test relationshipEnds() covers both pages; red when the to side names its own hypothesis.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
A hypothesis page's Relationships card now names the other end of each link on both sides.

Changes (bfe3e40): web/app.js detail() renders "This hypothesis <relation> X" on the from page and "X <relation> this hypothesis" on the to page; the to page used to name itself and link to itself.

Tests: DOM test relationshipEnds() covers both pages and is red when the to side names its own hypothesis. just e2e and nix flake check green at a38c06c.
<!-- SECTION:FINAL_SUMMARY:END -->
