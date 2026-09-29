---
id: HYPO-0005
title: CRLF record files are rejected with a misleading error
status: To Do
assignee: []
created_date: '2026-09-29 22:15'
updated_date: '2026-09-29 22:30'
labels:
  - bug
  - storage
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found in the initial review. Reproduced.

`decode` requires the exact bytes `---\n` and `\n---\n`. A record saved with CRLF line endings (Windows editor, `core.autocrlf=true` checkout) is reported as `file must start with YAML front matter (---)`, which is wrong and confusing. It then blocks all writes for the whole project.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A CRLF or BOM-prefixed record fails with an error that names the line-ending/BOM problem and the fix (not 'must start with YAML front matter')
- [ ] #2 `hyp init` (and open, if missing) writes `hyp/.gitattributes` with `* text eol=lf`, which is harmless without Git. Normalising on decode is rejected because revisions are raw-byte hashes: CRLF and LF copies would disagree on needs-review and tool writes would flip line endings
- [ ] #3 Unit tests for CRLF and BOM records
<!-- AC:END -->
