---
id: HYPO-0011
title: Add a Justfile with the common development recipes
status: Done
assignee:
  - '@claude'
created_date: '2026-09-29 22:16'
updated_date: '2026-09-29 22:48'
labels:
  - tooling
  - mvp
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
- [x] #1 Justfile with commented recipes: build, test, lint (clippy -D warnings), fmt, fmt-check, check (nix flake check), run/web, demo
- [x] #2 README Development section points to the recipes instead of duplicating commands
- [x] #3 Justfile reviewed with the justfile-hygiene checklist (no one-off recipes)
- [x] #4 `just e2e` runs the integration tests plus the DOM test (scripts/dom-test.cjs) against the real binary; jsdom comes from the flake (committed package.json/lockfile or a nix package), not an ad-hoc npm install
- [x] #5 The DOM test is also a flake check, so local runs and CI share one definition
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. jsdom: not packaged in pinned nixpkgs (no top-level jsdom; nodePackages removed). Commit scripts/package.json + scripts/package-lock.json (jsdom only) and build node_modules with pkgs.importNpmLock.buildNodeModules (no npmDepsHash to maintain; lockfile is the single source of truth).
2. flake.nix: share one domTestDeps derivation; dev shell gets just + NODE_PATH to it; add checks.<system>.e2e-dom running scripts/dom-test.cjs with HYP_BIN = the packaged binary.
3. Justfile (run from inside nix develop; just comes from the dev shell): build, test, lint, fmt, fmt-check, check, web, demo, e2e. e2e = cargo integration tests + dom-test.cjs against target/debug/hyp, same script and same node_modules as the flake check.
4. README Development section and CLAUDE.md header point at recipes; VALIDATION.md mention if needed.
5. Verify: nix develop -c just fmt-check/lint/test/e2e, git add, nix flake check -L.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented (uncommitted, staged for review):
- jsdom is not packaged in the pinned nixpkgs (no top-level jsdom; nodePackages was removed). So scripts/package.json + scripts/package-lock.json (jsdom 30.1.1, lockfileVersion 3) are committed and flake.nix builds node_modules with pkgs.importNpmLock.buildNodeModules (let-bound jsTestDeps). importNpmLock reads resolved+integrity per package from the lockfile, so there is no npmDepsHash to keep in sync; the lockfile is the single source of truth.
- Dev shell gains just and NODE_PATH=<jsTestDeps>/node_modules. New checks.<system>.e2e-dom runs the same scripts/dom-test.cjs with HYP_BIN = the packaged binary; it passes inside the nix sandbox (loopback networking is available there).
- Justfile is meant to be run from inside nix develop (just itself comes from the dev shell; it is not on the host PATH). Recipes: build, web, demo, test, e2e, fmt, fmt-check, lint, check. e2e = cargo test + cargo build + dom-test.cjs against target/debug/hyp. Same script and deps as the flake check; only the binary differs (debug vs packaged).
Gotchas:
- To bump jsdom: edit scripts/package.json, then in a scratch dir inside nix develop run npm install --package-lock-only --ignore-scripts and copy the lockfile back. Do not npm install into scripts/: a scripts/node_modules would shadow NODE_PATH.
- jsdom 30 requires node ^22.22.2 || ^24.15.0; pinned nixpkgs nodejs is 24.21. A nixpkgs bump that lowers or changes nodejs could break this.
- NODE_PATH works for CommonJS require only, not ESM import.
- nix develop sets TMPDIR to /tmp/nix-shell.XXXX and removes it on exit; the demo recipe's mktemp dir lives there and is also removed by its own trap.
- nix flake check (and just check) sees only Git-tracked files: git add new files first.
- Just runs recipes from the Justfile directory, so relative --project paths passed to just web resolve from the repo root.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Justfile with grouped, commented recipes (build, web, demo, test, e2e, fmt, fmt-check, lint, check), run inside the flake dev shell. jsdom is pinned in scripts/package-lock.json and built with importNpmLock (lockfile is the single source; no npmDepsHash). The DOM test runs in `just e2e` (against target/debug/hyp) and as the `e2e-dom` flake check (against the packaged binary). Reviewed: QA GO (22 Rust tests; DOM test 7/7 passes; failures propagate, exit 1), architect GO. Follow-up: HYPO-0019.
<!-- SECTION:FINAL_SUMMARY:END -->
