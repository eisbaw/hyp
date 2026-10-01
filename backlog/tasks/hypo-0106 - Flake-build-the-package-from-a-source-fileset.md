---
id: HYPO-0106
title: 'Flake: build the package from a source fileset'
status: To Do
assignee: []
created_date: '2026-10-01 18:58'
labels:
  - tooling
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The package source is pkgs.lib.cleanSource self, the whole repository. Any edit to backlog/, the Justfile, scripts, docs or flake.nix changes the source hash, so the package is rebuilt and every check that depends on it reruns (nix flake check locally and on CI), though none of those files goes into the build. The build needs Cargo.toml, Cargo.lock, src, tests, web, agents (SKILL.md, embedded) and README.md (include_str! in tests).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The package source is a lib.fileset of Cargo.toml, Cargo.lock, src, tests, web, agents and README.md
- [ ] #2 Editing a file under backlog/, docs/ or scripts/, or the Justfile, leaves the package derivation path unchanged (nix eval of its drvPath before and after)
- [ ] #3 nix flake check passes, including the package tests, e2e-dom and e2e-browser checks
<!-- AC:END -->
