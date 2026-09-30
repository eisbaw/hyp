---
id: HYPO-0049
title: WebUI assessment form offers only evidence linked to the hypothesis
status: To Do
assignee: []
created_date: '2026-09-30 11:31'
labels:
  - webui
  - agents
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Since decision-0003 (linked-only citations) the assessment form still lists every evidence record, so most choices are rejected with a 422 naming 'hyp link'. The form should offer what can be cited. Avoid a JS copy of the link rule (Snapshot::is_linked) that can drift: prefer the server exposing the citable evidence per hypothesis (e.g. in the derived state).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The assessment form lists only evidence linked to the selected hypothesis or its criteria or predictions, and updates when the hypothesis changes
- [ ] #2 The rule has one implementation (server side); the WebUI reads its result
- [ ] #3 DOM test covers it
<!-- AC:END -->
