---
id: HYPO-0112
title: >-
  Shell quoting of repair commands: zsh =word, POSIX-safe escapes, bidi
  controls, NUL, shared test vectors
status: To Do
assignee: []
created_date: '2026-10-01 18:59'
labels:
  - hardening
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
cli::shell_word (Rust, a15f73a) and shellWord (web/app.js, 56232af) quote repair commands that carry record text, for plain hyp check, the WebUI and the HTML export. Gaps:
- "=" is in the safe set, so a word starting with "=" stays unquoted and zsh expands it (EQUALS option, on by default).
- Control characters are written as \xHH inside $'…'; \xHH followed by a hex digit is unspecified in POSIX (bash reads at most two digits, other shells may not). Three-digit octal \NNN is unambiguous.
- Bidi controls and U+2028/U+2029 pass through as they are, so the displayed line can read differently from what runs.
- A NUL cannot be part of an argv word; what the quoting does with one is undefined.
- The two implementations are kept equal only by the dom-test comparing the page with hyp check output for a few bodies.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A word starting with = is quoted
- [ ] #2 Control characters are written as three-digit octal escapes inside $'…', never as \xHH
- [ ] #3 Bidi controls (U+202A to U+202E, U+2066 to U+2069, U+200E, U+200F, U+061C) and U+2028/U+2029 are escaped, so the displayed line shows them
- [ ] #4 A NUL in a value follows one defined, tested rule (refused or escaped) in both implementations
- [ ] #5 One test-vector file (input, expected word) is checked by both the Rust test and the dom-test, and bash turns each expected word back into its input
<!-- AC:END -->
