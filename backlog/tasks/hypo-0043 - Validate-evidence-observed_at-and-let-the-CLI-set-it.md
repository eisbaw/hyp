---
id: HYPO-0043
title: Validate evidence observed_at (and let the CLI set it)
status: To Do
assignee: []
created_date: '2026-09-30 11:27'
labels:
  - validation
  - cli
  - webui
  - bug
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Browser test-drive 2026-09-30. The WebUI evidence form saves any text as observed_at ('yesterday' was accepted) and hyp check does not report it; the field is a free text box prefilled with a raw ISO timestamp. The CLI has no flag for it, so every CLI observation is stamped with the time of recording, not of observation.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 validate() requires observed_at to be empty or an RFC 3339 timestamp or a date (YYYY-MM-DD); hyp check reports invalid values
- [ ] #2 hyp evidence add accepts --observed-at
- [ ] #3 The WebUI uses a date/datetime input
<!-- AC:END -->
