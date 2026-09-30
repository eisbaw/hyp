---
id: HYPO-0040
title: Require evidence for every judgment except untested
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 05:54'
updated_date: '2026-09-30 11:59'
labels:
  - agents
  - validation
  - mvp2
dependencies:
  - HYPO-0009
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Per decision-0003 (item 4): every assessment whose judgment is not 'untested' must cite at least one evidence ID, for all writers (CLI, apply, WebUI). falsified additionally needs a criterion (already enforced). Replaces the skill's prose rule ('for other judgments citing evidence is your rule'). With linked-only citations (decision-0003 item 2), the cited evidence must also be linked to the hypothesis.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The WebUI assessment form requires evidence and says why
- [x] #2 Skill and README state the rule as enforced; the skill's 'your rule' wording is removed
- [x] #3 Tests: CLI assess --status supported without --evidence exits 1; API 422; DOM form
- [x] #4 Creating an assessment (CLI, apply, WebUI) with a judgment other than untested and no evidence is rejected (exit 1 / 422) with a message naming the rule; checked when the assessment is created, not on stored assessments (decision-0003 item 3: no migration beyond needs-review)
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Rule: judgment != untested requires >=1 cited evidence, checked for every assessment create (CLI, apply, WebUI) with a message naming the rule.
2. WebUI form: evidence field help says it is required and why; the 422 message is shown.
3. Skill and README state the rule as enforced.
4. Tests: CLI exit 1, API 422, DOM form.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented 2026-09-30 with HYPO-0009 (uncommitted).
- Rule in Snapshot::validate_new (model.rs), checked by commit for every record created in the batch, against the state before the batch, after validate() passes: every judgment except untested cites at least one evidence ID, and each cited ID must be linked. Message: "a supported assessment must cite evidence: every judgment except untested needs at least one evidence ID linked to H-...". Exit 1 / HTTP 422.
- Deliberate deviation from AC #1's wording (validate()): validate() runs on every stored record at every read and write, so a standing rule would make any project holding an evidence-less assessment from an older hyp (the WebUI's default judgment was inconclusive, evidence optional) unhealthy and unwritable, with immutable history as the only fix. That breaks decision-0003 item 3 (no migration beyond needs-review). Hence create-time, as for linked-only citations (a later link archive must not invalidate stored assessments). Test judgments_other_than_untested_require_evidence also asserts a stored legacy one stays healthy. Follow-up HYPO-0051 (hyp check warning). AC #1 left unchecked for the orchestrator.
- falsified keeps its standing validate() rule (criterion and evidence).
- WebUI: the form shows "Required for every judgment except untested. Cite only evidence already linked..." under the evidence field; the server's 422 message appears in the form without the conflict draft hint. It does not pre-filter or block client-side (no JS copy of the rule); follow-up HYPO-0049 filters the evidence list.
- Tests: workflow judgments_other_than_untested_require_evidence; cli assess_requires_linked_evidence_for_a_judgment (supported without --evidence exits 1); api a_judgment_without_evidence_is_unprocessable (422); DOM steps (hint shown, 422 shown, nothing written). All RED on HEAD.

Round 2 (2026-09-30): AC changed because decision-0003 (item 3: no migration beyond needs-review) supersedes its wording. Old text of AC #1: "validate() rejects an assessment with judgment other than untested and no evidence (ordinary error, exit 1 / 422), with a message naming the rule". Removed and re-added as AC #4 (the numbering shifted; the backlog CLI cannot edit an AC in place). The rule lives in Snapshot::validate_new, now called at the create's position against the batch state. Verified by workflow judgments_other_than_untested_require_evidence (also: a stored legacy assessment stays healthy), cli assess_requires_linked_evidence_for_a_judgment, api a_judgment_without_evidence_is_unprocessable. QA grammar fix: the article follows the judgment ("an inconclusive assessment must cite evidence"); the test asserts it for inconclusive, supported and weakened.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Committed with the review basis v2 batch. Deep review over two rounds plus a confirmation: round 1 NO-GO (architect M1 batch-growth regression, Codex provenance and archive gaps), all fixed in round 2; confirmation QA GO (70 Rust tests + DOM, x2; flake check), architect GO, Codex GO.
<!-- SECTION:FINAL_SUMMARY:END -->
