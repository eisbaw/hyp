---
id: HYPO-0036
title: Skill guidance from the dogfood runs
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 02:26'
updated_date: '2026-09-30 17:08'
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
- [x] #1 The skill says: short observation as the title (one line), details and raw output in --body or `hyp evidence attach`
- [x] #2 The skill tells agents to search existing hypotheses first and to reuse evidence with `hyp link`
- [x] #3 A second dogfood run shows short evidence titles
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Rewrite agents/hyp/SKILL.md against the current CLI help, README contract and decision-0003 (target ~170 lines).
2. Make the Example runnable as written (IDs captured in shell variables).
3. Add a drift-guard test in tests/cli.rs that runs the Example against the built binary; prove it fails on a renamed flag.
4. Update README Agent onboarding and Development notes.
5. Gate: fmt-check, lint, e2e x2, nix flake check.
<!-- SECTION:PLAN:END -->

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

- SKILL.md rewritten (172 lines): new Method step "look before adding" (hyp list/search, reuse evidence with hyp link); short one-line titles with details in --body / evidence attach and the stdin title rule; write output (one full ID per line, --json {"written":[{id,kind,revision}],revision}, .written[i].revision as expected_revision for hyp apply; stderr summaries only on a terminal); hyp list defaults and exit 2 for impossible filters; Assessing rewritten per decision-0003 (plain show token line, 12+ hex prefix, linked-only citations, evidence for all but untested, what the basis covers, exit 3: re-read and compare .basis); hyp check codes, repair note/commands, restore over delete, blocks_writes only for malformed/attachment/invalid; exit codes 0/1/2/3/141; apply points to hyp apply --help; hyp edit left to people at a terminal.
- Removed stale content: whole-project-on-write, the old closure recipe, separate Finishing section.
- Example is now a runnable script: IDs captured with H1=$(hyp add ...), evidence ID with `| sed -n 1p` (reads all output, no SIGPIPE), review token from the plain `review token:` line (no jq dependency in the Nix sandbox). Ends with assess supported, gap resolved, lifecycle closed, hyp list --needs-review.
- New test tests/cli.rs skill_example_runs_as_written_and_ends_assessed_and_closed: extracts the ```bash blocks of ## Example, runs them with bash -euo pipefail in a fresh project with hyp on PATH, then asserts 2 hypotheses, none needs_review, exactly one closed and supported with one assessment, and hyp check passes. Red/green: renaming `hyp set --resolved` to `--is-resolved` made it fail (exit 2 "unexpected argument --resolved"); reverted, green. Runs in nix flake check too (bash and sed are in stdenv).
- README: Agent onboarding summary updated; Development notes that the Example is a test and must stay runnable.
- AC #3 (second dogfood run) left for the orchestrator.

- Review fix: the Example no longer reads hyp show twice. It keeps the reviewed output (SHOW=$(hyp show "$H1"); printf it) and takes TOKEN from that same output, so the token covers exactly the state reviewed. Assessing now says to take the token from the output you actually reviewed, never from a second read.

- Review round 2: Example also marks the experiment completed and assesses H2 weakened with its own show and token (citing the contradicting link of E1); Commands shows evidence add on H-|P-|F-; Method says to archive a duplicate or mistaken record (hyp archive, undone by hyp restore); README says the Example commands are tested, prose and Commands block reviewed by hand. Drift test now asserts H1 closed/supported and H2 draft/weakened (one assessment each), none needs review, `hyp list --needs-review` prints nothing, and the experiment is completed. Red check: renaming --experiment-status to --exp-status failed the test; reverted.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Skill rewritten to the current contract with a drift test (8939ff2). Dogfood v2: Claude Code and Codex, given only the skill, resumed yesterday's notebook, continued the existing hypothesis instead of duplicating it, falsified it against its criterion, supported the timezone hypothesis, fixed and verified the bug, closed both, nothing needs review; evidence titles 42-65 chars with details in --body (AC #3). Friction filed as HYPO-0071..0077.
<!-- SECTION:FINAL_SUMMARY:END -->
