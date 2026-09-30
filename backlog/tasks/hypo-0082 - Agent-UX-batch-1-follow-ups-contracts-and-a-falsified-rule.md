---
id: HYPO-0082
title: 'Agent UX batch 1 follow-ups: contracts and a falsified rule'
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 17:29'
updated_date: '2026-09-30 21:10'
labels:
  - agents
  - hardening
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
From the agent-UX batch 1 reviews. (1) Snapshot.bearings is documented 'for the WebUI' but hyp export --format json now includes it too. (2) Plain show line 1 ('H-...  hypothesis  review <12hex>') is scraped by the skill with sed, so it is a de facto contract: document it as stable, or move the skill to --json. (3) hyp status --json mixes counts (linked_evidence, criteria) and ID lists (open_gaps, experiments_without_runs); pick one before the shape settles. (4) The falsified rule is worded twice (validate_fields 'falsified requires a criterion and evidence' and Judgment::requirement). (5) Tool rule candidate (decision-0002: rules the tool enforces): a falsified assessment should cite at least one evidence that meets the criterion it names (Stance Against via that criterion); today only the criterion's presence is checked. (6) status counts evidence whose file is unparseable. (7) Note in the README the asymmetry that meeting a criterion is decisive while not meeting it is only mild corroboration.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Plain show line 1 documented as stable (or the skill uses --json); status JSON shape consistent; one source for the falsified rule wording
- [x] #2 falsified requires at least one cited evidence that meets the named criterion (ordinary error otherwise); test
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. validate_new: falsified must cite evidence with an active supports link to the named criterion (bearing Against via it).
2. One wording: Judgment::requirement feeds validate_fields and validate_new.
3. README: show line 1 stable; criterion asymmetry; status JSON shape.
4. status: criteria and linked_evidence as ID lists (counts derived); skip evidence that is not loaded; no "finished" while archived needs review.
5. Snapshot.bearings doc; skill Assessing bullet; fix tests that falsified without meeting evidence.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From the batch-1 confirmation: when the only hypothesis needing review is archived, the status header prints both '1 archived needs review' and 'investigation finished: hyp list shows the conclusions'; skip 'finished' when not_shown.needs_review > 0, and document what not_shown.needs_review counts (archived only).

- (a) validate_new: falsified must cite evidence with an active supports link to its (active) criterion (a bearing Against via it); ordinary error quoting the rule; create-time only, stored ones stay valid. Archived criterion named explicitly.
- (b) Judgment::requirement is the one wording; validate_fields quotes it.
- (c) README machine contract: plain show line 1 (ID  kind [  review <12hex>]) is stable.
- (d) status rows list IDs, never counts: criteria and linked_evidence became ID arrays (plain text still counts).
- (e) header omits "investigation finished" while not_shown.needs_review > 0 (archived only; documented).
- (f) Snapshot.bearings doc mentions export JSON. (g) linked_evidence skips evidence that did not load. (h) README note on the criterion asymmetry.
- Skill Assessing bullet and exit-code kinds updated (199 lines). README example, apply/assess help, WebUI hint and DOM test updated for the rule.

- Review round: validate_new skips the falsified criterion-meeting check when any cited evidence is missing, so the missing ID is reported instead. Test: cli a_falsified_citing_evidence_by_prefix_is_told_to_use_full_ids.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Committed in the schema-evolution batch (0.2.0; see commit message). Deep review: QA GO (4 scenarios, 7 extra DOM runs clean), architect GO; small fix round (min_hyp_version in Config, falsified check skipped on missing evidence, not_found with ids, apply creates with unmatched references not_found); confirmation GO from both. Codex review not run: the configured Codex model is rejected for the account. Gate: 130 Rust tests + DOM, nix flake check.
<!-- SECTION:FINAL_SUMMARY:END -->
