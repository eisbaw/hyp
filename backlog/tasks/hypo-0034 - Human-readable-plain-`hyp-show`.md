---
id: HYPO-0034
title: Human-readable plain `hyp show`
status: To Do
assignee: []
created_date: '2026-09-30 02:26'
updated_date: '2026-09-30 05:33'
labels:
  - cli
  - ux
dependencies:
  - HYPO-0009
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Test-drive 2026-09-30. Plain `hyp show H` prints the raw YAML front matter, including empty internal fields (tags: [], scope: '', untestable_reason: ''), nanosecond timestamps and 'archived: false'. The related list shows links by their auto title ('Evidence for H-ebba1771') without the relation or the evidence observation, and the review token needed by `hyp assess --reviewed` is only reachable via --json.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Plain show renders a readable summary: title, scope, lifecycle, judgment/confidence/needs-review, the review token, criteria, predictions, evidence grouped by relation with observation and source, experiments with runs, gaps
- [ ] #2 Empty fields are omitted; timestamps are shown to the second
- [ ] #3 --json output is unchanged
<!-- AC:END -->
