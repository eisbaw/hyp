---
id: HYPO-0059
title: 'Assess flow for humans: token from plain show, confidence range, what changed'
status: To Do
assignee: []
created_date: '2026-09-30 12:36'
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
- [ ] #1 Help and errors for --reviewed mention the plain `hyp show` line; decide whether a unique token prefix (e.g. 12+ hex) is accepted
- [ ] #2 --confidence help states the 0.0-1.0 range; a percent-looking value gets a hint
- [ ] #3 An assessment conflict names the records whose basis entries changed since the reviewed token, when the reviewer's basis is known (e.g. via an optional --reviewed-basis or a server-side record of recently issued tokens), or explains how to compare .basis
- [ ] #4 Plain show of an assessment labels based_on as the fingerprint it was based on
<!-- AC:END -->
