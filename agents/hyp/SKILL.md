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

1. **Start with `hyp status`.** It shows where the investigation stands:
   per open hypothesis its judgment, whether it needs review, a missing
   criterion, linked evidence, open gaps and experiments without runs, and
   anything that blocks writes. `hyp search "text"` finds what is already
   recorded. Continue an existing hypothesis rather than adding a duplicate,
   and reuse existing evidence with `hyp link`.
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
   wrong; assess it. Record gaps (`hyp gap`) you could not close, and cite
   the evidence that later answers one (`--by`). Archive a duplicate or
   mistaken record (`hyp archive ID`, undone by `hyp restore`).

## Commands

IDs look like `H-<uuid>`; the letter gives the kind (H hypothesis, F criterion,
P prediction, E evidence, L link, X experiment, R run, A assessment, G gap).
Pass the full ID or an unambiguous prefix: the first 10 characters
(`H-1a2b3c4d`) nearly always are.

```bash
hyp status                     # start here: where each hypothesis stands
hyp list [--needs-review]      # hypotheses; --kind KIND or --all for others
hyp search "text"              # any kind, any field
hyp show H-...                 # summary for reading; line 1 has the review token
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
hyp set G-... --resolved true --by E-...,E-...             # what answered it
hyp check                      # validate every file; see Files
```

`hyp <command> --help` documents every flag and value. Unknown flags or
values, and `hyp list` filters the listed kind cannot have, exit 2.

**Evidence on a criterion or prediction counts.** Evidence linked to an
active criterion or prediction is part of its hypothesis's basis and
citable in its assessments; do not add a second link to the hypothesis. On
a criterion, `hyp evidence add F-...` (supports) records that the refuting
observation was made, which counts against the hypothesis; `--against`,
that it was not. `hyp show H-...` groups each observation by what it means
for the hypothesis, with every link (`--json`: `.evidence[].stance`,
`.evidence[].bearings`).

**Titles are one line and short**: the claim, or the observation in a few
words ("200/200 passes with per-test temp dirs"); details, numbers, raw
output and command lines go in `--body`, whole logs in `hyp evidence
attach`. A text argument `-` reads stdin, at most one per command; for a
title, the first line of stdin is the title and the rest goes to the body.

**IDs from output.** A write prints the full ID of each record it wrote, one
per line, on stdout. Read it and pass it, or its first 10 characters, to the
next command, as in the Example. Do not capture IDs in shell variables:
agent sandboxes often block commands with `$VAR` or `$(...)`. With `--json`
a write prints `{"written": [{"id", "kind", "revision"}], "revision"}`;
`.written[i].revision` is that record's `expected_revision` for a follow-up
`hyp apply` patch, archive or delete. A write that changes nothing exits 0.

**Batches.** For a step of several records, `hyp apply` takes a JSON array of
changes on stdin (`hyp apply < step.json`, or a quoted heredoc
`hyp apply <<'EOF'`), all or nothing. A create may name its record
`"id": "@x"`; later changes write `"@x"` in record fields that take an ID
(and in `expected` keys). Omitted fields get the commands' defaults:

```json
[{"op": "create", "record": {"id": "@h", "kind": "hypothesis", "title": "Claim", "scope": "Where"}},
 {"op": "create", "record": {"id": "@f", "kind": "criterion", "hypothesis": "@h", "title": "What refutes it"}},
 {"op": "create", "record": {"id": "@x", "kind": "experiment", "hypothesis": "@h", "title": "Check", "targets": [{"id": "@f"}]}},
 {"op": "create", "record": {"id": "@e", "kind": "evidence", "title": "Seen", "source": "cmd", "locator": "job 7"}},
 {"op": "create", "record": {"kind": "link", "from": "@e", "to": "@f", "relation": "contradicts", "title": "Not met", "body": "Why"}},
 {"op": "create", "record": {"kind": "run", "title": "Run 1", "experiment": "@x", "evidence": ["@e"]}}]
```

`{"op": "patch", "id": "H-...", "expected_revision": "...", "set": {"title":
"..."}}` changes only the named fields. `hyp apply --help` documents every
change and what creating an assessment, experiment or run must state.

## Assessing

```bash
hyp show H-...     # review claim, criteria, evidence, runs
# line 1: H-...  hypothesis  review 46d8d8f5c79b   <- the token
hyp assess H-... --reviewed 46d8d8f5c79b --status weakened --confidence 0.3 \
  --evidence E-...,E-... --reason "Why these observations lead here"
```

- `--reviewed` takes the review token of the state you reviewed: the 12
  hex digits after `review` on the first line of `hyp show H-...`
  (`.state.review_token` with `--json`; its first 12 or more hex digits
  suffice). Take the token from the output you actually reviewed, never
  from a second read.
- Cite only evidence linked to the hypothesis or its criteria or predictions.
  For other evidence, `hyp link E-... H-...` first, then review again.
- Every judgment except `untested` needs `--evidence`; `falsified` also
  needs `--criterion F-...` and a cited observation that meets it (recorded
  with `hyp evidence add F-...`; evidence against H alone does not). See
  `hyp assess --help`. `--reason` is always required; `--confidence` 0.0-1.0.
- The token covers the basis (`.basis` in `hyp --json show`: claim, scope,
  assumptions, criteria, predictions, links, linked evidence with source
  and locator, runs, archiving any of them) and the current assessments;
  not closing, tags, experiment status or gaps. `.state.needs_review`: the
  basis changed since the current assessment; review and assess again.
  Every assessment changes the token, so read again before the next one.

When the investigation is over, `hyp set H-... --lifecycle closed`; that
does not mean true, the assessment is the judgment. An untestable hypothesis
gets `--untestable-reason "..."` instead of a criterion.

## Exit codes

- `0` success.
- `1` the input or project is wrong. Fix the cause; retrying unchanged will
  not help. `kind`: `invalid_input`, `not_found` (`ids` those that match
  nothing), `ambiguous_id` (use a longer prefix), `blocked` (repair files
  first, see Files), `unsupported_schema` (ask the user to upgrade hyp) or `io`.
- `2` invalid command-line arguments: see `hyp <command> --help`.
- `3` conflict (`kind` `conflict`, `ids` the records that changed, when
  known): something the write depended on changed since you read it.
  Nothing was written. Re-read, reconsider, retry with fresh values. For
  `hyp assess`: run `hyp show H-...` again, compare its basis with what you
  reviewed, reconsider the judgment, then assess with the new token; never
  copy a new token without reviewing what changed.
- `141` stdout was closed early (`| head`). A write may be on disk: check
  before retrying, and never truncate the output of a write.

Errors go to stderr; with `--json` as `{"error": "...", "kind": "..."}`.
Decide by `kind` and the exit code, not the message text.

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

IDs are illustrative; use those hyp prints. A multi-record step can be one batch.

```bash
hyp status                            # where things stand
hyp search "login"                    # anything recorded already?
hyp add "Flaky login test is caused by a shared temp dir" --scope "tests/login.rs on CI"
# prints H-3f2a9c1e-...: pass it, or its first 10 characters, on
hyp falsify-if H-3f2a9c1e "Test still fails with per-test temp dirs"   # F-8d0c22b1-...
hyp set H-3f2a9c1e --lifecycle investigating
hyp gap H-3f2a9c1e "Does isolation alone stop the failures?"          # G-5c0e2a19-...
hyp evidence add F-8d0c22b1 "200/200 passes with per-test temp dirs" --against \
  --source "cargo test login -- --test-threads=8" --locator "CI job 4411" \
  --body "Before: 23/200 failed on CI job 4402"    # E-7b1d0e44-..., then L-...
hyp show H-3f2a9c1e                   # review it all; line 1 ends "review 46d8d8f5c79b"
hyp assess H-3f2a9c1e --reviewed 46d8d8f5c79b --status supported --confidence 0.7 \
  --evidence E-7b1d0e44 --reason "Isolation removed all failures in 200 runs"
hyp set G-5c0e2a19 --resolved true --by E-7b1d0e44   # the evidence answered it
hyp set H-3f2a9c1e --lifecycle closed # the assessment still holds
```
