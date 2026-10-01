---
id: HYPO-0100
title: Store multi-line values ending in U+2028/U+2029 in the last front-matter field
status: To Do
assignee: []
created_date: '2026-10-01 18:57'
labels:
  - hardening
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found by the HYPO-0013 round-trip property. A multi-line value ending in U+2028 or U+2029 in the last front-matter field (prediction conditions, untestable reason, observed_at) makes serde_yaml end the YAML header without a line feed, so hyp wrote a record file it then reported as malformed, which blocks all writes. Since 9dad9fc encode refuses such a record (invalid_input, naming the field) and writes nothing, so the value cannot be stored at all.

Fixing it needs a format decision: escape the separators in encode, or let decode accept the closing --- after any YAML line break. The ignored test codec::a_last_field_ending_in_a_unicode_separator_is_readable pins the bug, and the round-trip property skips this input class (generate::unicode_separator_bug). This blocks HYPO-0013 AC #4.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A record whose last front-matter field is a multi-line value ending in U+2028 or U+2029 is written and reads back unchanged
- [ ] #2 The format rule chosen (escape in encode, or decode accepting any YAML line break before the closing ---) is documented, and every record file 0.4.0 can write still reads back the same
- [ ] #3 codec::a_last_field_ending_in_a_unicode_separator_is_readable runs without #[ignore] and passes; the round-trip property no longer skips the input class
<!-- AC:END -->
