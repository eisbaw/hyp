---
id: HYPO-0120
title: 'VALIDATION.md: the browser test now runs'
status: To Do
assignee: []
created_date: '2026-10-07 00:03'
labels:
  - docs
dependencies: []
references:
  - VALIDATION.md
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
VALIDATION.md (the per-release validation record) still says scripts/browser-test.cjs 'has never completed' and makes no real-browser claim. Since HYPO-0012 it passes as the e2e-browser flake check on Linux CI, and since HYPO-0118.01 as nix run .#browser-test on macOS CI. Refresh it at the next release.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 VALIDATION.md states which browser checks ran for the version it validates, and where
<!-- AC:END -->
