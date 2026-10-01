---
id: HYPO-0109
title: 'hyp web: a machine-readable kind on every error response'
status: To Do
assignee: []
created_date: '2026-10-01 18:59'
labels:
  - contract
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
CLI --json errors and the WebUI's transaction errors carry {error, kind} (HYPO-0078), but hyp web's plain-text 4xx responses (axum body rejections such as malformed JSON or a wrong content type) and the 403 responses of the Host/Origin guard have plain-text bodies without a kind. A client or agent talking to the API must parse prose to tell them apart. Noted as not done in HYPO-0067.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every 4xx and 5xx response of hyp web has a JSON body with error and a documented kind, including body rejections and the Host/Origin 403
- [ ] #2 tests/api.rs covers a malformed body, a wrong content type, a bad Host and a bad Origin
- [ ] #3 The README machine contract lists the kinds hyp web can return
<!-- AC:END -->
