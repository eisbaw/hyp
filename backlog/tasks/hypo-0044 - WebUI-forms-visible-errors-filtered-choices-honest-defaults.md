---
id: HYPO-0044
title: 'WebUI forms: visible errors, filtered choices, honest defaults'
status: To Do
assignee: []
created_date: '2026-09-30 11:27'
updated_date: '2026-10-01 19:00'
labels:
  - webui
  - ux
dependencies:
  - HYPO-0009
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Browser test-drive 2026-09-30 (screenshots /tmp/claude-1000/-home-mpedersen-topics-hyp/b93cfa51-31aa-4941-a678-5890abf7545d/scratchpad/webtd/19-new-hyp-error.jpg, 25-conflict.jpg). (1) In tall dialogs the error, including 409 conflicts, appears at the bottom of the scroll area below the fold, so Save looks like a no-op; errors come one per submit (an assessment took four tries); fields are not marked required; 'title is required' vs label 'Statement / title'. (2) The assessment form lists criteria and evidence of every hypothesis, which the server then rejects; per decision-0003 only linked evidence may be cited. The experiment form's 'Predictions / criteria to freeze' is unfiltered, shows no IDs and does not explain 'freeze'. (3) Dropdowns mix kinds without kind/ID (evidence 'Interpretation target'); the Relationships picker truncates identical-prefix titles. (4) Defaults: relation 'supports', judgment 'inconclusive', an empty interpretation silently copies the evidence title; multi-selects give no Ctrl-click hint. (5) Saving a criterion or evidence from a hypothesis's Add button navigates to the new record instead of back to the hypothesis.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Errors appear next to the offending field and at the top of the dialog, and are scrolled into view; required fields are marked; all client-side-detectable problems are reported in one submit
- [ ] #2 Assessment and experiment forms offer only records that belong to or are linked to the hypothesis, with kind and ID
- [ ] #3 No biased defaults: relation and judgment start unselected; an empty interpretation is rejected rather than copied
- [ ] #4 After saving a child record the user returns to the parent hypothesis
- [ ] #5 DOM test covers the error placement and the filtered options
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Re-confirmed after the 0.4.0 merge (2026-10-01), covered by AC #2 and #3: the "Review hypothesis" (assessment) form still defaults the judgment to inconclusive (select("judgment", …, "inconclusive") in web/app.js) even when no evidence is linked, so the default cannot be saved (every judgment except untested needs cited evidence); its "Evidence considered" list is objectOptions("evidence"), every evidence record, linked or not, while the server accepts only linked evidence (decision-0003). HYPO-0049 covers the evidence-list half with a server-side rule.
<!-- SECTION:NOTES:END -->
