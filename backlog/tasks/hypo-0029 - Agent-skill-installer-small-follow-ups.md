---
id: HYPO-0029
title: 'Agent skill installer: small follow-ups'
status: To Do
assignee: []
created_date: '2026-09-30 01:47'
updated_date: '2026-09-30 02:00'
labels:
  - agents
  - hardening
dependencies:
  - HYPO-0016
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Non-blocking findings from the HYPO-0016 review:
- newer() treats pre-release versions (0.2.0-rc.1) as unparsable, so a skill installed by a newer pre-release is silently downgraded.
- `update --force` over a skill from a newer hyp reports 'updated' although it is a downgrade.
- InitAgents repeats the Agent variants; Step.target stores a derivable path; print_steps uses serde_json::to_value only to get an action name.
- `hyp --json init --agents ...` prints only the snapshot, not which skill files were installed.
- If agents::apply fails after Store::init, hyp/ exists and there is no hint to run `hyp agents install`.
- `remove` deletes a pre-existing empty .claude/ or .agents/skills/ (cannot tell it from one hyp created).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Pre-release versions compare correctly; a forced downgrade is reported as such
- [ ] #2 init --json lists installed skill files; a failed install after init prints the recovery command
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
More from the HYPO-0016 re-review: remove_empty_dirs with --force through a symlinked .claude also deletes the now-empty skills/hyp and skills dirs inside the link target (document or stop at the link); indirect_component treats any error (not only NotFound) as 'no symlink'; a regular file where a directory is expected gives 'cannot inspect ... Not a directory' instead of the clear blocked message.
<!-- SECTION:NOTES:END -->
