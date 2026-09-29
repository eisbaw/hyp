---
id: HYPO-0012
title: Add GitHub Actions CI driven by the nix flake
status: To Do
assignee: []
created_date: '2026-09-29 22:16'
updated_date: '2026-09-29 22:30'
labels:
  - tooling
  - ci
dependencies:
  - HYPO-0011
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
CI should run the same checks as a developer, through the flake, so CI and local results cannot drift. `nix flake check` already builds the package and runs the tests, clippy and rustfmt. The e2e (DOM/browser) tests are not yet flake checks.

Do not push to GitHub or enable the workflow on a remote without explicit approval.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `.github/workflows/ci.yml` installs Nix (with a binary cache such as magic-nix-cache or cachix) and runs `nix flake check -L` on x86_64-linux
- [ ] #2 aarch64-linux is either built in CI (native ARM runner) or README/VALIDATION says it is untested
- [ ] #3 Workflow validated locally (e.g. `act` or a dry run of the same commands) before asking to push
- [ ] #4 All CI logic lives in the flake (`nix flake check` including the DOM e2e check); the GitHub workflow is a thin wrapper, so switching CI hosts means only a new wrapper
- [ ] #5 The repository currently has no remote: do not add one or push without explicit approval
<!-- AC:END -->
