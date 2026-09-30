---
id: HYPO-0016
title: 'Agent onboarding: `hyp init` installs Claude/Codex skill'
status: In Progress
assignee:
  - '@claude'
created_date: '2026-09-29 22:23'
updated_date: '2026-09-30 02:19'
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
- [x] #1 `hyp init --agents claude,codex` installs the agent onboarding files; `--agents none` (or omitting the flag) installs nothing
- [x] #2 `hyp agents install|update|remove` manages them in an existing project; content is embedded in the binary and carries a version marker
- [x] #3 Idempotent: re-running does not duplicate content; managed blocks use start/end markers, and user content outside them is preserved; locally modified managed files are not overwritten without `--force`
- [x] #4 Codex skill/instructions location verified against current Codex documentation, with the source cited in the task notes
- [x] #5 Integration tests: fresh install, re-install, update after a version bump, remove; all in a plain directory without Git
- [ ] #6 Dogfood: a Claude Code session in a sample project, given only the skill, uses hyp correctly on a small debugging task (transcript summary in the task notes)
- [x] #7 Skill content covers the method (hypothesis before action, falsify-if first, cited evidence, conflict retry, no direct file edits) and states the assessment policy: agents may record any assessment, but each must cite at least one evidence ID and a rationale
- [x] #8 The machine contract is documented in the README and the skill: exit codes 0/1/2/3, 141 (output pipe closed; a write may already be on disk), HTTP 403/409/422, and that conflict messages start with 'conflict:'
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Build bottom-up:
1. Write the skill as a file in the repo (the content an agent needs) and add `hyp skill` (or `hyp agents print`) to print it to stdout.
2. Dogfood with a real Claude Code session (AC on dogfooding) and fix the skill.
3. Only then add `hyp init --agents` and the install/update/remove commands with managed blocks.
Depends on HYPO-0007 (a reliable exit code 3 for 'retry on conflict') and HYPO-0002 (agents otherwise hit spurious conflicts).

Implementer (2026-09-30):
a) agents/hyp/SKILL.md, embedded with include_str!.
b) A print command for it.
c) install/update/remove for Claude (.claude/skills/hyp/SKILL.md) and Codex (location verified against Codex docs first), with a version + content-hash marker; locally modified files are not overwritten without --force; remove deletes only what hyp installed; no Git.
d) Machine contract (exit codes, HTTP statuses, 'conflict:' prefix) in README and skill.
Step 2 (dogfood) is run by the orchestrator after this change.
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

Codex mechanism verified (2026-09-30), AC #4:
- Docs: https://developers.openai.com/codex/skills (redirects to https://learn.chatgpt.com/docs/build-skills): repository-scoped skills are read from $CWD/.agents/skills, $CWD/../.agents/skills (parent, in Git repos) and $REPO_ROOT/.agents/skills; user-scoped from $HOME/.agents/skills; 'The SKILL.md file must include name and description' (YAML frontmatter). Same format as a Claude Code skill.
- Local check with codex-cli 0.158.0 (offline, no model call): a plain directory (no Git) with .agents/skills/hypprobe/SKILL.md; `codex debug prompt-input` run in that directory lists 'hypprobe: <description> (file: r3/hypprobe/SKILL.md)'. Run from a subdirectory of that plain directory, the skill is NOT listed: without Git, Codex does not walk up to parents. Gotcha for users: start Codex in the project root (the directory holding hyp/) unless it is a Git repo.
- Decision: install the same SKILL.md at .agents/skills/hyp/SKILL.md for Codex; no AGENTS.md block needed.

Implementation (2026-09-30, uncommitted; orchestrator reviews and commits):
- Skill text: agents/hyp/SKILL.md (embedded with include_str! in src/agents.rs). Frontmatter has only name and description (Codex's validator and Claude Code both accept that). Covers the method, the assessment policy (any judgment, but at least one evidence ID and a rationale), --reviewed tokens, exit codes 0/1/2/3/141, HTTP 403/409/422, the 'conflict:' prefix, full IDs or unambiguous prefixes, no direct edits, hyp check after external changes, and a worked example. Every command in the skill was run against the real binary in a scratch project.
- Commands: `hyp agents print|install|update|remove [--agents claude,codex] [--force]` and `hyp init --agents claude,codex|none`. One `agents` noun groups all onboarding (print is the read-only member) instead of a separate top-level `hyp skill`. print needs no project; install/update/remove act on the discovered project root. install defaults to both agents; update refreshes only files already installed; remove deletes only hyp's files.
- Managed marker: one YAML comment line, last in the front matter: `# hyp-managed: version=<hyp version> sha256=<sha256 of the file without that line>`. Line endings are normalized first. A file without the marker is not hyp's. Every operation plans first (read-only) and fails naming all blocked files, writing nothing, if a file was modified, is not hyp's, or was installed by a newer hyp, unless --force. remove never deletes a file without the marker, even with --force; it deletes directories on the skill path that it leaves empty. `init --agents` plans before creating hyp/, so a blocked skill fails init before anything is created.
- AC #3 deviation: the files are whole-file managed (dedicated skill directories), so there is no start/end block inside a shared file; the in-file marker plays that role. CLAUDE.md, AGENTS.md and other skills are never touched (tested). No pointer block in CLAUDE.md/AGENTS.md was added: both agents discover the skill by its description.
- Tests (tests/cli.rs, real binary, plain temp dirs without Git): agents_print_writes_the_embedded_skill_without_a_project, init_with_agents_installs_the_skill_once (incl. install/update byte- and mtime-identical, CRLF not an edit, init/--agents none install nothing), agents_leave_other_files_alone_and_keep_local_edits_without_force, agents_update_replaces_an_older_skill_but_not_a_newer_one. Plus two unit tests in src/agents.rs. Mutation check: each of 10 mutations (no modified-file protection, rewrite when current, foreign removable with --force, no downgrade check, println instead of print, init ignores --agents, update installs missing, no directory cleanup, no CRLF normalization, hash ignored) made exactly one of these tests fail.
- AC #6 (dogfood) NOT done here: left for the orchestrator's Claude Code test-drive.
Gotchas:
- Codex without Git does not look in parent directories: start Codex in the project root. (With Git it finds $REPO_ROOT/.agents/skills from a subdirectory; verified with codex 0.158 `codex debug prompt-input`.)
- `hyp init --json --agents ...` still prints only the snapshot (stable init contract); the installed files are not listed in JSON mode.
- Claude Code discovery from a subdirectory was not verified; the dogfood run should start Claude Code in the project root.
- The skill leaves `hyp apply`'s expected object to the README, which is not installed: filed HYPO-0027.

Review fixes (2026-09-30, still uncommitted):
- Symlinks: plan refuses (naming the component, writing nothing) when an existing directory on the skill path (.claude, .claude/skills, .agents, .agents/skills, the hyp skill dir) is a symlink or not a directory and the action would write or delete; --force writes through it. The cleanup climb after remove stops at the first ancestor that is not a real directory, so a symlinked .claude/skills stays and remove no longer fails half-way. hyp does not record which directories it created: remove deletes the directories on the path that end up empty (documented in README and the code).
- Blocked-file messages carry their own hint; a file hyp did not install gets no --force hint on remove.
- The marker is recognized only inside the front matter.
- init --agents is parsed by a clap value parser (none alone, or claude/codex comma-separated): a bad list exits 2 before anything is created. The InitAgents enum is gone.
- Skill files are written 0644 less the umask (store::atomic_readable); records under hyp/ are still 0600: filed HYPO-0030.
- `hyp --json show ID` gained an additive "evidence" key: the evidence records that the related records link or cite (source, locator, observation), so the assess flow shows what a judgment rests on. Limit: evidence cited only by a run of an experiment (not linked to the hypothesis) is not included, because the run refers to the experiment, not the hypothesis.
- Skill: write commands are read as plain output (--json on a write prints the whole project); correct statement of what the tool enforces (evidence only for falsified, plus --criterion); .related/.evidence explained; ask before hyp init; do not start hyp web; a Finishing section (gap resolved, evidence attach, lifecycle closed does not mean true, --untestable-reason); exit-3 re-read wording.
- README: HTTP 403 also for untrusted Host/Origin, 422 for domain rejections, other 4xx for malformed/oversized/wrong-type bodies; softer intro sentence; show's related/evidence keys.
- New tests: agents_do_not_reach_through_a_symlinked_skills_directory, show_includes_the_evidence_a_hypothesis_links; init test now checks file mode and exit 2 for --agents none,claude; foreign-remove hint check; unit test for a marker outside the front matter. Each new check was shown to fail under a targeted mutation (symlink check off, climb ignores symlinks, show without evidence, private file mode, none mixed accepted, force hint on foreign remove, marker searched anywhere).

Re-review fixes (2026-09-30, still uncommitted):
- `hyp --json show H` now exposes everything the review token covers, for hypotheses (null for other kinds): `runs` (runs of its experiments, full records; chosen as a separate key so `related` keeps meaning 'records that refer to this one'), `evidence` (every Evidence record in Snapshot::relevant(H), the token's own source; replaces last round's related-based filter), and `basis` (map of every covered record ID to its revision, i.e. the fingerprint's input). `basis` was added beyond the request because relevant(H) also covers links touching cited evidence and their far ends (e.g. a competing hypothesis): renaming the competing hypothesis changes H's token, and without `basis` show H would not reveal it. Test show_reveals_every_change_the_review_token_covers (red before: identical show before/after a run; green after). Assessment IDs are covered by state.assessment_ids, not basis.
- hyp check no longer warns 'no active falsification criterion' for a hypothesis with a non-empty untestable_reason (test check_accepts_an_untestable_reason_instead_of_a_criterion, red before).
- Wording: remove through a symlink says '--force removes through it'; '--agents none,claude' says 'none cannot be combined with other agents' (exit 2); a blocked init --agents points to 'hyp agents install --force'.
- The WebUI HTTP codes line was cut from SKILL.md at the orchestrator's request (agents do not run hyp web); the README keeps it. AC #8 ('documented in the README and the skill') is thus met for HTTP codes in the README only, by decision.

Committed the installer and skill after QA GO and architect GO (three review rounds). AC #6 (dogfood with a real Claude Code session) is still open; the orchestrator runs it next.
<!-- SECTION:NOTES:END -->
