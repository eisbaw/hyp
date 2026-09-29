---
id: HYPO-0011
title: Add a Justfile with the common development recipes
status: To Do
assignee: []
created_date: '2026-09-29 22:16'
updated_date: '2026-09-29 22:30'
labels:
  - tooling
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The repo has no Justfile. Build, lint, test and run commands are only listed in the README and flake. Recipes should run inside the flake dev shell (`nix develop`).

The global workflow expects `just e2e` before every commit. Today the end-to-end checks are the Rust integration tests (tests/workflow.rs, tests/api.rs) plus the optional `scripts/dom-test.cjs` (jsdom) and `scripts/browser-test.cjs` (Playwright). The browser suite has never actually been run: VALIDATION.md says Chromium failed in the authoring sandbox.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Justfile with commented recipes: build, test, lint (clippy -D warnings), fmt, fmt-check, check (nix flake check), run/web, demo
- [ ] #2 README Development section points to the recipes instead of duplicating commands
- [ ] #3 Justfile reviewed with the justfile-hygiene checklist (no one-off recipes)
- [ ] #4 `just e2e` runs the integration tests plus the DOM test (scripts/dom-test.cjs) against the real binary; jsdom comes from the flake (committed package.json/lockfile or a nix package), not an ad-hoc npm install
- [ ] #5 The DOM test is also a flake check, so local runs and CI share one definition
<!-- AC:END -->
