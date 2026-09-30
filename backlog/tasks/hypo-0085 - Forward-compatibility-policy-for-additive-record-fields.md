---
id: HYPO-0085
title: Forward-compatibility policy for additive record fields
status: To Do
assignee: []
created_date: '2026-09-30 17:53'
updated_date: '2026-09-30 18:20'
labels:
  - design
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while implementing HYPO-0076. Record data uses deny_unknown_fields, so any additive field (e.g. a gap's resolved_by) makes files written by a newer hyp malformed for an older one, which blocks all its writes. Fine for a single user, painful when a Git-shared project is used with mixed hyp versions. Decide a policy (tolerate-and-preserve unknown fields, a schema_version bump, or documented caveats per field).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A decision record states how additive fields stay readable (or not) by older versions
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From the agent-UX batch 2 architect review: make this a gate before the NEXT additive field (resolved_by on gaps is the first one; an older hyp reports such a gap as malformed and blocks writes). Mixed-version clones syncing through Git are the realistic risk; schema_version stays 1.
<!-- SECTION:NOTES:END -->
