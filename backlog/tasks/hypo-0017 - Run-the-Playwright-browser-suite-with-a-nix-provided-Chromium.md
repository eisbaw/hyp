---
id: HYPO-0017
title: Run the Playwright browser suite with a nix-provided Chromium
status: To Do
assignee: []
created_date: '2026-09-29 22:30'
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
