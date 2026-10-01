---
id: HYPO-0107
title: Regenerate VALIDATION.md for 0.4.0
status: To Do
assignee: []
created_date: '2026-10-01 18:58'
labels:
  - docs
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
VALIDATION.md is scoped to 9a62a67 (built as 0.3.0; HYPO-0089) and still says the browser suite never completed. Since then 0.4.0 was released (a38c06c), the Playwright suite passes with nix-provided Chromium (just browser-test and the e2e-browser flake check, HYPO-0017), property tests exist (tests/properties, just fuzz, HYPO-0013), and GitHub CI runs nix flake check on x86_64-linux (HYPO-0012). The README link no longer claims VALIDATION.md covers this release. HYPO-0017 AC #3 depends on this task.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 VALIDATION.md names 0.4.0 and the commit it validates, and lists the checks run on that commit with their results
- [ ] #2 It reports the browser suite result (just browser-test and the e2e-browser flake check) instead of saying it never completed
- [ ] #3 It covers the property tests (fixed seed, capped case counts, just fuzz) and GitHub CI (what runs, x86_64-linux only, aarch64-linux unverified)
- [ ] #4 The README link to VALIDATION.md names the version it covers
<!-- AC:END -->
