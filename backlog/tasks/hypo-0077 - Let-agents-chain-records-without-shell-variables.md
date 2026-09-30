---
id: HYPO-0077
title: Let agents chain records without shell variables
status: To Do
assignee: []
created_date: '2026-09-30 17:08'
labels:
  - agents
  - ux
dependencies:
  - HYPO-0053
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Dogfood v2, 2026-09-30 (Claude Code and Codex resuming yesterday's notebook on a timezone bug; both succeeded; friction reported by the agents). Under common Claude Code permission rules, commands with $VAR or $(...) are blocked ('Contains simple_expansion'); the skill's recommended `H1=$(hyp add ...)` capture fails, so the agent copied IDs by hand and skipped recording the experiment and run (too many ID round-trips). Two parts: the skill must not depend on shell variables (show the read-the-printed-ID way first), and hyp apply should accept batch-local references so a multi-record step (hypothesis+criterion+prediction, or experiment+run+evidence) is one command. Related: HYPO-0053 (apply defaults and patch updates).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The skill's main flow works with commands that contain no shell expansion (IDs read from printed output; short prefixes); the variable-capture example is optional
- [ ] #2 hyp apply accepts batch-local references (e.g. "id": "@x" on a create and "@x" wherever an ID is expected later in the batch), resolved server-side; documented in apply --help
- [ ] #3 A test creates hypothesis, criterion, experiment, run and evidence in one apply batch using local references
<!-- AC:END -->
