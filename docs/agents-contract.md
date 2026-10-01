# Agents: the skill and the machine contract

Agents are the primary users, so the CLI's `--json` output, exit codes and error kinds are a stable contract ([decision-0002](../backlog/decisions/decision-0002%20-%20hyp-is-primarily-a-tool-for-agents-to-work-in-a-structured-way-with-tentative-unconfirmed-information-humans-inspect-and-steer.md)).

## Agent onboarding

hyp ships a skill that teaches coding agents the method (look for existing hypotheses first, record the hypothesis before acting on it, write the falsification criterion first, cite evidence, assess only with evidence and a rationale) and how to use the tool: short titles with details in the body, reading the IDs writes print (no shell variables needed), batches with `hyp apply`, reviewing and assessing, exit codes, conflicts and `hyp check` repairs. Its commands use no shell variables, which agent sandboxes often block; a test checks that. The method it teaches also exists as a script, `tests/fixtures/skill_flow.sh`, and its `hyp apply` batch example as JSON; tests run both against the built binary, so they cannot fall behind the CLI unnoticed (the prose, the Example and the Commands block are reviewed by hand):

```bash
hyp init --agents claude,codex   # new project: also install the skill
hyp agents install               # existing project (default: --agents claude,codex)
hyp agents update                # refresh installed skills after upgrading hyp
hyp agents remove                # delete what hyp installed
hyp agents print                 # the skill on stdout, to read or paste elsewhere
```

| Agent       | Installed as                  | Read by                                                                   |
| ----------- | ----------------------------- | ------------------------------------------------------------------------- |
| Claude Code | `.claude/skills/hyp/SKILL.md` | Claude Code started in the project                                        |
| Codex       | `.agents/skills/hyp/SKILL.md` | Codex started in the project root (or below it, inside a Git repository) |

The skill is embedded in the executable. An installed file carries a marker line, `# hyp-managed: version=… sha256=…`, in its front matter. Installing again changes nothing when the file is current; `update` replaces a file from another hyp version. A skill file you edited, one hyp did not install, one installed by a newer hyp, or one whose directory path goes through a symlink (e.g. a `.claude` managed by a dotfile tool) is left alone and the command fails, writing nothing, unless you pass `--force`. `remove` deletes only files hyp installed, never one without the marker, plus the directories on their path that end up empty (hyp does not record which of them it created). Nothing else, such as `CLAUDE.md` or `AGENTS.md`, is touched.

## Machine contract

Agents depend on these; they are kept stable.

| CLI exit code | Meaning                                                                                                                                             |
| ------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| `0`           | Success.                                                                                                                                            |
| `1`           | Any other error: invalid input, an unknown or ambiguous ID, a missing or malformed precondition, project errors, I/O (`kind` tells them apart). Retrying unchanged will not help. |
| `2`           | Invalid command-line arguments.                                                                                                                     |
| `3`           | Conflict: something the write depended on changed since it was read. Nothing was written. Re-read, review, retry.                                  |
| `141`         | Killed by SIGPIPE: stdout was closed early (`hyp list \| head`). A write command prints after writing, so its write may already be on disk.           |

Write commands (`add`, `evidence add`, `assess`, `set`, `apply`, `capture`, `evidence attach` and the rest) print the full ID of each record their changes named, one per line and in order (`evidence add`: the evidence, then its link; `add`: the hypothesis, then one link per `--explains`, then one per `--competes-with`; `observe`: the evidence; `capture`: the data record; `evidence attach`: the evidence, then the data record, since 0.3.0). With `--json` they print `{"written": [{"id": "H-…", "kind": "hypothesis", "revision": "…"}], "revision": "…"}`: each record's revision after the write (`null` once deleted), usable as the `expected_revision` of a next change, and the project revision (what `apply --expected-revision` takes). `hyp --json init` prints the same shape, listing every record of the new project. A write that converted legacy attachments also has `"migrated"` (see [Schema versions](storage.md#schema-versions)). In `hyp apply` output, an entry whose create gave a batch-local reference also has `"ref": "@name"`. Stdout is the same when a change turned out to change nothing (the revision stays as it was). Stderr carries a human summary only when it is a terminal, `no changes` only without `--json`.

Plain output is for people and may change, except the first line of `hyp show ID`: the full ID and the kind separated by two spaces, then for a hypothesis two spaces, `review` and the first 12 hex digits of its review token (`H-…  hypothesis  review 46d8d8f5c79b`), for any other record two spaces, `revision` and the first 12 hex digits of its revision (`X-…  experiment  revision 0f3ac2d19e7b`; hyp 0.3.0 printed only ID and kind). These are what `--reviewed` takes (see [Preconditions](#preconditions)). Scripts may read it; the skill's flow script does. Everything else is in `--json`.

Errors go to stderr, with `--json` as `{"error": "…", "kind": "…"}`. `kind` is decided by the error's type where it arises, not by its text, and is one of:

| `kind`          | Meaning                                                                                              |
| --------------- | ---------------------------------------------------------------------------------------------------- |
| `conflict`      | Exit `3`. Something the write depended on changed. `ids` lists the records whose statement failed, when known (not for `--expected-revision`). |
| `invalid_input` | Input the rules reject: a malformed value or batch, a missing statement, a write that would add a `hyp check` error. For the last, the message names each new error's path and code, and `diagnostics` lists them (with `repair` null: nothing was stored to repair). |
| `not_found`     | A named record (or the project) does not exist, including one a change states that was deleted since it was read (see [Preconditions](#preconditions)). `ids` lists the given IDs that matched nothing, when known. |
| `ambiguous_id`  | An ID prefix matches more than one record; the message lists them.                                   |
| `blocked`       | A file that `hyp check` reports as blocking (`blocks_writes`) must be repaired before any write; `diagnostics` lists the blocking ones. An invalid record can be repaired through hyp (see [Files and concurrency](storage.md#files-and-concurrency)); a repair that leaves it invalid is `blocked` too, its `diagnostics` naming what is still wrong. |
| `check_failed`  | `hyp check` found errors (with `--strict`, also warnings); stdout lists them. hyp 0.3.0 gave `invalid_input` ("validation failed"). |
| `unsupported_schema` | The notebook uses a newer schema than this hyp reads; the message names the hyp version to upgrade to. Nothing is read or written. See [Schema versions](storage.md#schema-versions). |
| `io`            | Reading or writing a file failed.                                                                    |

`diagnostics` has the form `hyp --json check` prints: `[{"path", "code", "message", "severity", "blocks_writes", "repair"}]`, where `(path, code)` identifies each (see [Files and concurrency](storage.md#files-and-concurrency)). In the error of a rejected write, or of one blocked by invalid records, a diagnostic about a record one of the write's changes names also has `"change"`, that change's index (from 0, in the order given; a CLI command's changes are in the order it prints their IDs; when several changes name one record, the last), and `"ref"` when the change gave a batch-local reference: a record the write would create has a path that names no file yet. (A write blocked by a malformed file or missing bytes is refused before its changes are read, so its diagnostics have no `change`.) `severity` and `blocks_writes` always describe the code as `hyp check` reports it, not the write: a refused `observed_at` comes as `bad_observed_at`, a warning that does not block writes, although this write was refused. Decide by `kind` and `code`.

The set of kinds may grow: treat a kind you do not know by the exit code. A conflict's message starts with `conflict:`. Argument errors (exit `2`) are clap's usage text, not JSON. The WebUI's HTTP API answers errors with the same JSON body, and `403` for a missing or invalid request token or an untrusted `Host`/`Origin`, `409` for a conflict, `422` for input the domain rules reject, and another `4xx` for other rejected input (malformed JSON, an oversized body, a wrong content type).

## Preconditions

Every change states what it depends on, as read from a snapshot. `hyp --json show ID` gives a record's `entry.revision` and the records that refer to it (`related`); for a hypothesis also its `state` and `basis`, what its fingerprint covers (see [The model](model.md#the-model)), with the `runs` and `evidence` in it in full; for evidence the hypotheses it `bears_on` and whether it is `unexplained`. `hyp export --format json` gives everything. A change that depended on something that changed is rejected; a change that did not is unaffected by other writers.

- update, archive, delete: `expected_revision`, the record's revision. An update or archive that changes nothing is not written.
- create an assessment: `expected.hypotheses[H].review_token`, the `state.review_token` of its hypothesis as read. The token covers the fingerprint and the current assessments (their IDs and revisions).
- create an experiment or run: `expected.revisions`, full ID to revision, for each experiment target (the hypothesis must be one of the targets), or for a run's experiment and the evidence it cites.
- `based_on`, `supersedes`, a target's `revision`, `title` and `body`, and a run's `plan` are set by the server; a create that fills them in is rejected. Targets are given as `{"id": "P-…"}`.

```json
[{"op": "create",
  "record": {"kind": "assessment", "title": "Weakened", "body": "Why …",
             "hypothesis": "H-…", "judgment": "weakened", "evidence": ["E-…"]},
  "expected": {"hypotheses": {"H-…": {"review_token": "…"}}}}]
```

A statement that no longer holds (the record changed) is a conflict: exit `3`, HTTP 409, and the message names what changed. Re-read, review what changed, then retry. A stated ID that matches no record (of an update, patch, archive or delete, or a key of `expected`) is kind `not_found` (exit `1`, HTTP 422), whether it was mistyped or the record was deleted since it was read: without tombstones hyp cannot tell the two apart, and retrying either cannot succeed. Re-read; if the record is gone, drop the change. The same holds for an ID in a create's reference fields (hypothesis, from, to, experiment, evidence, criterion, targets, resolved_by, data) that matches no record before or within the batch, as it does for that ID given to the CLI. Naming a record an earlier change of the same batch deleted is an ordinary error. A missing or malformed statement is an ordinary error (exit `1`, HTTP 422) naming the field; retrying it unchanged will not help. Records created earlier in the same batch need no statement (one given for them is not checked), so a create that depends only on them may omit `expected`.

An assessment may cite only evidence with an active link to its hypothesis, or to one of its active criteria or predictions (`hyp link E-… H-… --relation supports --reason "…"`). Citing other evidence, a judgment other than `untested` without evidence, or `falsified` without cited evidence that meets its criterion ([The model](model.md#the-model)), is an ordinary error. In an `apply` batch, the assessment's basis may grow only by records the batch creates: to bring in evidence that already existed, link it in an earlier write, re-read, then assess.

The CLI commands state preconditions from their own read, so they protect only the moment between that read and the write. `hyp assess` is the exception: it requires `--reviewed` with the review token of the state you reviewed: the 12 hex digits after `review` on the first line of `hyp show H-…`, or `.state.review_token` of `hyp --json show H-…` or `hyp --json list` (its first 12 or more hex digits suffice). If the hypothesis's basis or its current assessments changed since, it writes nothing and exits `3`; `hyp show H-…` then lists the basis to compare with what you reviewed. A malformed token, or a `--confidence` outside 0.0 to 1.0, exits `1`. `hyp apply` takes only the full token.

The commands that freeze content take the same statement, optionally: `hyp experiment add H-… --reviewed TOKEN`, the hypothesis's review token as for `assess`. The token covers the claim, criteria and predictions the experiment freezes as its targets (not their lifecycle or tags), but just as much the rest of the basis and the current assessments: new or changed linked evidence, links, runs or an assessment since your review also exit `3`, though the experiment would freeze nothing different; re-read and retry. (Precise per-target statements are `expected.revisions` in `hyp apply`, which compare raw revisions, so a lifecycle or tag change does conflict there.) And `hyp run X-… --reviewed REVISION`, the experiment's revision as line 1 of `hyp show X-…` prints it (12 or more hex digits of `.entry.revision`), so the run freezes the plan you executed. A change since then exits `3` and writes nothing; any change to the experiment counts, its status too, so review it after setting it running. Without `--reviewed` they freeze what their own read finds. They are optional because, unlike a judgment, the frozen content is kept in the record itself: what the experiment or run rests on is never lost, only possibly newer than what you saw. A run's cited evidence is named, not frozen, so `--reviewed` does not cover it. To protect a longer window for other records, use `hyp apply`. The WebUI states everything as of the moment a form was opened.
