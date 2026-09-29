---
id: decision-0001
title: 'hyp does not require or depend on Git, but is Git-friendly'
date: '2026-09-29 22:21'
status: accepted
---
## Context

hyp stores all state as plain files under `hyp/`. People keep such files in Git, another VCS, a sync tool (Syncthing, Dropbox), archives, or nothing at all. backlog.md is the model: it works in any directory and is pleasant to use with Git, but does not rely on it. The initial code already never shells out to git, but its wording ("Git-native") and some of the first review's tasks presumed Git.

## Decision

hyp does not require, presume or depend on Git. A plain directory is the primary case; a Git working tree is one instance of it. hyp never runs `git` and never needs a `.git` directory. hyp is deliberately Git-friendly.

## Consequences

- Storage must survive what copies, syncs and clones do: missing empty directories, CRLF conversion, files arriving from a merge or sync. Handle these on open or report them clearly (HYPO-0001, HYPO-0003, HYPO-0005).
- Git-friendliness means: one file per record, deterministic serialization, IDs that do not collide across branches, no generated index files in `hyp/`, and a `.hyp/.gitignore`, which is harmless without Git.
- Divergent assessment heads can come from any merge or sync, not only Git merges; reconciliation is explicit.
- Keeping history is the user's job, with whatever version control they use; hyp does not claim to be an audit log.
- Wording says "Git-friendly", never "Git-native" (HYPO-0015). Tests treat a plain directory as the main case; Git-specific tests are optional and skipped when git is absent.
