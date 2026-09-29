---
id: HYPO-0018
title: 'Fail loudly: broken-pipe panic and silently swallowed web errors'
status: To Do
assignee: []
created_date: '2026-09-29 22:30'
labels:
  - bug
  - cli
  - web
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found by the architect review; the panic was reproduced.

- `hyp list | head -c0` panics: `failed printing to stdout: Broken pipe (os error 32)`, exit 101. Agents and shell pipelines routinely close pipes early (`| head`).
- The web monitor turns any snapshot error into the SSE event `"unavailable"` without logging the cause, and `notify` watcher errors are discarded. The server logs nothing, so a user sees "Unavailable" with no way to find out why.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Writing to a closed stdout exits quietly (conventional SIGPIPE behaviour or exit 0/141), with no panic; test with `hyp list | head -c0`
- [ ] #2 Snapshot and watcher errors in `hyp web` are logged to stderr with the cause
- [ ] #3 The WebUI shows the error message behind 'Unavailable'
<!-- AC:END -->
