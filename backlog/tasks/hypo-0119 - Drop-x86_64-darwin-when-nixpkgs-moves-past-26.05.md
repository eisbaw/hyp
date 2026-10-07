---
id: HYPO-0119
title: Drop x86_64-darwin when nixpkgs moves past 26.05
status: To Do
assignee: []
created_date: '2026-10-06 13:28'
labels:
  - portability
  - ci
dependencies: []
references:
  - flake.nix
  - .github/workflows/ci.yml
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Nixpkgs 26.05 is the last release supporting x86_64-darwin (evaluation warns), and GitHub's macos-15-intel is its last Intel runner image. When flake.nix moves to a newer nixpkgs, x86_64-darwin and the macos-15-intel CI job must go. Follows HYPO-0118.01.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 flake.nix systems and the CI matrix no longer list x86_64-darwin / macos-15-intel once nixpkgs is newer than 26.05
- [ ] #2 docs/usage.md lists the supported systems correctly
<!-- AC:END -->
