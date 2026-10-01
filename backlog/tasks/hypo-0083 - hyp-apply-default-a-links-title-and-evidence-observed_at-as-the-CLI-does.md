---
id: HYPO-0083
title: 'hyp apply: default a link''s title and evidence observed_at as the CLI does'
status: To Do
assignee: []
created_date: '2026-09-30 17:53'
updated_date: '2026-10-01 19:00'
labels:
  - agents
  - cli
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while implementing HYPO-0053. apply creates now default lifecycle, experiment status, run outcome and experiment targets, but a link still needs a title the CLI generates ('Evidence for H-…' / '<from> <relation> <to>'), and evidence observed_at stays empty where the CLI sets the current time. Decide whether apply should fill them (observed_at 'now' may overstate what the agent knows).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A link create without a title gets the CLI's generated title, or the decision not to is documented
- [ ] #2 observed_at on an apply evidence create is either defaulted like the CLI or documented as intentionally empty
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Wider follow-up from the 0.4.0 merge review (2026-10-01): the WebUI duplicates the CLI's "create a hypothesis that explains an observation" (hyp add --explains): web/app.js has its own EXPLAINS_REASON (copied from cli.rs Command::Add), builds the supports link itself (newLink) and repeats the CLI's link title. One server-side "create with explains" operation (or apply defaults for a link's title and reason), used by both the CLI and the WebUI, would remove the copies. If this task fills a link create's title, it should take the reason default too, so the WebUI can drop both. Related: HYPO-0035 (what the generated titles say).
<!-- SECTION:NOTES:END -->
