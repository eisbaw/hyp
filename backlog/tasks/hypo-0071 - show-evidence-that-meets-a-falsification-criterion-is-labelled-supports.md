---
id: HYPO-0071
title: 'show: evidence that meets a falsification criterion is labelled ''supports'''
status: To Do
assignee: []
created_date: '2026-09-30 17:08'
labels:
  - agents
  - ux
  - webui
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Dogfood v2, 2026-09-30 (Claude Code and Codex resuming yesterday's notebook on a timezone bug; both succeeded; friction reported by the agents). Evidence linked 'supports' to a criterion F (i.e. the falsifying observation was made) appears under 'supports' in plain `hyp show H`, so the proof that H is wrong reads as support for H. Agents and humans misread this. Also applies to the WebUI and the evidence matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Plain show, --json show (a derived field) and the WebUI present evidence by what it means for the hypothesis: 'meets criterion F-... (counts against H)', 'matches prediction P-...', 'supports/contradicts/qualifies H'
- [ ] #2 Test: evidence supporting a criterion is shown under the criterion, not as support
<!-- AC:END -->
