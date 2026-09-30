---
id: HYPO-0059
title: 'Assess flow for humans: token from plain show, confidence range, what changed'
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 12:36'
updated_date: '2026-09-30 16:36'
labels:
  - cli
  - ux
  - agents
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Interactive terminal test-drive 2026-09-30 (xterm+tmux, first-time user, release build of fda0faa). The biggest first-run friction. --reviewed help and errors point to `hyp --json show ... .state.review_token` although plain `hyp show` prints 'review token:'; a short token prefix is rejected (full 64 hex required). `--confidence high` gives 'invalid float literal' and `--confidence 80` 'must be between 0 and 1' with no range in the help. An exit-3 conflict says 'review what changed' without naming what changed. The assessment's based_on (fingerprint) and the token passed differ by design, which confuses readers of `hyp show A-`.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Help and errors for --reviewed mention the plain `hyp show` line; decide whether a unique token prefix (e.g. 12+ hex) is accepted
- [x] #2 --confidence help states the 0.0-1.0 range; a percent-looking value gets a hint
- [x] #3 An assessment conflict names the records whose basis entries changed since the reviewed token, when the reviewer's basis is known (e.g. via an optional --reviewed-basis or a server-side record of recently issued tokens), or explains how to compare .basis
- [x] #4 Plain show of an assessment labels based_on as the fingerprint it was based on
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. --reviewed: accept 12..64 hex (case-insensitive), compare as a prefix of the current token; help and errors point at the "review token:" line of plain `hyp show`.
2. --confidence: clap parser accepts any number (non-numbers exit 2 with the range); out of range exits 1, with "did you mean 0.NN?" for 1 < c <= 100.
3. Conflict message: nothing written; `hyp show H` lists the basis to compare with. Follow-up task for storing the reviewed basis.
4. Plain show of an assessment labels based_on "based on fingerprint".
5. Tests in tests/cli.rs; README.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Implemented as planned (src/cli.rs Assess arm, parse_confidence, confidence_in_range; src/show.rs).
- Prefix: 12 hex digits minimum (48 bits); `hyp apply` expected.hypotheses still takes only the full 64-hex token (the JSON contract, unchanged).
- Exit codes kept: `--confidence high` stays exit 2 (now says "expected a number from 0.0 to 1.0"); 80 stays exit 1 (message now "--confidence must be from 0.0 to 1.0, got 80 (did you mean 0.8?)"). The model's apply-path message "confidence must be between 0 and 1" is unchanged.
- AC#3: the reviewer's basis is not stored, so the conflict explains how to compare (hyp show H) instead of naming records; follow-up HYPO-0069.
- Tests: assess_accepts_a_review_token_prefix_of_12_or_more_hex_digits, a_percent_looking_confidence_gets_a_hint, updated assess_is_based_on_the_state_the_agent_reviewed (a 63-hex prefix is now valid; 11 hex and 65 hex are rejected). All fail on HEAD 277b78a.

- Review fix: the "did you mean 0.NN?" hint only for 2..=100 (1.5 is more likely a slip than 1.5 %).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Committed in human CLI batch 2 (see commit message). Review: QA GO; architect NO-GO (edit reopen loop with a non-interactive editor; abort message lacked the error) -> fixed; confirmation QA GO, architect GO. Gate: 99 Rust tests + DOM, nix flake check.
<!-- SECTION:FINAL_SUMMARY:END -->
