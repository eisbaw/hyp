---
name: hyp
description: Track suspected causes and uncertain claims as falsifiable hypotheses with the hyp CLI. Use when debugging, doing root-cause analysis, investigating why something fails or behaves unexpectedly, weighing competing explanations, or before declaring a cause, fix or claim confirmed.
---

# hyp: work with tentative claims

`hyp` records hypotheses, what would falsify them, cited evidence, experiments
and explicit judgments as Markdown files under `hyp/`, so that a suspected
cause stays a hypothesis until evidence decides it and a human can inspect how
you got there (with `hyp web`, a server: do not start it yourself). If there
is no `hyp/` directory in the project root, ask the user before `hyp init`.

## Method

1. **Look before adding.** `hyp list` and `hyp search "text"` show what is
   already recorded. Continue an existing hypothesis rather than adding a
   duplicate, and reuse existing evidence with `hyp link`.
2. **Record the hypothesis before acting on it.** When you suspect a cause,
   `hyp add` it with a scope before you change code or config because of it.
   Record serious alternatives too and link them with `competes-with`.
3. **Write the falsification criterion first.** `hyp falsify-if` states the
   observation that would prove the hypothesis wrong, before you gather
   evidence. Add `hyp predict` for what you expect to observe if it holds.
4. **Prefer the experiment that could falsify.** Choose the check whose
   outcome could contradict the hypothesis, not one that can only agree.
5. **Cite evidence with a source and a locator.** Every observation names
   where it came from (`--source` file, command or URL; `--locator` lines,
   test name, job, timestamp). Record only what you actually observed; never
   invent, extrapolate or paraphrase it into something stronger.
6. **Assess only with evidence and a reason** (see Assessing). "supported"
   never means proven. Do not delete or rewrite a hypothesis that turned out
   wrong; assess it. Record gaps (`hyp gap`) you could not close. Archive a
   duplicate or mistaken record (`hyp archive ID`, undone by `hyp restore`).

## Commands

IDs look like `H-<uuid>`; the letter gives the kind (H hypothesis, F criterion,
P prediction, E evidence, L link, X experiment, R run, A assessment, G gap).
Pass the full ID or an unambiguous prefix.

```bash
hyp list [--needs-review]      # hypotheses; --kind KIND or --all for others
hyp search "text"              # any kind, any field
hyp show H-...                 # summary for reading, with the review token
hyp --json show H-...          # .entry .state .basis .related .evidence .runs
hyp add "Title" --scope "where it applies" --tags a,b --body "Details"
hyp falsify-if H-... "Observation that would falsify it"
hyp predict H-... "Expected observation" --conditions "..."
hyp set H-... --lifecycle investigating                    # needs a criterion
hyp evidence add H-|P-|F-... "Short observation" --source PATH_OR_CMD \
  --locator "lines 10-20" [--against | --qualifies] --reason "Why it matters" \
  --body "Numbers, raw output, the exact command"          # prints E-, then L-
hyp evidence attach E-... ./capture.log                    # keep the raw file
hyp link E-... H-... --relation supports --reason "..."    # reuse evidence
hyp link H-... H-... --relation competes-with --reason "..."
hyp experiment add H-... "Short procedure" --targets F-...,P-... --body "..."
hyp run X-... "Run 1" --outcome observed --evidence E-...
hyp gap H-... "Open question"
hyp check                      # validate every file; see Files
```

`hyp <command> --help` documents every flag and value. Unknown flags or
values, and `hyp list` filters the listed kind cannot have, exit 2.

**Titles are one line and short**: the claim, or the observation in a few
words ("200/200 passes with per-test temp dirs"). Put details, numbers, raw
output and command lines in `--body`, and whole logs in
`hyp evidence attach`. A long title makes `hyp list` unreadable. A text
argument `-` reads stdin, at most one per command; for a title, the first
line of stdin is the title and the rest goes to the body.

**Write output.** A write prints the full ID of each record it wrote, one per
line, on stdout: capture it with `H1=$(hyp add ...)`, as in the example. With
`--json` it prints `{"written": [{"id", "kind", "revision"}], "revision"}`;
`.written[i].revision` is that record's `expected_revision` for a follow-up
`hyp apply` update, archive or delete, with no `show` in between. A human
summary goes to stderr only when stderr is a terminal; a write that changes
nothing exits 0 (`no changes` on stderr without `--json`).

**Batches.** `hyp apply` takes a JSON array of changes on stdin, all or
nothing, each stating what it depends on; `hyp apply --help` documents every
change type with examples.

## Assessing

```bash
hyp show H-...     # review claim, criteria, evidence, runs; note the token
hyp assess H-... --reviewed TOKEN --status weakened --confidence 0.3 \
  --evidence E-...,E-... --reason "Why these observations lead here"
```

- `--reviewed` takes the review token of the state you reviewed: the
  `review token:` line of `hyp show H-...` (`.state.review_token` with
  `--json`). Its first 12 or more hex digits suffice. Take the token from
  the output you actually reviewed, never from a second read.
- Cite only evidence linked to the hypothesis or its criteria or predictions.
  For other evidence, `hyp link E-... H-...` first, then review again.
- Every judgment except `untested` needs `--evidence`; `falsified` also
  needs `--criterion F-...`. `--reason` is always required; `--confidence`
  is 0.0 to 1.0.
- The token covers the basis (`.basis` in `hyp --json show`: the claim,
  scope and assumptions, criteria, predictions, links and linked evidence
  with source and locator, runs, and archiving any of them) and the current
  assessments. Closing, tags, experiment status and gap resolution do not
  change it. `.state.needs_review` is true when the basis changed after the
  current assessment: review and assess again. Every assessment changes the
  token, so read again before the next one.
- Exit 3 means the state changed since you read it; nothing was written.
  Run `hyp --json show H-...` again, compare `.basis` with what you reviewed,
  reconsider the judgment, then assess with the new token. Never copy a new
  token without reviewing what changed.

When the investigation is over, `hyp set H-... --lifecycle closed`; that
does not mean true, the assessment is the judgment. An untestable hypothesis
gets `--untestable-reason "..."` instead of a criterion.

## Exit codes

- `0` success.
- `1` the input or project is wrong (unknown or ambiguous ID, invalid value,
  unlinked or missing evidence, a write that would add a `hyp check` error,
  files hyp cannot load). Fix the cause; retrying unchanged will not help.
- `2` invalid command-line arguments: see `hyp <command> --help`.
- `3` conflict: something the write depended on changed since you read it.
  Nothing was written. Re-read (the message names what to re-read),
  reconsider, retry with fresh values.
- `141` stdout was closed early (`| head`). A write may be on disk: check
  before retrying, and never truncate the output of a write.

Errors go to stderr (`--json`: `{"error": "..."}`).

## Files

Never edit files under `hyp/` directly; use `hyp set ID` (`--title`,
`--body`, `--tags`, ...). `hyp edit` opens an editor and reopens it on an
error only at a terminal: leave it to people. After an editor, merge or sync
touched `hyp/`, run `hyp check`. Each diagnostic has a `code` and may carry
a repair (`hyp --json check`: `.repair.note`, and `.repair.commands` as argv
arrays to run in the project directory). Read the note first: a missing
record may still be arriving from a sync or merge, so prefer restoring it
over deleting what refers to it; a delete cannot be undone without version
control. Only `malformed`, `attachment` and `invalid` block writes
(`blocks_writes`): restore or fix those files by hand.

## Example

```bash
hyp search "login"                       # anything recorded already?
H1=$(hyp add "Flaky login test is caused by a shared temp dir" \
  --scope "tests/login.rs on CI" --tags ci,flaky)
F1=$(hyp falsify-if "$H1" "Test still fails with per-test temp dirs")
hyp set "$H1" --lifecycle investigating
H2=$(hyp add "Flaky login test is caused by clock skew" --scope "CI")
hyp link "$H1" "$H2" --relation competes-with --reason "Both explain it"
X1=$(hyp experiment add "$H1" "Rerun with per-test temp dirs" --targets "$F1" \
  --body "cargo test login -- --test-threads=8, 200 iterations")
E1=$(hyp evidence add "$H1" "200/200 passes with per-test temp dirs" \
  --source "cargo test login -- --test-threads=8" --locator "CI job 4411" \
  --reason "Isolation alone removed the failures" \
  --body "Before: 23/200 failed on CI job 4402. Clock not varied." \
  | sed -n 1p)                           # the E- line; the L- line follows
hyp run "$X1" "200 iterations" --outcome observed --evidence "$E1"
hyp set "$X1" --experiment-status completed
hyp link "$E1" "$H2" --relation contradicts \
  --reason "Clock unchanged, yet the failures stopped"
G1=$(hyp gap "$H1" "Which test leaves files behind?")
SHOW=$(hyp show "$H1"); printf '%s\n' "$SHOW"   # review it all before judging
TOKEN=$(printf '%s\n' "$SHOW" | sed -n 's/^review token: *//p')
hyp assess "$H1" --reviewed "$TOKEN" --status supported --confidence 0.7 \
  --evidence "$E1" --reason "Isolation removed all failures in 200 runs"
SHOW=$(hyp show "$H2"); printf '%s\n' "$SHOW"   # each judgment its own review
TOKEN=$(printf '%s\n' "$SHOW" | sed -n 's/^review token: *//p')
hyp assess "$H2" --reviewed "$TOKEN" --status weakened --confidence 0.3 \
  --evidence "$E1" --reason "Failures stopped with the clock unchanged"
hyp set "$G1" --resolved true
hyp set "$H1" --lifecycle closed         # the assessment still holds
hyp list --needs-review                  # empty: nothing to review again
```
