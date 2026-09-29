---
id: HYPO-0015
title: 'Describe hyp as Git-friendly, not Git-native'
status: To Do
assignee: []
created_date: '2026-09-29 22:21'
updated_date: '2026-09-29 22:30'
labels:
  - docs
  - design
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Decision: hyp does not require or depend on Git, but is Git-friendly (like backlog.md).

The code already follows this: it never runs git or needs a repository, and the only Git-specific action is writing `.hyp/.gitignore`, which is harmless without Git. The wording does not follow it. "Git-native" appears in the README tagline, the Cargo.toml description, the flake description and meta, and the clap `about` text. The README also says "Git is still needed to retain all manual file-edit history" and gives merge advice as if Git were assumed.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 README has a short 'Using hyp with Git (optional)' section listing what makes it Git-friendly (one file per record, deterministic serialization, UUIDs that do not collide across branches, no generated index files, `.hyp/` ignored) and how to handle merges; history advice is phrased as 'use any version control, e.g. Git'
- [ ] #2 No code path shells out to git or requires a `.git` directory (checked by grep in review)
- [ ] #3 Tagline and descriptions lead with the agent-first purpose (decision-0002), not 'for humans and agents'
- [ ] #4 "Git-native" and Git-presuming wording is replaced everywhere (README incl. 'Divergent heads after a Git merge' and 'Git is still needed…', VALIDATION.md, Cargo.toml, flake.nix, clap about) with wording that says hyp works on a plain directory and is Git-friendly
<!-- AC:END -->
