---
id: HYPO-0091
title: 'hyp observe: record an observation first and explain it later'
status: To Do
assignee: []
created_date: '2026-09-30 20:09'
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
- [ ] #1 hyp observe records a standalone observation and prints its ID (plain and --json written shape)
- [ ] #2 hyp add --explains E-... creates a hypothesis linked to the observation(s) in one command
- [ ] #3 hyp status lists unexplained observations; hyp show E- lists the hypotheses it bears on
- [ ] #4 The skill documents the observation-first flow; a test runs it end to end (observe, add two explanations, falsify one, support the other)
<!-- AC:END -->
