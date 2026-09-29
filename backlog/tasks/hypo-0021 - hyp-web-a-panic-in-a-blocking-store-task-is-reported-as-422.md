---
id: HYPO-0021
title: 'hyp web: a panic in a blocking store task is reported as 422'
status: To Do
assignee: []
created_date: '2026-09-29 23:06'
labels:
  - bug
  - web
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while implementing HYPO-0007. Handlers run store work with tokio::task::spawn_blocking and map the JoinError into anyhow, so a panic inside the store (a bug) reaches the client as 422 Unprocessable Entity, the status for invalid input. It should be 500 so clients and agents do not treat a server bug as their own mistake. ApiError in src/web.rs now carries an explicit status, so this is a small change.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A JoinError (panic or cancellation) in a handler's blocking task is answered with 500 and an error message
- [ ] #2 Test covers the mapping
<!-- AC:END -->
