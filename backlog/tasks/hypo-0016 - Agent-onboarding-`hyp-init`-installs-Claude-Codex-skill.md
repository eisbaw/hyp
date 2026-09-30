---
id: HYPO-0016
title: 'Agent onboarding: `hyp init` installs Claude/Codex skill'
status: To Do
assignee: []
created_date: '2026-09-29 22:23'
updated_date: '2026-09-30 00:44'
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
- [ ] #8 The machine contract is documented in the README and the skill: exit codes 0/1/2/3, 141 (output pipe closed; a write may already be on disk), HTTP 403/409/422, and that conflict messages start with 'conflict:'
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Build bottom-up:
1. Write the skill as a file in the repo (the content an agent needs) and add `hyp skill` (or `hyp agents print`) to print it to stdout.
2. Dogfood with a real Claude Code session (AC on dogfooding) and fix the skill.
3. Only then add `hyp init --agents` and the install/update/remove commands with managed blocks.
Depends on HYPO-0007 (a reliable exit code 3 for 'retry on conflict') and HYPO-0002 (agents otherwise hit spurious conflicts).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Forward-carried from HYPO-0007 and HYPO-0018, for the agent skill:
- Exit codes: 0 success; 1 any other error; 3 conflict (store::Conflict in the error chain: re-read with 'hyp --json list/show' and retry). Errors go to stderr; with --json as {"error": "..."}, and a conflict message starts with 'conflict:'. There is no machine-readable error kind field yet; add one if the skill needs more than the exit code.
- Closed stdout: every command except 'hyp web' now dies quietly from SIGPIPE (shell status 141) when its stdout is closed early (hyp list | head). A write command prints after committing, so 'hyp add ... | head -c0' reports 141 although the record was written; under 'set -o pipefail' that looks like a failure. Tell agents not to truncate the output of write commands, or to check with 'hyp list' afterwards.
- WebUI API: 403 for a missing/invalid x-hyp-token, 409 for conflicts, 422 for other rejected input.

Forward-carried from HYPO-0002 (write preconditions; the README section after the exit codes is the reference), for the agent skill:
- Unrelated concurrent writes do not cause exit 3; only changes to what a write depends on do.
- hyp assess REQUIRES --reviewed <token>: .state.review_token of `hyp --json show H-...` (also in each hypothesis row of `hyp --json list`), taken when the agent reviewed the hypothesis. It is a hash of the fingerprint and the current assessment IDs. If the hypothesis's records (evidence, links, criteria, predictions, experiments, runs) or its current assessments changed since, the command writes nothing and exits 3. A malformed token (not 64 hex digits) exits 1; a missing flag exits 2. The revisions of the evidence it cites come from the command's own read (a small window).
- The other CLI commands (set, edit, archive, experiment add, run, ...) state preconditions from their own read, so they protect only their read-to-write moment. For a longer window use hyp apply.
- hyp apply: update/archive/delete carry expected_revision (entry.revision from `hyp --json show ID`; empty is an error). Creating an assessment, experiment or run needs an "expected" object next to "record" (all keys full IDs, no prefixes):
  - assessment: expected.hypotheses[H] = {"fingerprint": state.fingerprint, "assessment_ids": state.assessment_ids} ([] for none; the store contract uses these two fields, not the review token). expected.revisions: for each cited evidence E, state E, every link in .related of `hyp --json show E`, and both ends of each such link. Stating records that are already covered is harmless.
  - experiment: record.targets = [{"id": H}, {"id": P}, ...] (must include the hypothesis); expected.revisions gives each target's revision.
  - run: omit record.plan; expected.revisions gives the experiment's revision and the revision of every evidence ID the run cites.
  - based_on, supersedes, target revision/title/body and plan are server-set: a create that fills them in is rejected (exit 1).
  - Records created earlier in the same batch need no statement. A batch that links existing evidence into the hypothesis (or creates a run citing existing evidence) before assessing must also state what that brings in; the conflict names the IDs. Put an assessment last in a batch, or later records in the batch flag it needs_review at once.
  - --expected-revision (whole project) is optional and makes any concurrent write a conflict; do not use it for ordinary work.
- Example: [{"op":"create","record":{"kind":"assessment","title":"Weakened","body":"Why","hypothesis":"H-...","judgment":"weakened","evidence":["E-..."]},"expected":{"hypotheses":{"H-...":{"fingerprint":"...","assessment_ids":["A-..."]}},"revisions":{"E-...":"...","L-...":"...","H-other...":"..."}}}]
- Exit 3 lists the IDs that changed, do not exist (deleted since the read, or never existed), or were not stated (e.g. a link added since the read). Re-read, look at those records, reconsider the judgment, then retry with freshly read values. Do not copy a new token or fingerprint without reviewing: that defeats the check. Messages deliberately omit current values.
- Exit 1 naming expected, expected.hypotheses, assessment_ids, fingerprint, expected.revisions, "use the full ID", "set by the server", "must include its hypothesis" or "64-hex-digit": the input is incomplete or malformed; fix it, do not retry it as is.
<!-- SECTION:NOTES:END -->
