---
id: HYPO-0016
title: 'Agent onboarding: `hyp init` installs Claude/Codex skill'
status: To Do
assignee: []
created_date: '2026-09-29 22:23'
updated_date: '2026-09-29 22:31'
labels:
  - agents
  - cli
  - feature
  - mvp
dependencies:
  - HYPO-0007
  - HYPO-0002
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Motivation: the tool was conceived as hypothesis tracking "for humans and coding/research agents", and agents are the strongest use case. They propose root causes constantly and declare victory too early. Today hyp only offers `--json`, `hyp apply` and exit codes; nothing tells an agent when to use hyp or how to use it well. Much of backlog.md's adoption comes from `backlog init --agent-instructions claude,agents`, which writes a managed guidelines block into CLAUDE.md/AGENTS.md.

Proposal: `hyp init --agents claude,codex` (and `hyp agents install|update|remove` for existing projects) installs an onboarding skill/instructions, embedded in the binary and versioned with it:
- Claude Code: a project skill at `.claude/skills/hyp/SKILL.md` (frontmatter name/description that triggers on debugging/root-cause/investigation work), plus optionally a short pointer block in CLAUDE.md.
- Codex: a managed block in AGENTS.md and/or the Codex skills location. Verify Codex's current skill location and format before implementing; do not guess.

The skill should teach the method, not only the commands:
- Record a hypothesis before acting on a suspected cause.
- Write `falsify-if` before gathering evidence.
- Cite evidence with source and locator; never invent observations.
- Prefer the experiment that could falsify the hypothesis.
- Use `--json` and unambiguous ID prefixes; retry on exit code 3 (conflict) after re-reading.
- Never edit `hyp/` files directly; use `hyp check` after external edits.

Assessment policy (decided by the user): agents may record assessments themselves, including `falsified`, but every agent assessment must cite evidence (at least one evidence ID) and give a rationale. The tool currently requires evidence only for `falsified`, so for other judgments this is a rule the skill must teach. Whether the tool should also enforce it, for everyone or via a recorded actor/provenance field on assessments, is a follow-up to decide separately.

Per decision-0001, installing agent files must not require or assume Git.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `hyp init --agents claude,codex` installs the agent onboarding files; `--agents none` (or omitting the flag) installs nothing
- [ ] #2 `hyp agents install|update|remove` manages them in an existing project; content is embedded in the binary and carries a version marker
- [ ] #3 Idempotent: re-running does not duplicate content; managed blocks use start/end markers, and user content outside them is preserved; locally modified managed files are not overwritten without `--force`
- [ ] #4 Codex skill/instructions location verified against current Codex documentation, with the source cited in the task notes
- [ ] #5 Integration tests: fresh install, re-install, update after a version bump, remove; all in a plain directory without Git
- [ ] #6 Dogfood: a Claude Code session in a sample project, given only the skill, uses hyp correctly on a small debugging task (transcript summary in the task notes)
- [ ] #7 Skill content covers the method (hypothesis before action, falsify-if first, cited evidence, conflict retry, no direct file edits) and states the assessment policy: agents may record any assessment, but each must cite at least one evidence ID and a rationale
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Build bottom-up:
1. Write the skill as a file in the repo (the content an agent needs) and add `hyp skill` (or `hyp agents print`) to print it to stdout.
2. Dogfood with a real Claude Code session (AC on dogfooding) and fix the skill.
3. Only then add `hyp init --agents` and the install/update/remove commands with managed blocks.
Depends on HYPO-0007 (a reliable exit code 3 for 'retry on conflict') and HYPO-0002 (agents otherwise hit spurious conflicts).
<!-- SECTION:PLAN:END -->
