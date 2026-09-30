---
id: HYPO-0097
title: Data records follow-ups from the deep review
status: To Do
assignee: []
created_date: '2026-09-30 23:15'
labels:
  - storage
  - hardening
dependencies:
  - HYPO-0090
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
From the HYPO-0090 deep review (QA, architect, adversarial scout). (1) A hand-edited sha256 in a D- file pointing at other stored bytes of the same size is invisible to hyp check (only needs_review catches it, and only inside an assessed basis); consider a record self-hash or recording the D- revision in referrers. (2) No in-tool way to accept a lost stored file: add/archive/delete are blocked; add 'hyp data restore D- FILE' (hash-checked, stores bytes without a new record) and a documented way to accept loss. (3) The legacy attachment check exists three times (read_unlocked, commit validation loop, attachment_bytes); keep one. Data::references and Record::references are both public; narrow Data::references so callers cannot miss data refs. (4) origin accepts multi-line text. (5) Migration guesses media type without the file name. (6) Leftover .tmp files in hyp/assets after a killed capture (see HYPO-0094). (7) Referencing an archived data record gives no warning. (8) decision-0005 should state that data refs on assessments are exempt from the linked-only rule (D- records are immutable). (9) A capture that only restores missing bytes prints 'no changes'.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 hyp data restore exists and is tested; a lost file can be accepted in-tool
- [ ] #2 One legacy-attachment check; Data::references narrowed
<!-- AC:END -->
