---
id: HYPO-0002
title: >-
  Scope write preconditions to what each change depends on (spurious
  whole-project conflicts)
status: To Do
assignee: []
created_date: '2026-09-29 22:15'
updated_date: '2026-09-29 23:15'
labels:
  - bug
  - webui
  - concurrency
  - cli
  - mvp
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found in the initial review; the fix was corrected by the architect review.

Symptom (reproduced): the WebUI sends `expected_revision: draftRevision`, the whole-project revision from when the editor opened. Any unrelated write (an agent adding another hypothesis via the CLI) makes the save fail with 409 `conflict: project changed`. Most CLI commands also commit with `Some(&s.revision)` in `cli.rs`, so agents get a spurious exit 3 when an unrelated write lands between their read and their commit. Archive/restore/delete in the WebUI (`transact()` default) have the same problem.

Root cause: the precondition is too coarse. Dropping it would be wrong: `Store::commit` computes an assessment's `based_on` fingerprint on the server at commit time, and freezes experiment targets and run plans at commit time too. Without a precondition, a human writing an assessment while an agent adds counter-evidence would record an assessment "based on" evidence nobody saw, with `needs_review` false. That is the exact failure hyp exists to prevent.

Fix: each change carries the precondition it actually depends on:
- update/archive/delete: the object's revision (already present);
- assessment create: the hypothesis fingerprint the author saw; the server rejects if it differs;
- experiment create: the target revisions the author saw (the client already sends `FrozenRef.revision`; the server currently ignores it and re-freezes);
- run create: the experiment revision the author saw.
The whole-project `expected_revision` stays available (`hyp apply --expected-revision`) but is optional and not used for ordinary edits.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 WebUI saves and CLI commands succeed when only unrelated records changed since they read the project
- [ ] #2 An assessment is rejected as a conflict when the hypothesis fingerprint changed after the author read it (e.g. new counter-evidence), and the WebUI draft is preserved
- [ ] #3 Experiment/run creation is rejected when the frozen targets/plan changed after the author read them
- [ ] #4 Updates, archives and deletes of a record that changed are still rejected as conflicts
- [ ] #5 Tests cover each case above, including an unrelated concurrent CLI write during a WebUI save
- [ ] #6 The WebUI decides 'conflict' from the HTTP status (409), not from the message prefix "conflict:" (web/app.js save() currently uses e.message.startsWith)
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Forward-carried from HYPO-0007: conflicts are now a type, store::Conflict(String). Raise new preconditions (hypothesis fingerprint, frozen target and plan revisions) with bail!(Conflict("...".into())) and do not write the 'conflict: ' prefix in the message (Display adds it). CLI exit 3 (cli::exit_code) and HTTP 409 (web::ApiError) then follow automatically, even through .context(). web/app.js still checks e.message.startsWith('conflict:') to show the preserve-your-draft hint, so do not wrap conflicts in context on the API path. Test templates: tests/cli.rs stale_cli_write_exits_with_code_3_and_a_conflict_message (real binary, 'apply --expected-revision'), tests/api.rs api_and_cli_store_share_state_and_reject_stale_writes (409 plus 'conflict:' body).
<!-- SECTION:NOTES:END -->
