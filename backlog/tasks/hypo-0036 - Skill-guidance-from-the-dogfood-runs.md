---
id: HYPO-0036
title: Skill guidance from the dogfood runs
status: To Do
assignee: []
created_date: '2026-09-30 02:26'
updated_date: '2026-09-30 05:54'
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
<!-- SECTION:NOTES:END -->
