---
id: HYPO-0053
title: >-
  hyp apply is hard for agents to write: required defaults and full-record
  updates
status: To Do
assignee: []
created_date: '2026-09-30 12:18'
labels:
  - agents
  - cli
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
From the CLI ergonomics QA (2026-09-30). Creating a hypothesis via hyp apply fails with serde's 'missing field lifecycle' (no default; the CLI sets draft). An apply update must repeat the full stored record, including created_at, or fails with 'cannot change creation time', so an agent must round-trip the whole record to change one field. HYPO-0027 already notes that the apply format is only documented in the uninstalled README.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Create inputs default lifecycle (draft), status (planned) and other fields the CLI defaults, so a minimal create works
- [ ] #2 A patch-style update op changes only the given fields (with expected_revision), or update keeps stored created_at when it is omitted
- [ ] #3 hyp apply --help shows a minimal create and update example
<!-- AC:END -->
