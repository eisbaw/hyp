---
id: HYPO-0036
title: Skill guidance from the dogfood runs
status: To Do
assignee: []
created_date: '2026-09-30 02:26'
updated_date: '2026-09-30 16:30'
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

Carried from human CLI batch 2 (HYPO-0058, 0059, 0063, 0068), 2026-09-30:
- `hyp assess --reviewed` accepts the full review token or a prefix of 12+ hex digits (case-insensitive). The plain `hyp show H` line "review token:" prints it, so the skill may show plain show for people; agents still parse `hyp --json show` (.state.review_token). `hyp apply` expected.hypotheses[H].review_token still requires the full 64 hex digits.
- A stale token conflict (exit 3) says nothing was written and to compare `hyp show H` with what was reviewed; it does not name the changed records (HYPO-0069).
- `--confidence` is 0.0-1.0; 80 is an ordinary error (exit 1) with a hint; a non-number exits 2.
- Stdout of write commands is unchanged (IDs; --json shape). Human summaries go to stderr ONLY when stderr is a terminal, so agents (pipes) see nothing new, except: a write that changes nothing prints "no changes" (or "no changes: ID is not archived" / "is already archived") to stderr in plain mode, even when piped, and exits 0. With --json stderr stays reserved for {"error"}; an unchanged .written[i].revision says it.
- Only one text argument per command may be '-'; two or more is exit 1 before stdin is read.
- `hyp edit` refuses assessments and runs before opening the editor; agents should not use `hyp edit` anyway (interactive).

- (batch 2 review fix) `hyp edit` reopens the editor only on a terminal; without one the first error fails with exit 1, the error and a kept-copy path (also in --json {"error"}).
<!-- SECTION:NOTES:END -->
