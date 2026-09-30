---
id: HYPO-0023
title: 'WebUI: recover a stale draft after a 409 instead of discard-and-retype'
status: To Do
assignee: []
created_date: '2026-09-29 23:50'
updated_date: '2026-09-30 05:33'
labels:
  - webui
  - ux
dependencies:
  - HYPO-0002
  - HYPO-0009
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
From the HYPO-0002 architect review (L2, L3). Now that assessments carry precise preconditions, a stale assessment draft is the most common 409. The hint 'reopen the record to reconcile' does not apply to creates (there is no record to reopen), so the only way to recover is to copy the text, discard the draft and retype. Also, the 'changed while editing' notice only appears when the form is dirty: an open but untouched form gets a 409 with no warning.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 On a 409 the editor offers 'show what changed' (the records whose revisions differ from what the draft saw) and 'rebase draft', which keeps the typed text and re-seeds the preconditions from the current snapshot after the user has seen the changes
- [ ] #2 An open form, dirty or clean, shows a notice when a record its preconditions depend on changes
- [ ] #3 DOM test covers rebase after counter-evidence arrives
<!-- AC:END -->
