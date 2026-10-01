---
id: HYPO-0012
title: Add GitHub Actions CI driven by the nix flake
status: Done
assignee: []
created_date: '2026-09-29 22:16'
updated_date: '2026-10-01 20:00'
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
- [x] #1 `.github/workflows/ci.yml` installs Nix (with a binary cache such as magic-nix-cache or cachix) and runs `nix flake check -L` on x86_64-linux
- [x] #2 aarch64-linux is either built in CI (native ARM runner) or README/VALIDATION says it is untested
- [x] #3 Workflow validated locally (e.g. `act` or a dry run of the same commands) before asking to push
- [x] #4 All CI logic lives in the flake (`nix flake check` including the DOM e2e check); the GitHub workflow is a thin wrapper, so switching CI hosts means only a new wrapper
- [x] #5 The repository currently has no remote: do not add one or push without explicit approval
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Forward-carried from HYPO-0011: nix flake check -L now includes checks.<system>.e2e-dom (jsdom UI test against the packaged binary), so CI running nix flake check covers the same DOM test as just e2e. npm tarballs are fetched as fixed-output derivations from the committed scripts/package-lock.json (importNpmLock); CI needs registry.npmjs.org reachable or a cache that has them.

From the HYPO-0004 QA: tests/reads.rs contains wall-clock-bound concurrency tests (starvation < 5 s debug, atomicity races); 5 clean runs locally, but they may flake on slow or busy CI runners. Consider a CI-specific timeout multiplier env var or running them in a separate, retried job.

Closing review 2026-10-01 (1292965; remote eisbaw/hyp, private, created and pushed with the user's explicit approval):
- AC #1 met: .github/workflows/ci.yml installs Nix (DeterminateSystems/nix-installer-action), uses magic-nix-cache (FlakeHub disabled, no id-token) and runs nix flake check -L on ubuntu-latest (x86_64-linux). GitHub runs 36863402027 (1292965) and 36869136610 (e4e4ecc) succeeded.
- AC #2 met: aarch64-linux is not built in CI; VALIDATION.md says no ARM build is recorded and ci.yml says it is not built. The README still says the flake "supports" aarch64-linux without saying it is untested.
- AC #4 met: the workflow is a thin wrapper around nix flake check, which includes the package tests, clippy, rustfmt, e2e-dom and (since 43b1130) e2e-browser.
- AC #5 met: the remote was added and pushed only after explicit approval.
- AC #3 NOT met as written, so the task stays In Progress: the pre-push gate ran actionlint and just fmt-check, lint and e2e, but its verification note says "skipped: local nix flake check (CI runs it)", and act was not run; the workflow's one command was first run on GitHub. Mitigation: nix flake check -L had passed locally on the same flake (unchanged since 9a62a67) in the stream D gate (a6d5c1c), and the first CI run was green, so the risk the AC guarded against did not materialise. Check AC #3 if that is accepted; otherwise run nix flake check -L locally (or act) and record it.

- Closed after GitHub CI run 36916759499 passed on master 418dca7 (0.4.0), the first run including the e2e-browser check. AC #3 accepted with the caveat that the workflow was validated by actionlint plus the same nix flake check run locally, not by act.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
GitHub Actions CI runs nix flake check -L on x86_64-linux for every master push and pull request; all checks live in the flake.

Changes (1292965):
- .github/workflows/ci.yml: checkout, Determinate Nix installer, magic-nix-cache (optional speed-up), nix flake check -L. permissions contents: read, no id-token, FlakeHub off, diagnostics telemetry off. Superseded runs are cancelled for pull requests only, so every master commit gets a result. 60-minute timeout.
- Remote: private GitHub repository eisbaw/hyp, created and pushed with the user's explicit approval.

Tests: actionlint; local just fmt-check, lint, e2e; GitHub runs 36863402027, 36869136610 and 36916759499 succeeded. The last, on 0.4.0 (418dca7), is the first with the e2e-browser check (package tests, clippy, rustfmt, e2e-dom, e2e-browser).

AC #3 accepted with a caveat: the workflow was validated by actionlint and the same nix flake check run locally, not by act. Unverified: aarch64-linux. Follow-ups: timing-bound tests may flake on shared runners (HYPO-0105); the package rebuilds on any repository edit (HYPO-0106).
<!-- SECTION:FINAL_SUMMARY:END -->
