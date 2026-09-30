---
id: HYPO-0091
title: 'hyp observe: record an observation first and explain it later'
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 20:09'
updated_date: '2026-09-30 22:09'
labels:
  - feature
  - agents
  - cli
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
User request (2026-09-30): support observation-first (abductive) work. Start from an observation and form several competing explanatory hypotheses for it, instead of always starting from a hypothesis.

The data model already allows it: evidence records have no owner and connect to hypotheses only through links, and the WebUI can create evidence without a target. But the CLI's `hyp evidence add` requires a hypothesis, no command creates a hypothesis already explaining an observation, `hyp status` and `list` do not surface unexplained observations, and the skill teaches only hypothesis-first.

Proposal:
- `hyp observe "Short observation" --source ... [--locator ...] [--body ...] [--data D-...]` records a standalone observation (an evidence record with no link) and prints its E- ID. `hyp evidence add` keeps working, and may make its target optional.
- `hyp add "Explanation" --explains E-...` (repeatable) creates a hypothesis and links each observation to it (relation supports, with a required reason or a default), in one command. Several hypotheses explaining the same observation are then linked competes-with (optionally `--competes-with H-...`).
- `hyp status` (and --json) lists unexplained observations: evidence not linked to any active hypothesis, criterion or prediction.
- `hyp show E-...` lists the hypotheses an observation bears on and how. The evidence matrix already compares them.
- The skill gets a short 'start from an observation' section. Keep it under 200 lines.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 hyp observe records a standalone observation and prints its ID (plain and --json written shape)
- [x] #2 hyp add --explains E-... creates a hypothesis linked to the observation(s) in one command
- [x] #3 hyp status lists unexplained observations; hyp show E- lists the hypotheses it bears on
- [x] #4 The skill documents the observation-first flow; a test runs it end to end (observe, add two explanations, falsify one, support the other)
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. model.rs: Snapshot::bears_on and Snapshot::unexplained (one definition, from the derived bearings)
2. cli.rs: hyp observe (+ --observed-at parser), hyp add --explains/--reason/--competes-with, JSON show bears_on/unexplained
3. status.rs: unexplained_observations (JSON all, plain first 5 + "(N more)"); show.rs: Bears on section
4. Tests in tests/cli.rs (e2e flow, observe contract, --explains errors, status cap/archiving); skill_flow.sh uses observe/--explains/--competes-with
5. Skill section (< 200 lines), README
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Unexplained = evidence not archived that is in no bearings of a non-archived hypothesis (active link to an active H, or to an active F/P of one). Evidence only a run or gap names counts as unexplained: an assessment cannot cite it until linked. One function (Snapshot::unexplained) feeds status, plain show and JSON show.
- --explains links are E supports H (default reason "Proposed as an explanation of this observation"); --competes-with links go from the new H to each rival, reason names shared observations. Repeated IDs (also by prefix) get one link. Output: H, then explains links, then competes links.
- --observed-at validated only at create time (RFC 3339 or YYYY-MM-DD, stored as given, exit 2 otherwise); no schema bump, no change to stored records.
- Mutation check: dropping the archived-hypothesis filter or the dedupe turns the new status and --explains tests red.
- Skill kept at 199 lines by tightening Evidence/Exit codes/Files/Assessing prose and dropping duplicated search lines.
- Follow-up filed: HYPO-0092 (WebUI).

Review round (QA and architect GO, one shared gap):
- "Unexplained" now means no LIVE hypothesis (not archived, judgment not falsified; Snapshot::is_live) accounts for the observation: a bearing of stance for or qualifies. Observations whose explanations are all falsified, or that only count against live hypotheses or only falsified one, are unexplained. Same function (Snapshot::unexplained) and key (unexplained_observations). Rule stated in status/observe help, README, skill.
- show E- Bears on: stance, judgment, lifecycle; JSON bears_on gains judgment.
- rival_reason stores full IDs. init Next hint mentions hyp observe.
- e2e flow: the falsifying observation is linked to the survivor (asserted unexplained before that).
- Red first: new test failed at the contradicts-only case; a mutation ignoring the falsified judgment fails it at the falsified case.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
hyp observe records standalone observations; hyp add --explains/--competes-with creates explanations and rival links in one write; an observation is explained only when a live (not archived, not falsified) hypothesis has a for/qualifies bearing to it; hyp status lists unexplained observations; hyp show E- lists what it bears on with judgments. Review: QA GO, architect GO; fix round redefined 'unexplained' (both reviewers); confirmation GO from both. Gate: 135 Rust tests + DOM, nix flake check. Follow-ups: HYPO-0092 (WebUI), HYPO-0093.
<!-- SECTION:FINAL_SUMMARY:END -->
