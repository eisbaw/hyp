---
id: HYPO-0060
title: Show relations with the CLI spelling everywhere
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 12:36'
updated_date: '2026-09-30 13:56'
labels:
  - cli
  - ux
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Interactive terminal test-drive 2026-09-30 (xterm+tmux, first-time user, release build of fda0faa). The CLI requires `competes-with`, but `list --all`, `show L-...`, `graph` and `export` print `competes_with`. Users copy what they see. Files keep underscores (format unchanged).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Every human-facing output uses the hyphenated CLI spelling
- [x] #2 JSON keeps the serialized form (document)
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Relation::as_str/Display now give the CLI spelling (depends-on, competes-with); serde keeps snake_case, so files, JSON and fingerprints are unchanged. show.rs generic fields and export markdown YAML override the relation value. Cycle error now says "depends-on cycle detected". README documents it once.
- Limit: titles of links created before this change keep "competes_with" text (stored data).
- Test: plain_output_spells_relations_as_the_cli_takes_them.

- Review round: web/app.js shows relations as the CLI spells them (relationName: cards, detail, matrix badges, graph edge titles, link form options); the HTML export embeds the same code. The raw "Structured record" JSON stays serialized. dom-test asserts competes-with on All records (fails on the old app.js).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Committed in human CLI batch 1 (see commit message). Review: QA GO and architect GO; small cleanup round (wrap_help dropped, web relation spelling, --project help, multi-line-title repair note); confirmation QA GO, architect GO. Gate: 90 Rust tests + DOM, nix flake check.
<!-- SECTION:FINAL_SUMMARY:END -->
