---
id: HYPO-0033
title: 'hyp list: validate filters and show status'
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 02:26'
updated_date: '2026-09-30 12:28'
labels:
  - cli
  - ux
  - agents
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Test-drive 2026-09-30. `hyp list --kind hypotheses` (a plural typo) or an unknown --status silently prints nothing and exits 0, so an agent concludes there are no hypotheses. The default output interleaves every record kind sorted by ID (links, criteria, assessments), and hypothesis rows show neither the judgment, the lifecycle nor needs-review, so the list cannot answer 'what is the state of my investigation'.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 --kind and --status are validated (clap value enums); an unknown value exits 2 and lists valid values
- [x] #2 Hypothesis rows show judgment, lifecycle and a needs-review marker
- [x] #3 Decide whether plain `hyp list` defaults to hypotheses only (with --all for everything); document it
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. --kind as a clap value enum from a Kind enum (single source for Data::kind); --status from the judgment, lifecycle and experiment-status enums
2. Default kind hypothesis, --all for every kind; a --status that the selected kind cannot have is an error, not an empty list
3. Hypothesis rows: judgment, lifecycle, needs-review marker
4. README/SKILL; CLI tests
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-09-30 implementation (uncommitted, awaiting review):
- `--kind` is a clap value enum (`model::Kind`, now the single source behind `Data::kind()`); `--status` uses a PossibleValuesParser built from the Judgment, Lifecycle and ExperimentStatus enums. Unknown values exit 2 and list the valid ones.
- Decided: plain and --json `hyp list` default to hypotheses; `--all` lists every kind (conflicts with --kind). README and SKILL say so.
- Gotcha found while deciding: with the hypotheses default, `hyp list --status planned` would silently list nothing again. A --status the listed kind cannot have is now an ordinary error (exit 1) naming `--kind experiment or --all`; same for `--needs-review` with a non-hypothesis --kind.
- Plain hypothesis rows: `ID  hypothesis  <judgment> <lifecycle> [needs-review]  title` (fixed-width columns). --json rows already carried `state` (judgment, needs_review, review_token) and record.lifecycle.
- Tests (tests/cli.rs, red on the old binary): list_rejects_an_unknown_kind_or_status_instead_of_listing_nothing, list_shows_hypotheses_by_default_and_every_kind_with_all, list_rows_show_judgment_lifecycle_and_needs_review.

2026-09-30 review fix: impossible filter combinations (`--status planned` with the default kind, `--needs-review` with a non-hypothesis --kind) are now clap-level errors, exit 2 with `Usage: hyp list [OPTIONS]`, via `Cli::check` called from `Cli::parse_checked` in main (library callers of `cli::run` skip it). Test updated; red on the round-1 build (it exited 1).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Committed in the CLI output ergonomics batch (see commit message). Review: QA GO and architect GO, one fix round (contract: 'written' with per-object revisions, init --json same shape, exit 2 for bad filter combinations), then confirmation QA GO and architect GO. Gate: 75 Rust tests plus DOM, nix flake check.
<!-- SECTION:FINAL_SUMMARY:END -->
