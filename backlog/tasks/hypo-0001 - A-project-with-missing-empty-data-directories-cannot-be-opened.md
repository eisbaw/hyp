---
id: HYPO-0001
title: A project with missing empty data directories cannot be opened
status: Done
assignee:
  - '@claude'
created_date: '2026-09-29 22:15'
updated_date: '2026-09-29 23:15'
labels:
  - bug
  - storage
  - mvp
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found in the initial review. Reproduced with the debug build.

`hyp init` creates `hyp/{hypotheses,predictions,criteria,evidence,links,experiments,runs,assessments,gaps,assets}`. Many ways of moving a project around do not preserve empty directories: `git clone` (Git does not track them), some sync tools, and archive or copy tools with file filters. When a data directory is missing, `Store::open` -> `ensure_layout` -> `safe_dir` fails with a bare `hyp: No such file or directory (os error 2)` and no path. Even `hyp init --demo` is affected, because `assets/` is empty.

Repro (with git): `hyp init --demo; git init; git add hyp; git commit; git clone . ../c; hyp --project ../c list`.
Repro (without git): copy only the files of `hyp/` to a new location and run `hyp list` there.

Per the decision "hyp does not require or depend on Git, but is Git-friendly", a plain directory is the primary case and a Git checkout is one instance of it.

Related Git-friendliness issue: `.hyp/.gitignore` is written only by `init`. When `.hyp/` is recreated on open, `.hyp/write.lock` is not ignored and shows as untracked in `git status`. The file is harmless when Git is not used.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Opening a project whose record/asset directories are missing (e.g. a fresh clone) works; missing directories are created (still rejecting symlinks) or treated as empty
- [x] #2 I/O errors from open/layout checks name the offending path
- [x] #3 `.hyp/.gitignore` is ensured whenever `.hyp/` is created (harmless without Git); in a Git working tree, `git status` stays clean after running hyp
- [x] #4 Integration test without Git: copy only the files of a demo project (dropping empty directories), then run list/check/add in the copy
- [x] #5 Integration test with Git (skipped if git is not installed): commit, clone, run list/check/add in the clone
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Reproduce (copy demo files without empty dirs). 2. ensure_layout creates missing data dirs (create_dir, not create_dir_all, so a missing hyp/ is still an error), still rejecting symlinks/non-dirs; ensure .hyp/.gitignore on every open/lock. 3. Add path context to all I/O in open/ensure_layout/safe_dir/lock. 4. Tests: plain copy of demo files -> list/check/add via real binary; optional git clone test skipped without git; git status clean.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented (uncommitted, awaiting review):
- store.rs: new ensure_dir() = create_dir (not create_dir_all) tolerating AlreadyExists, then safe_dir(), so symlinks and files in place of a directory are still rejected and a missing hyp/ itself is still an error (not recreated). ensure_layout() uses it for all data dirs, assets and .hyp/, and writes .hyp/.gitignore ('*') when it is missing; an existing one is left alone. init() now calls ensure_layout() instead of duplicating the directory list.
- Path context on I/O in init/open/safe_dir/ensure_dir/lock/read_unlocked(read_dir)/recover(journal read); config.toml parse errors name the file.
- Tests: tests/cli.rs project_copied_without_empty_directories_opens_and_accepts_writes (copies only hyp/ files, runs real binary list/check/add, asserts dirs and .hyp/.gitignore recreated); git_clone_of_project_opens_and_status_stays_clean (skips without git; isolates git via GIT_CONFIG_GLOBAL=/dev/null etc.; only hyp/ committed; porcelain empty after list/check, only the new record after add); workflow.rs missing_data_directories_are_recreated_but_non_directories_rejected, io_errors_name_the_offending_path.
- Red/green: all four failed before the fix ('No such file or directory (os error 2)'); the .gitignore part was confirmed red separately by disabling only the gitignore write (git test showed '?? .hyp/').
Gotchas:
- A Git test that commits with 'git add .' passes vacuously if init also stops writing the .gitignore (write.lock gets committed). The test commits only hyp/, matching the real repro.
- The web monitor re-creates a deleted data directory within about 2 s, so tests that remove and recreate a directory under a running server race with it.
- ensure_layout runs on every snapshot (reads can write), so a missing directory is recreated even by 'hyp list'; on a read-only filesystem that was already required for .hyp/write.lock.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Implemented in the cycle-2 batch (HYPO-0001/0007/0018). Gate: fmt-check, lint, e2e (31 Rust tests: api 4, cli 5, workflow 22; plus DOM test), nix flake check; QA GO (5 test runs, sequential and parallel, no flakes; manual scenarios verified) and architect GO. Follow-ups: HYPO-0020 (widened: stdout write errors other than EPIPE), HYPO-0021 (panic in a blocking task reported as 422), HYPO-0022 (paper cuts).
<!-- SECTION:FINAL_SUMMARY:END -->
