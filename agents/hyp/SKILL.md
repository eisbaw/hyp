---
name: hyp
description: Track suspected causes and uncertain claims as falsifiable hypotheses with the hyp CLI. Use when debugging, doing root-cause analysis, investigating why something fails or behaves unexpectedly, weighing competing explanations, or before declaring a cause, fix or claim confirmed.
---

# hyp: work with tentative claims

`hyp` records hypotheses, what would falsify them, cited evidence, experiments
and explicit judgments as Markdown files under `hyp/`. Use it so that a
suspected cause stays a hypothesis until evidence decides it, and so that a
human can inspect how you reached a conclusion (a human can run `hyp web`;
do not start it yourself, it is a long-running server).

If there is no `hyp/` directory in the project root, ask the user before
running `hyp init`.

## Method

1. **Record the hypothesis before acting on it.** When you suspect a cause
   ("the timeout is caused by X"), `hyp add` it with a scope before you change
   code or config because of it. Record serious alternatives too and link
   them with `competes-with`.
2. **Write the falsification criterion first.** `hyp falsify-if` states the
   observation that would prove the hypothesis wrong, before you gather
   evidence. Add `hyp predict` for what you expect to observe if it holds.
3. **Prefer the experiment that could falsify.** Choose the check whose
   outcome could contradict the hypothesis, not one that can only agree.
4. **Cite evidence with a source and a locator.** Every observation names
   where it came from (`--source` file, command or URL; `--locator` lines,
   test name, timestamp). Record only what you actually observed. Never
   invent, extrapolate or paraphrase an observation into something stronger.
5. **Assess only with evidence and a reason.** You may record any judgment
   (inconclusive, supported, weakened, falsified). The tool requires a
   rationale (`--reason`) and, for every judgment except `untested`, at least
   one evidence ID (`--evidence`) already linked to the hypothesis or its
   criteria or predictions; `falsified` also needs `--criterion`.
   "supported" never means proven.
6. **Keep the record honest.** Do not delete or rewrite a hypothesis because
   it turned out wrong; assess it. Record gaps (`hyp gap`) you could not close.

## Commands

IDs look like `H-<uuid>`. Use the full ID or an unambiguous prefix. Write
commands print the IDs they write, one per line (`evidence add` prints the
evidence ID, then the link ID); with `--json`,
`{"written": [{"id", "kind", "revision"}], "revision"}`. Use `--json` for
`show` and `list`.
`hyp list` lists hypotheses only; add `--kind KIND` or `--all` for others.

```bash
hyp add "Title" --scope "where it applies" --tags a,b   # -> H-...
hyp falsify-if H-... "Observation that would falsify it"   # -> F-...
hyp predict H-... "Expected observation" --conditions "..."
hyp set H-... --lifecycle investigating                    # needs a criterion
hyp evidence add H-... "What was observed" --source PATH_OR_CMD \
  --locator "lines 10-20" [--against | --qualifies] --reason "Why it matters"
hyp link E-... H-... --relation supports --reason "..."    # reuse evidence
hyp link H-... H-... --relation competes-with --reason "..."
hyp experiment add H-... "Procedure" --targets F-...,P-...
hyp run X-... "Run 1" --outcome observed --evidence E-...
hyp gap H-... "Open question"
hyp --json show H-...          # .entry .state .related .runs .evidence .basis
hyp --json list [--needs-review]   # hypotheses, each with .state
hyp search "text"
hyp check                      # validate all files
```

Text arguments accept `-` to read stdin.

## Assessing

`hyp assess` requires `--reviewed` with the `review_token` of the state you
reviewed. `hyp --json show H-...` gives what the token covers:

- `.basis`: the content the fingerprint hashes, by record ID (see `.basis`
  itself for which records and fields). The token also covers the current
  assessments (`.state.assessment_ids`).
- `.evidence` and `.runs`: those observations and runs in full, with source
  and locator. `.related`: records that refer to the hypothesis. Read these
  before judging.

Cite only evidence that an active link connects to the hypothesis (or its
active criteria or predictions). For other evidence, `hyp link E-... H-...`
first, then re-read and review before assessing; in a `hyp apply` batch, link
pre-existing evidence in an earlier write.

```bash
hyp --json show H-...          # review it; note .state.review_token
hyp assess H-... --reviewed <review_token> --status weakened \
  --confidence 0.3 --evidence E-... --reason "Why these observations lead here"
```

Read the hypothesis again before each assessment, including right after your
own previous one: every assessment changes the token. `.state.needs_review`
is true when the basis changed after the current assessment; review and
assess again. Archiving the hypothesis changes the basis; closing,
retagging, experiment status and gap resolution do not.

## Finishing

```bash
hyp set G-... --resolved true                   # a gap you closed
hyp evidence attach E-... ./capture.log         # keep the raw observation
hyp set H-... --lifecycle closed                # investigation over
```

Closing records that you stopped investigating; it does not mean the
hypothesis is true. The judgment is the assessment. A hypothesis that cannot
be tested gets `hyp set H-... --untestable-reason "..."` instead of a
criterion before it can be `investigating`.

## Exit codes and errors

- `0` success.
- `1` the input is wrong (unknown or ambiguous ID, invalid value, a judgment
  without evidence, unlinked evidence, malformed token), the write would add a
  project error, or the project has files hyp cannot load (`hyp check` names
  them). Fix that; retrying unchanged will not help.
- `2` invalid command-line arguments (see `hyp <command> --help`).
- `3` conflict: something the write depended on changed since you read it.
  Nothing was written. Re-read: look at the IDs the message names (for
  `assess`, re-read `hyp --json show H-...`), reconsider your judgment, then
  retry with freshly read values. Never copy a new token without reviewing what changed: that
  defeats the check.
- `141` stdout was closed early (e.g. `| head`). A write command prints after
  it has written, so the write may already be on disk: check with
  `hyp list` before retrying. Do not truncate the output of write commands.

Errors go to stderr; with `--json` as `{"error": "..."}`. Conflict messages
start with `conflict:`.

## Files

Never edit files under `hyp/` directly; use the commands (`hyp set ID` with
`--title`, `--body`, `--tags` or `--lifecycle` changes a record). If files
were changed outside hyp (an editor, a merge or a sync), run `hyp check` and
fix what it reports before writing. A diagnostic may carry a `note:` and
`repair:` commands (`hyp --json check`: `.repair.note`, `.repair.commands` as
argv arrays, run in the project directory). Read the note first. A missing
record may still be arriving from a sync or merge: prefer restoring it over
deleting what references it, because a delete cannot be undone without
version control. A malformed or invalid file must be restored or fixed by
hand. For batches, `hyp apply` takes a JSON
array of changes on stdin; each states what it depends on. `hyp apply --help`
shows the change formats with examples, including an assessment.

## Example

```bash
hyp add "Flaky login test is caused by a shared temp dir" \
  --scope "tests/login.rs on CI" --tags ci,flaky            # H-3f2a...
hyp falsify-if H-3f2a "Test still fails with a per-test temp dir"
hyp add "Flaky login test is caused by clock skew" --scope "CI"   # H-9c1d...
hyp link H-3f2a H-9c1d --relation competes-with --reason "Both explain it"
# Run the falsifying experiment: per-test temp dirs, 200 iterations.
hyp evidence add H-3f2a "200/200 passes with per-test temp dir" \
  --source "cargo test login -- --test-threads=8" \
  --locator "CI job 4411, iterations 1-200" \
  --reason "Isolation removed the failures; skew was not varied"
hyp --json show H-3f2a | jq -r .state.review_token
hyp assess H-3f2a --reviewed <token> --status supported --confidence 0.7 \
  --evidence E-7b10 --reason "Isolation removed all failures in 200 runs"
```
