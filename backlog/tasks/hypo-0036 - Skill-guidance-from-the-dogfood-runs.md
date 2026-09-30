---
id: HYPO-0036
title: Skill guidance from the dogfood runs
status: To Do
assignee: []
created_date: '2026-09-30 02:26'
updated_date: '2026-09-30 12:22'
labels:
  - agents
  - docs
dependencies:
  - HYPO-0028
  - HYPO-0033
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Test-drive 2026-09-30: fresh Claude Code and Codex sessions, given only the installed skill, investigated a bug (see HYPO-0016 notes). Both followed the method. Paper cuts in how they used hyp: evidence titles became long paragraphs (the whole observation, numbers and commands in the title; `hyp list` becomes unreadable), since the skill shows the observation as the title and never mentions --body. Claude also first tried to capture IDs with `$(hyp add ...)`, which its permission rules blocked; the skill could say that write commands print one ID per line to read back. The skill does not tell agents to look for existing hypotheses (`hyp search`, `hyp list --kind hypothesis`) before adding new ones; this will matter in long-lived notebooks.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The skill says: short observation as the title (one line), details and raw output in --body or `hyp evidence attach`
- [ ] #2 The skill tells agents to search existing hypotheses first and to reuse evidence with `hyp link`
- [ ] #3 A second dogfood run shows short evidence titles
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Per decision-0003, rewrite the skill's Assessing section once, after HYPO-0009 and the evidence-rule task: linked-only citations, and evidence enforced for all judgments except untested.

Carried from HYPO-0028/0033/0034 (2026-09-30): write commands with --json now print `{"ids":[{"id","kind"}],"revision"}` (no snapshot; `.ids[0].id`, not `.ids[0]`); plain `hyp apply` prints IDs, not JSON; `evidence attach` prints the evidence ID. `hyp list` lists hypotheses only by default (`--kind K` or `--all` for others) and rows show judgment, lifecycle and needs-review; unknown --kind/--status exit 2. Plain `hyp show H` now shows the review token and a readable summary, so a skill example may use it for humans, but agents should still parse `hyp --json show`. SKILL.md Commands was minimally updated for the shapes; the rewrite is this task.

Correction to the note above (2026-09-30 review fix): the write JSON is `{"written":[{"id","kind","revision"}],"revision"}` (not `ids`); read `.written[0].id`, and `.written[i].revision` is the record's revision to state in a follow-up update/archive/delete. `hyp --json init` prints the same shape. Impossible `hyp list` filter combinations exit 2.
<!-- SECTION:NOTES:END -->
