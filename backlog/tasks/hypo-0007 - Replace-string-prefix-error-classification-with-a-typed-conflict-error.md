---
id: HYPO-0007
title: Replace string-prefix error classification with a typed conflict error
status: Done
assignee:
  - '@claude'
created_date: '2026-09-29 22:15'
updated_date: '2026-09-29 23:15'
labels:
  - refactor
  - web
  - cli
  - mvp
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
- [x] #1 Conflicts are a typed error (e.g. a `Conflict` struct downcast via anyhow) and exit code / HTTP status derive from the type
- [x] #2 Invalid or missing token returns 403
- [x] #3 `/report.html` uses spawn_blocking like the other handlers
- [x] #4 Existing conflict tests still pass; a test asserts exit code 3 for a stale CLI write
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. pub struct Conflict(String) in store.rs (Display prints 'conflict: ...'), raised via bail!(Conflict(..)) at every conflict site. 2. main.rs exit 3 when the error chain contains Conflict (downcast, survives .context()). 3. ApiError gets an explicit status; Conflict in chain -> 409; bad/missing token -> 403. 4. /report.html via spawn_blocking. 5. Tests: exit 3 for stale CLI write (real binary), 409 stale API write, 403 bad/missing token, context-wrapped conflict still classified.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented (uncommitted, awaiting review):
- store.rs: pub struct Conflict(pub String); Display prints 'conflict: {msg}' so user-visible text is unchanged; Conflict::in_chain(&anyhow::Error) checks every cause, so .context() wrapping keeps the classification. All five former 'conflict:' ensure! sites now bail!(Conflict(..)).
- cli.rs: pub fn exit_code(&anyhow::Error) -> i32 (3 for Conflict, else 1); main.rs uses it. Moved into the library so it is testable.
- web.rs: ApiError { status, error }; From<anyhow::Error> picks 409 for Conflict else 422; invalid/missing token -> 403 explicitly. /report.html runs snapshot + export_html in spawn_blocking.
- Tests: workflow.rs conflicts_are_classified_by_type_even_when_wrapped_in_context (red with the old string-prefix logic: wrapped conflict gave 1, and plain text 'conflict: ...' must give 1); tests/cli.rs stale_cli_write_exits_with_code_3_and_a_conflict_message (real binary, plain and --json; passes before and after, it is the regression guard the AC asks for); api.rs: missing and wrong token -> 403 (red: was 422), stale API write -> 409 with 'conflict:' body, report_does_not_block_the_runtime_while_the_store_is_locked (holds .hyp/write.lock from the test; on the current-thread test runtime a blocking handler makes the request finish instead of timing out; red before, green after).
Gotcha: ensure!(cond, Conflict(..)) would also work (anyhow keeps the type for a single error argument) but if/bail! reads more plainly.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Implemented in the cycle-2 batch (HYPO-0001/0007/0018). Gate: fmt-check, lint, e2e (31 Rust tests: api 4, cli 5, workflow 22; plus DOM test), nix flake check; QA GO (5 test runs, sequential and parallel, no flakes; manual scenarios verified) and architect GO. Follow-ups: HYPO-0020 (widened: stdout write errors other than EPIPE), HYPO-0021 (panic in a blocking task reported as 422), HYPO-0022 (paper cuts).
<!-- SECTION:FINAL_SUMMARY:END -->
