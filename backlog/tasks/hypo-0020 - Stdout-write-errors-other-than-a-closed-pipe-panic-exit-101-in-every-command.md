---
id: HYPO-0020
title: Stdout write errors other than a closed pipe panic (exit 101) in every command
status: To Do
assignee: []
created_date: '2026-09-29 23:06'
updated_date: '2026-09-29 23:15'
labels:
  - bug
  - web
  - cli
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while implementing HYPO-0018. `hyp web` deliberately keeps SIGPIPE ignored (see restore_default_sigpipe in src/main.rs: hyper writes responses with writev(2), which does not suppress SIGPIPE). Its only stdout write is the startup banner in web::serve (println!). If stdout is already closed when the server starts (e.g. `hyp web > /dev/full`, or a supervisor that closed the pipe), println! panics and the server exits 101 instead of running or failing with a clear error. Low impact: `hyp web | head -1` works because the banner is written once.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The banner write in hyp web handles a closed or failing stdout without panicking (either ignore EPIPE for the banner or fail with a clear error)
- [ ] #2 Test runs the real binary with a closed stdout
- [ ] #3 Every command exits 1 with a message (no panic) when stdout returns a write error other than EPIPE; test with /dev/full
- [ ] #4 Web monitor and watcher logging cannot panic on a closed stderr
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Widened after the cycle-2 architect review: `hyp --json list > /dev/full` panics with 'No space left on device' and exit 101. Restoring SIGPIPE only covers closed pipes; other write errors still hit the println! panic, in every command, not only the web banner. Fix: write via writeln! on a locked stdout and propagate the error through run(), so the command exits 1 with a message. Also: eprintln! in the web monitor and watcher callback panics if stderr is closed, silently killing the monitor.
<!-- SECTION:NOTES:END -->
