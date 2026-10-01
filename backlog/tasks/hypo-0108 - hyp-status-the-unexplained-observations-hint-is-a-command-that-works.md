---
id: HYPO-0108
title: 'hyp status: the unexplained-observations hint is a command that works'
status: To Do
assignee: []
created_date: '2026-10-01 18:59'
updated_date: '2026-10-01 19:00'
labels:
  - contract
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Plain hyp status heads the unexplained observations with: hyp add "…" --explains E-…, or hyp link E-… H-… (src/status.rs). hyp link requires --relation and --reason, so an agent that fills in the IDs and runs the suggested link command gets a usage error (exit 2). Agents act on hints literally (decision-0002), so every suggested command must run as printed once its placeholders are filled.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every command hyp status suggests for unexplained observations runs as printed once its placeholders are filled (the link form includes --relation and --reason)
- [ ] #2 Test: fill the placeholders of each suggested command and run it; it exits 0 and the observation is no longer unexplained
<!-- AC:END -->
