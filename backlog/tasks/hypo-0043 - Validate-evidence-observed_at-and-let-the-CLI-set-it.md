---
id: HYPO-0043
title: Validate evidence observed_at (and let the CLI set it)
status: In Progress
assignee:
  - '@implementer-B'
created_date: '2026-09-30 11:27'
updated_date: '2026-10-01 19:02'
labels:
  - validation
  - cli
  - webui
  - bug
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Browser test-drive 2026-09-30. The WebUI evidence form saves any text as observed_at ('yesterday' was accepted) and hyp check does not report it; the field is a free text box prefilled with a raw ISO timestamp. The CLI has no flag for it, so every CLI observation is stamped with the time of recording, not of observation.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 validate() requires observed_at to be empty or an RFC 3339 timestamp or a date (YYYY-MM-DD); hyp check reports invalid values
- [x] #2 hyp evidence add accepts --observed-at
- [ ] #3 The WebUI uses a date/datetime input
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Interactive terminal test-drive 2026-09-30 (xterm+tmux, first-time user, release build of fda0faa). Confirmed as a paper cut in the exact scenario tested: evidence about 'the night of 2026-09-12' is stamped with the recording time.

HYPO-0091 added hyp observe --observed-at with a clap value parser (cli.rs parse_observed_at: RFC 3339 or YYYY-MM-DD, stored as given, exit 2 otherwise). Reuse it for hyp evidence add --observed-at (AC #2); stored records are still not validated (AC #1).

2026-10-01 implementation (implementer B): model::is_observed_at (RFC 3339, or exactly YYYY-MM-DD) is shared by the CLI value parser and validate(); hyp check reports any other non-empty stored value as invalid (blocks writes; repairable with hyp set E --observed-at, see HYPO-0067). hyp evidence add --observed-at and hyp set E --observed-at added.

A write from any writer (CLI, apply, WebUI) refuses a new or changed observed_at more than a day after now (covers HYPO-0093 AC #2); hyp check does not judge the clock.

AC #3 (WebUI date input) is the WebUI lane. The server rejects datetime-local values without an offset (YYYY-MM-DDTHH:MM), so the form must send a date or an RFC 3339 timestamp.

Review round 2 (P1): a stored unreadable observed_at no longer blocks writes. hyp check reports it as the warning bad_observed_at (blocks_writes false; --strict fails), with a repair command that moves the text into the body and clears the field (hyp set E --body=... --observed-at=). Writes refuse a new or changed unreadable or future value for every writer (model::observed_at_refusal), kind invalid_input with a bad_observed_at diagnostic. --observed-at '' clears it (unknown) on observe, evidence add and set.

Closing review 2026-10-01 (after the 0.4.0 merge): AC #1 and #2 are met; AC #3 is NOT met, so the task stays In Progress. The WebUI evidence form still renders observed_at as a text input (field("observed_at", "Observation date", …, "text", …) in web/app.js). Stream A (5c49f6c) only changed its default (now only for a new record; an edit keeps an empty value) and its help text (RFC 3339 with offset, or YYYY-MM-DD). The server refuses unreadable new values, so bad input is rejected on save, but the form is not a date/datetime input. Note for whoever takes AC #3: a plain datetime-local value (YYYY-MM-DDTHH:MM, no offset) is refused by the server, so the form must send a date or add the offset.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
observed_at is validated for every writer, the CLI can set it, and a stored unreadable value warns instead of blocking. The WebUI date input (AC #3) is still open.

Changes (335ae01, 8bd8ba2):
- model::is_observed_at (RFC 3339, or exactly YYYY-MM-DD) shared by the CLI value parser and validate().
- hyp check reports a stored unreadable value as the warning bad_observed_at (blocks_writes false; --strict fails on it) with a repair that moves the text into the body and clears the field: hyp set E --body=… --observed-at=.
- Every writer (CLI, apply, WebUI) refuses a new or changed value that is unreadable or more than a day ahead: invalid_input with a bad_observed_at diagnostic (model::observed_at_refusal).
- hyp evidence add --observed-at and hyp set E --observed-at; --observed-at '' states unknown (observe, evidence add, set).

Tests: tests/cli.rs an_unreadable_stored_observed_at_warns_and_does_not_block and updated tests, red against the previous src. just e2e and nix flake check green at a38c06c.

Open: AC #3, the WebUI form uses a text input. Follow-up: the repair has no expected-revision precondition (HYPO-0101).
<!-- SECTION:FINAL_SUMMARY:END -->
