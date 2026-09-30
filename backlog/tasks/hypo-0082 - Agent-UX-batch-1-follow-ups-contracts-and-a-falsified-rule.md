---
id: HYPO-0082
title: 'Agent UX batch 1 follow-ups: contracts and a falsified rule'
status: To Do
assignee: []
created_date: '2026-09-30 17:29'
updated_date: '2026-09-30 17:36'
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
- [ ] #1 Plain show line 1 documented as stable (or the skill uses --json); status JSON shape consistent; one source for the falsified rule wording
- [ ] #2 falsified requires at least one cited evidence that meets the named criterion (ordinary error otherwise); test
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From the batch-1 confirmation: when the only hypothesis needing review is archived, the status header prints both '1 archived needs review' and 'investigation finished: hyp list shows the conclusions'; skip 'finished' when not_shown.needs_review > 0, and document what not_shown.needs_review counts (archived only).
<!-- SECTION:NOTES:END -->
