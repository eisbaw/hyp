---
id: HYPO-0007
title: Replace string-prefix error classification with a typed conflict error
status: To Do
assignee: []
created_date: '2026-09-29 22:15'
labels:
  - refactor
  - web
  - cli
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found in the initial review.

The CLI exit code 3 (`main.rs`) and HTTP 409 (`web.rs::ApiError`) are chosen by `message.starts_with("conflict:")`. Wrapping an error in `.context()` silently changes its classification. An invalid session token returns 422 instead of 403.

Also in `web.rs`, the `/report.html` handler calls the blocking `store.snapshot()` (file lock plus full read) directly on the async runtime. Every other handler uses `spawn_blocking`.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Conflicts are a typed error (e.g. a `Conflict` struct downcast via anyhow) and exit code / HTTP status derive from the type
- [ ] #2 Invalid or missing token returns 403
- [ ] #3 `/report.html` uses spawn_blocking like the other handlers
- [ ] #4 Existing conflict tests still pass; a test asserts exit code 3 for a stale CLI write
<!-- AC:END -->
