---
id: HYPO-0017
title: Run the Playwright browser suite with a nix-provided Chromium
status: To Do
assignee: []
created_date: '2026-09-29 22:30'
updated_date: '2026-09-29 22:40'
labels:
  - testing
  - webui
dependencies:
  - HYPO-0011
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from HYPO-0011. `scripts/browser-test.cjs` has never passed: VALIDATION.md says Chromium failed in the authoring sandbox. Pixel layout, responsive rendering and real-browser behaviour are unverified. Getting Playwright from nix requires the npm `playwright` version to match nixpkgs `playwright-driver` (PLAYWRIGHT_BROWSERS_PATH).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `just browser-test` runs scripts/browser-test.cjs headless with nix-provided Chromium and passes
- [ ] #2 Playwright version pinned to match nixpkgs playwright-driver; documented in the Justfile/flake
- [ ] #3 VALIDATION.md updated with the actual result
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Forward-carried from HYPO-0011: flake.nix has a let-bound jsTestDeps (importNpmLock.buildNodeModules over scripts/package.json + package-lock.json), exposed to the dev shell as NODE_PATH and used by checks.e2e-dom. Add playwright to scripts/package.json pinned to the nixpkgs playwright-driver version (1.59.1 at the current flake.lock), regenerate the lockfile in a scratch dir with npm install --package-lock-only --ignore-scripts, and set PLAYWRIGHT_BROWSERS_PATH=${pkgs.playwright-driver.browsers} (plus PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1). browser-test.cjs uses CommonJS require, so NODE_PATH resolution works. Add a browser-test Justfile recipe in the test group; consider a matching flake check only if Chromium runs in the nix sandbox.
<!-- SECTION:NOTES:END -->
