---
id: HYPO-0085
title: Forward-compatibility policy for additive record fields
status: Done
assignee:
  - '@claude'
created_date: '2026-09-30 17:53'
updated_date: '2026-09-30 21:10'
labels:
  - design
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while implementing HYPO-0076. Record data uses deny_unknown_fields, so any additive field (e.g. a gap's resolved_by) makes files written by a newer hyp malformed for an older one, which blocks all its writes. Fine for a single user, painful when a Git-shared project is used with mixed hyp versions. Decide a policy (tolerate-and-preserve unknown fields, a schema_version bump, or documented caveats per field).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A decision record states how additive fields stay readable (or not) by older versions
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. store.rs: SCHEMAS table (schema -> first hyp version), Config read in two phases (lenient schema_version check, then strict), new ErrorKind::UnsupportedSchema for a too-new notebook.
2. Data::schema(): 2 for a gap with resolved_by. commit_written raises config.toml to the max needed, as a journal entry next to the records (recover accepts config.toml).
3. Bump crate to 0.2.0 so the table is honest.
4. Tests: schema-1 unchanged until --by, then 2; journal roll-forward of config; schema-3 refused (with/without min_hyp_version); unknown key/garbage invalid.
5. README Files + machine contract; skill kinds.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From the agent-UX batch 2 architect review: make this a gate before the NEXT additive field (resolved_by on gaps is the first one; an older hyp reports such a gap as malformed and blocks writes). Mixed-version clones syncing through Git are the realistic risk; schema_version stays 1.

- Implemented decision-0004: store::SCHEMAS table (1 -> 0.1.0, 2 -> 0.2.0); crate bumped to 0.2.0 so the table is honest.
- config.toml keeps {schema_version, name}; read in two phases: lenient schema check first (newer -> kind unsupported_schema, exit 1, naming min_hyp_version from the config when present, else "newer than <this version>"), then strict parse (unknown key / garbage / schema 0 -> invalid_input).
- Data::schema(): 2 for a gap with resolved_by. commit_written raises config.toml to the max over all records, as a journal entry next to the records (recover accepts config.toml); only when the commit writes something; never lowers. Reads never raise.
- read_unlocked re-checks the schema, so a long-running hyp web refuses a notebook raised past it.
- min_hyp_version is written only from schema 3 on (none yet), so hyp 0.1.0 keeps its "unsupported schema version 2" message.
- Tests (tests/cli.rs): a_schema_1_notebook_is_raised_to_2_only_by_the_first_resolved_by, an_interrupted_schema_raise_is_rolled_forward_with_the_records, a_notebook_of_a_newer_schema_is_refused_with_the_version_to_upgrade_to, a_config_with_an_unknown_key_or_garbage_is_invalid, the_schema_table_matches_this_version_and_the_readme. README: Schema versions section, unsupported_schema kind. Follow-ups: HYPO-0088, HYPO-0089.

- Review round: Config gains min_hyp_version (Option, omitted when None); store::raised() sets it from SCHEMAS when the raised schema >= FIRST_SCHEMA_NAMING_ITS_HYP (3), none before. Unit test store::tests::a_raise_names_the_hyp_version_from_schema_3_on (fake 4-row table).
- Comment at Store::target: an older hyp recovering a journal with a config.toml entry rejects it (needs crash mid-raise plus downgrade; accepted).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Committed in the schema-evolution batch (0.2.0; see commit message). Deep review: QA GO (4 scenarios, 7 extra DOM runs clean), architect GO; small fix round (min_hyp_version in Config, falsified check skipped on missing evidence, not_found with ids, apply creates with unmatched references not_found); confirmation GO from both. Codex review not run: the configured Codex model is rejected for the account. Gate: 130 Rust tests + DOM, nix flake check.
<!-- SECTION:FINAL_SUMMARY:END -->
