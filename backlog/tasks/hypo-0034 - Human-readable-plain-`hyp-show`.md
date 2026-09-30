---
id: HYPO-0034
title: Human-readable plain `hyp show`
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 02:26'
updated_date: '2026-09-30 12:28'
labels:
  - cli
  - ux
dependencies:
  - HYPO-0009
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Test-drive 2026-09-30. Plain `hyp show H` prints the raw YAML front matter, including empty internal fields (tags: [], scope: '', untestable_reason: ''), nanosecond timestamps and 'archived: false'. The related list shows links by their auto title ('Evidence for H-ebba1771') without the relation or the evidence observation, and the review token needed by `hyp assess --reviewed` is only reachable via --json.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Plain show renders a readable summary: title, scope, lifecycle, judgment/confidence/needs-review, the review token, criteria, predictions, evidence grouped by relation with observation and source, experiments with runs, gaps
- [x] #2 Empty fields are omitted; timestamps are shown to the second
- [x] #3 --json output is unchanged
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. New src/show.rs: plain rendering of a hypothesis (header, state, review token, criteria, predictions, evidence by relation, experiments with runs, gaps, current assessments) and a compact field list for other kinds
2. Empty fields omitted, timestamps to the second, archived children hidden
3. --json path untouched; CLI test
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-09-30 implementation (uncommitted, awaiting review):
- New src/show.rs (`show::plain`); the --json branch of `hyp show` is untouched.
- Hypothesis: title, scope, assumptions, lifecycle, untestable reason, judgment (confidence, needs review), review token, tags, created/updated to the second, body; then sections criteria, predictions (conditions), evidence grouped supports/contradicts/qualifies (title, source, locator, observed, attachment count, link ID and target when it is a criterion/prediction, link reason unless it repeats the title), experiments with runs (outcome, cited evidence), gaps (open/resolved), links to other hypotheses, current assessment(s) with rationale.
- Other kinds: the data fields alphabetically (serde_json map order), frozen copies and lists by ID, empties omitted, then "Referenced by".
- Archived child records are hidden in plain show (they cannot be cited); superseded assessments are not listed. Both remain in --json.
- The plain text is documented as for people and may change; agents keep --json.
- Test: plain_show_summarises_a_hypothesis_for_people (red on the old binary).

2026-09-30 review fix: archived criteria and predictions are listed with [archived] (they are in the review basis); evidence shows its body (the observation) under its title; relations use the CLI spelling (competes-with); the evidence section comes from the new `Snapshot::evidence_links` (which `linked_evidence` is now built on), so it cannot drift from what an assessment may cite. Linked evidence that is itself archived is shown marked. Test extended; red on the round-1 build.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Committed in the CLI output ergonomics batch (see commit message). Review: QA GO and architect GO, one fix round (contract: 'written' with per-object revisions, init --json same shape, exit 2 for bad filter combinations), then confirmation QA GO and architect GO. Gate: 75 Rust tests plus DOM, nix flake check.
<!-- SECTION:FINAL_SUMMARY:END -->
