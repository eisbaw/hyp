---
id: HYPO-0010
title: Document or close the local multi-user exposure of `hyp web`
status: To Do
assignee: []
created_date: '2026-09-29 22:16'
labels:
  - security
  - docs
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found in the initial review. The server binds to 127.0.0.1 and guards Host/Origin, which stops browser DNS-rebinding and CSRF attacks. The mutation token, however, is served unauthenticated at `GET /api/session`. Any other local user or process on the machine can `curl -H 'Host: 127.0.0.1:7432'` to read the whole notebook and write to it. That is acceptable for a single-user laptop, but the README does not state it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 README states the trust boundary explicitly
- [ ] #2 Optionally: print a one-time token in the startup URL (as Jupyter does) instead of serving it at /api/session, or support a Unix socket
<!-- AC:END -->
