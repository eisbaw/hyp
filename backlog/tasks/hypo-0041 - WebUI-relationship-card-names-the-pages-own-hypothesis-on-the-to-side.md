---
id: HYPO-0041
title: 'WebUI: relationship card names the page''s own hypothesis on the ''to'' side'
status: To Do
assignee: []
created_date: '2026-09-30 11:26'
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
- [ ] #1 Relationship cards name the other end of the link on both the from and the to page
- [ ] #2 DOM test covers both sides
<!-- AC:END -->
