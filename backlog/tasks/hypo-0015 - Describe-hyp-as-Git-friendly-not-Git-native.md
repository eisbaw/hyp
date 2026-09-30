---
id: HYPO-0015
title: 'Describe hyp as Git-friendly, not Git-native'
status: Done
assignee:
  - '@claude'
created_date: '2026-09-29 22:21'
updated_date: '2026-09-30 02:19'
labels:
  - docs
  - design
  - mvp
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
- [x] #1 README has a short 'Using hyp with Git (optional)' section listing what makes it Git-friendly (one file per record, deterministic serialization, UUIDs that do not collide across branches, no generated index files, `.hyp/` ignored) and how to handle merges; history advice is phrased as 'use any version control, e.g. Git'
- [x] #2 No code path shells out to git or requires a `.git` directory (checked by grep in review)
- [x] #3 Tagline and descriptions lead with the agent-first purpose (decision-0002), not 'for humans and agents'
- [x] #4 "Git-native" and Git-presuming wording is replaced everywhere (README incl. 'Divergent heads after a Git merge' and 'Git is still needed…', VALIDATION.md, Cargo.toml, flake.nix, clap about) with wording that says hyp works on a plain directory and is Git-friendly
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Replace 'Git-native' in README, VALIDATION.md, Cargo.toml, flake.nix and the clap about with agent-first, plain-directory, Git-friendly wording.
2. Rephrase Git-presuming README text (merge, history).
3. Add a 'Using hyp with Git (optional)' README section.
4. Grep for git invocations / .git requirements in src/.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implementation (2026-09-30, uncommitted):
- New wording (Cargo.toml description, flake meta.description, clap about): 'Hypothesis tracking for coding and research agents: plain files in any directory, Git-friendly'. flake description: 'hyp: hypothesis tracking for coding and research agents, with a CLI and live WebUI; plain files, Git-friendly'.
- README: agent-first intro; 'Using hyp with Git (optional)' section (one file per record, deterministic serialization and writes touch only changed records, UUIDs, no generated index files, .hyp/ ignores itself, hyp check after merge/checkout/sync, divergent heads reconciled by a new assessment); 'Divergent heads after any merge or sync'; 'To keep a history of all file edits, use any version control, e.g. Git'; lock caveat now names editors, sync tools and version control. Also fixed agent-first contradictions: 'The human judges...' and 'No AI-generated judgments' reworded (agents may record judgments per decision-0002).
- VALIDATION.md: Git-presuming limits reworded; replaced the stale test count with a description (counts rot).
- AC #2 grep: `grep -rn 'process::Command\|Command::new\|"git"\|\.git\b' src/ web/` finds only `std::process::Command::new("sh")` in `hyp edit` (runs $VISUAL/$EDITOR). No git invocation, no .git lookup; the only Git-specific artefact is .hyp/.gitignore. The agent tests run in plain temp dirs and assert no .git exists.
- Remaining 'Git' mentions are deliberate: the Git section, 'e.g. Git', store.rs comments about clones, the dev shell's git package and the Justfile's 'Git-tracked tree' (flake check).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Descriptions (Cargo.toml, flake, clap about) now read 'Hypothesis tracking for coding and research agents: plain files in any directory, Git-friendly'. README has an agent-first intro and a 'Using hyp with Git (optional)' section; Git-presuming sentences now say 'any merge or sync'. A grep of src/ and web/ shows no git invocation (the only spawned process is sh for $EDITOR). Reviewed with HYPO-0016: QA GO, architect GO.
<!-- SECTION:FINAL_SUMMARY:END -->
