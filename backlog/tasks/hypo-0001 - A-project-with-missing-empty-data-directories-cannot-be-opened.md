---
id: HYPO-0001
title: A project with missing empty data directories cannot be opened
status: To Do
assignee: []
created_date: '2026-09-29 22:15'
updated_date: '2026-09-29 22:27'
labels:
  - bug
  - storage
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
- [ ] #1 Opening a project whose record/asset directories are missing (e.g. a fresh clone) works; missing directories are created (still rejecting symlinks) or treated as empty
- [ ] #2 I/O errors from open/layout checks name the offending path
- [ ] #3 `.hyp/.gitignore` is ensured whenever `.hyp/` is created (harmless without Git); in a Git working tree, `git status` stays clean after running hyp
- [ ] #4 Integration test without Git: copy only the files of a demo project (dropping empty directories), then run list/check/add in the copy
- [ ] #5 Integration test with Git (skipped if git is not installed): commit, clone, run list/check/add in the clone
<!-- AC:END -->
