---
id: HYPO-0019
title: Single-source the formatting check and the e2e binary path
status: To Do
assignee: []
created_date: '2026-09-29 22:47'
labels:
  - tooling
  - hardening
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
From the HYPO-0011 architect review (non-blocking). (1) The flake's formatting check runs `rustfmt --edition 2024 --check src/*.rs tests/*.rs`, a flat glob that silently misses nested modules (e.g. src/web/*.rs) and hardcodes the edition a second time; `just fmt-check` runs `cargo fmt --check`. (2) The debug binary path is defined twice: `HYP_BIN=target/debug/hyp` in the Justfile e2e recipe and a default in scripts/dom-test.cjs, so the script can quietly test a stale binary.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The flake formatting check runs `cargo fmt --check` (same definition as `just fmt-check`)
- [ ] #2 scripts/dom-test.cjs fails loudly when HYP_BIN is unset; the binary path is defined in exactly one place per caller
<!-- AC:END -->
