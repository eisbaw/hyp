---
id: HYPO-0018
title: 'Fail loudly: broken-pipe panic and silently swallowed web errors'
status: Done
assignee:
  - '@claude'
created_date: '2026-09-29 22:30'
updated_date: '2026-09-29 23:15'
labels:
  - bug
  - cli
  - web
  - mvp
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
- [x] #1 Writing to a closed stdout exits quietly (conventional SIGPIPE behaviour or exit 0/141), with no panic; test with `hyp list | head -c0`
- [x] #2 Snapshot and watcher errors in `hyp web` are logged to stderr with the cause
- [x] #3 The WebUI shows the error message behind 'Unavailable'
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Reproduce hyp list | head -c0 panic. 2. Restore SIGPIPE default at start of main on unix (libc). 3. hyp web: log monitor snapshot errors (on change of cause, plus recovery) and notify watcher errors to stderr with cause. 4. UI: confirm /api/snapshot error text reaches the notice behind 'Unavailable'; add dom-test coverage. 5. Tests: real binary with early-closed stdout pipe; web binary stderr contains cause.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented (uncommitted, awaiting review):
- SIGPIPE: main.rs restores SIG_DFL (libc, unix-only dependency; Cargo.lock only gains libc in hyp's dependency list, the crate was already locked) for every command except 'hyp web'. Chosen over handling BrokenPipe at each println! site (about 20 sites, easy to regress). Shell status is 141, like other filters.
- Why web is exempt: std's TcpStream::write uses send(MSG_NOSIGNAL), but write_vectored is plain writev(2), and strace showed hyper writes responses with writev. With SIGPIPE at default, an EPIPE there would kill the server. I could not trigger EPIPE in practice (clients that reset or close gave ECONNRESET, or hyper saw EOF first), so the exemption is defensive.
- Web logging: the monitor logs 'hyp web: cannot read project: <cause>' once per distinct cause and 'hyp web: project readable again (was: ...)' on recovery; notify watcher errors are logged with the cause. The UI already fetched /api/snapshot after the 'unavailable' SSE event and showed its error body in #notice next to 'Unavailable · retrying'; what was missing was a useful cause (bare 'No such file or directory'). HYPO-0001's path context fixes that.
- Tests: tests/cli.rs closed_stdout_ends_quietly_without_panic (stdout is a UnixStream whose peer is closed, so EPIPE is deterministic; asserts no panic, not 101, signal 13; red before: panic, 101); web_logs_why_the_project_is_unavailable_and_when_it_recovers (real 'hyp web --port 0', replaces hyp/gaps with a file, waits for the logged cause naming hyp/gaps, removes the file and waits for the recovery line; red before: no stderr at all); scripts/dom-test.cjs renames hyp/ away and waits for 'Unavailable' plus a notice naming <root>/hyp, then renames it back and waits for 'Live' (red against the original source: timed out, the notice had no path).
- Not tested: the watcher error branch (no reliable way to make notify fail in a test); the web SIGPIPE exemption (see above).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Implemented in the cycle-2 batch (HYPO-0001/0007/0018). Gate: fmt-check, lint, e2e (31 Rust tests: api 4, cli 5, workflow 22; plus DOM test), nix flake check; QA GO (5 test runs, sequential and parallel, no flakes; manual scenarios verified) and architect GO. Follow-ups: HYPO-0020 (widened: stdout write errors other than EPIPE), HYPO-0021 (panic in a blocking task reported as 422), HYPO-0022 (paper cuts).
<!-- SECTION:FINAL_SUMMARY:END -->
