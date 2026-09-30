---
id: HYPO-0039
title: 'hyp graph: labels lose < > [ ] characters'
status: To Do
assignee: []
created_date: '2026-09-30 02:26'
labels:
  - cli
  - ux
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Test-drive 2026-09-30. `hyp graph` replaces < > [ ] and newlines in titles with spaces, so 'Resolver latency > 5s in logs' renders as 'Resolver latency   5s in logs', which changes the meaning. Mermaid supports entity codes (#gt; #lt; #91; #93;).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Special characters are escaped with Mermaid entity codes instead of being dropped
- [ ] #2 Test with a title containing < > [ ] " &
<!-- AC:END -->
