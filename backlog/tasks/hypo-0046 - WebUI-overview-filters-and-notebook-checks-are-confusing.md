---
id: HYPO-0046
title: 'WebUI: overview filters and notebook checks are confusing'
status: To Do
assignee: []
created_date: '2026-09-30 11:27'
labels:
  - webui
  - ux
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Browser test-drive 2026-09-30. 'Needs review' shows nothing while notebook checks are open, so its meaning is unclear; 'Archived' means 'include archived'; an empty filter result shows the first-run 'Start with a question' state instead of 'no matches'; the search box has no label. The notebook-check banner shows 'WARNING · <full UUID>' in a <pre> with no link to the record, is identical on every page, and still warns about archived hypotheses. Related: HYPO-0038 (checkbox layout).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Filters have clear labels ('Needs review', 'Include archived'); an empty result says 'No matches' and offers to clear filters
- [ ] #2 Each notebook check links to its record, shows its title and skips archived records
<!-- AC:END -->
