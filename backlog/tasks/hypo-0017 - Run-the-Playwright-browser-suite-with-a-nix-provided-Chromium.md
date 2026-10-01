---
id: HYPO-0017
title: Run the Playwright browser suite with a nix-provided Chromium
status: In Progress
assignee:
  - '@implementer-c'
created_date: '2026-09-29 22:30'
updated_date: '2026-10-01 19:02'
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
- [x] #1 `just browser-test` runs scripts/browser-test.cjs headless with nix-provided Chromium and passes
- [x] #2 Playwright version pinned to match nixpkgs playwright-driver; documented in the Justfile/flake
- [ ] #3 VALIDATION.md updated with the actual result
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Pin npm playwright to the nixpkgs playwright-driver version (1.59.1) in scripts/package.json; regenerate the lockfile in a scratch dir (npm install --package-lock-only --ignore-scripts).
2. flake.nix: PLAYWRIGHT_BROWSERS_PATH from playwright-driver.browsers overridden to the Chromium headless shell only, plus PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS, in the dev shell and a checks.e2e-browser derivation.
3. just browser-test recipe; run it repeatedly; fix the test where it is stale.
4. Report the VALIDATION.md text to the docs implementer (D owns that file).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Forward-carried from HYPO-0011: flake.nix has a let-bound jsTestDeps (importNpmLock.buildNodeModules over scripts/package.json + package-lock.json), exposed to the dev shell as NODE_PATH and used by checks.e2e-dom. Add playwright to scripts/package.json pinned to the nixpkgs playwright-driver version (1.59.1 at the current flake.lock), regenerate the lockfile in a scratch dir with npm install --package-lock-only --ignore-scripts, and set PLAYWRIGHT_BROWSERS_PATH=${pkgs.playwright-driver.browsers} (plus PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1). browser-test.cjs uses CommonJS require, so NODE_PATH resolution works. Add a browser-test Justfile recipe in the test group; consider a matching flake check only if Chromium runs in the nix sandbox.

- scripts/package.json pins playwright 1.59.1 = pkgs.playwright-driver at the current flake.lock; lockfile regenerated with npm install --package-lock-only --ignore-scripts in a scratch dir. Documented in flake.nix next to playwrightBrowsers: after a nixpkgs update, pin the new version and regenerate the lockfile, since each Playwright release looks for its own browser revision.
- flake.nix: playwrightBrowsers = playwright-driver.browsers.override with only the Chromium headless shell (what headless launch uses since Playwright 1.49; no full Chromium, Firefox or WebKit in the closure); playwrightEnv sets PLAYWRIGHT_BROWSERS_PATH and PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS for the dev shell and checks.e2e-browser.
- First real run failed at the assessment step: the test predates decision-0003 and saved an inconclusive assessment without evidence, which the server rightly rejects. Fixed the test: link evidence through the CLI, wait for the live update, cite it in the form. Then PASS on 4 consecutive runs of just browser-test (Chromium headless shell 1217).

- checks.e2e-browser runs the same test in the nix build sandbox and PASSES. The first sandbox run crashed Chromium (Skia FATAL in SkFontMgr_FontConfigInterface: no /etc/fonts in the sandbox); fixed by giving the check FONTCONFIG_FILE = makeFontsConf with dejavu_fonts. The browser-test recipe fails fast when PLAYWRIGHT_BROWSERS_PATH is unset (outside the dev shell). The test reads the evidence ID from --json output.
- VALIDATION.md is owned by the docs implementer: the result goes to them for AC 3.

Closing review 2026-10-01: AC #1 and #2 are met; AC #3 is NOT met, so the task stays In Progress. VALIDATION.md is still "as of 9a62a67" and says the browser suite has never completed, although just browser-test and the e2e-browser flake check now pass (the a38c06c release gate ran both). AC #3 depends on HYPO-0107 (Regenerate VALIDATION.md for 0.4.0).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The Playwright browser suite runs headless with Chromium from nixpkgs, through just browser-test and the e2e-browser flake check, and passes.

Changes (43b1130, 9dad9fc):
- scripts/package.json pins playwright to the nixpkgs playwright-driver version; flake.nix fails at evaluation (lib.assertMsg) unless they match, and says to re-pin and regenerate the lockfile after a nixpkgs update.
- playwrightBrowsers: playwright-driver.browsers with only the Chromium headless shell; PLAYWRIGHT_BROWSERS_PATH and PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS in the dev shell and checks.e2e-browser; fonts via makeFontsConf for the sandbox (Chromium crashed without /etc/fonts).
- just browser-test fails fast outside the dev shell; browser-test.cjs no longer reads CHROMIUM_PATH.
- The test predated decision-0003 (an assessment without evidence); it now links evidence through the CLI and cites it.

Tests: just browser-test passed on repeated runs; the e2e-browser flake check passes in the sandbox; both ran in the a38c06c release gate.

Open: AC #3, VALIDATION.md still says the suite never completed; waits on HYPO-0107.
<!-- SECTION:FINAL_SUMMARY:END -->
