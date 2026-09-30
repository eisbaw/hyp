---
id: decision-0004
title: 'Schema evolution: strict parsing, versioned notebook, clear upgrade error'
date: '2026-09-30 19:35'
status: accepted
---
## Context

Record files are parsed strictly (`deny_unknown_fields`): an unknown field is a typo or corruption guard. The first additive field (a gap's `resolved_by`, HYPO-0076) showed the cost: an older hyp reading the same notebook through Git or a sync reports such a file as malformed and blocks all writes, one confusing error per file.

## Decision

Keep strict parsing. The notebook carries a schema version in `hyp/config.toml`. A hyp binary supports a range of schema versions. Introducing a field or value that older versions cannot read raises the schema version; the notebook is upgraded when the new feature is first written (not silently on read). An older hyp that meets a newer schema refuses the whole project up front with one clear message naming the minimum hyp version, instead of reporting individual files as malformed.

## Consequences

- Typo detection on hand edits stays.
- Each additive field or value is a schema bump with a test that an older schema version is read correctly and that a too-new version is refused with the upgrade message.
- Mixed-version collaboration requires everyone to upgrade once a newer feature has been used; that is explicit rather than a per-file surprise.
- The schema version and minimum hyp version are part of the documented machine contract.
