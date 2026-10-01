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

1. **Start with `hyp status`**: per open hypothesis its judgment, needs-review,
   missing criterion, linked evidence, open gaps and experiments without runs;
   then unexplained observations and what blocks writes. `hyp search "text"`
   finds what exists: continue a hypothesis rather than adding a duplicate,
   and reuse evidence with `hyp link`.
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

## Start from an observation

When something unexplained shows up first (a failure, an anomaly), record it
before guessing why, then add each serious explanation, naming earlier
explanations with `--competes-with`. The observation an explanation was made to
fit cannot tell rival explanations apart; decide with the experiment that could
falsify one (Method step 3 on). An observation stays unexplained until a live
hypothesis (not archived, not falsified) has it counting for or qualifying it.

```bash
hyp observe "Login test fails 23/200 on CI" --source "CI job 4402" \
  --locator "test login" --observed-at 2026-09-12        # prints E-...
hyp add "A shared temp dir causes it" --explains E-4c1d9a07   # H-, then L-
hyp add "Clock skew causes it" --explains E-4c1d9a07 --competes-with H-3f2a9c1e
```

## Commands

IDs look like `H-<uuid>`; the letter gives the kind (H hypothesis, F criterion,
P prediction, E evidence, L link, X experiment, R run, A assessment, G gap,
D data). Pass the full ID or an unambiguous prefix: the first 10 characters
(`H-1a2b3c4d`) nearly always are. `hyp <command> --help` documents the rest.

```bash
hyp status                     # start here: where each hypothesis stands
hyp list [--needs-review]      # hypotheses; --kind KIND or --all for others
hyp show H-...                 # summary for reading; line 1 has the review token
hyp --json show H-...          # .entry .state .basis .related .evidence .runs
hyp add "Title" --scope "where it applies" --tags a,b --body "Details"
hyp falsify-if H-... "Observation that would falsify it"
hyp predict H-... "Expected observation" --conditions "..."
hyp set H-... --lifecycle investigating                    # needs a criterion
hyp evidence add H-|P-|F-... "Short observation" --source PATH_OR_CMD \
  --locator "lines 10-20" [--against | --qualifies] --reason "Why it matters" \
  --body "Numbers, raw output, the exact command"          # prints E-, then L-
hyp capture ./run.log --origin "scp rig:/var/log/run.log"  # raw data, prints D-
hyp link E-...|H-... H-... --relation supports|competes-with --reason "..."
hyp experiment add H-... "Short procedure" --targets F-...,P-... --body "..."
hyp run X-... "Run 1" --outcome observed --evidence E-...
hyp gap H-... "Open question"
hyp set G-... --resolved true --by E-...,E-...             # what answered it
hyp check                      # validate every file; see Files
```

**Evidence on a criterion or prediction counts.** Evidence linked to an
active criterion or prediction is part of its hypothesis's basis and
citable in its assessments; do not also link it to the hypothesis. On a
criterion, `hyp evidence add F-...` (supports) records that the refuting
observation was made, against the hypothesis; `--against`, that it was not.
`hyp show H-...` groups observations by what they mean for it.

**Titles are one line and short**: the claim, or the observation in a few
words ("200/200 passes with per-test temp dirs"); details, numbers and command
lines go in `--body`. A text argument `-` reads stdin, once per command; for a
title, its first line is the title and the rest goes to the body.

**Keep raw data** (whole logs, outputs, files): `hyp capture FILE --origin
"where from"` or `cmd | hyp capture - --origin "cmd"` stores an immutable `D-`
record; name it from any record with `--data D-...` (observe, add, run, set).

**IDs from output.** A write prints the full ID of each record it wrote, one
per line; pass it (or its first 10 characters) on, as in the Example. Do not
capture IDs in shell variables: agent sandboxes often block `$VAR` and
`$(...)`. With `--json` a write prints
`{"written": [{"id", "kind", "revision"}], "revision"}`; `.written[i].revision`
is the `expected_revision` for a follow-up `hyp apply` patch, archive or delete.

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

`hyp apply --help` documents every change (a `"patch"` sets only named fields).

## Assessing

```bash
hyp show H-...     # review claim, criteria, evidence, runs
# line 1: H-...  hypothesis  review 46d8d8f5c79b   <- the token
hyp assess H-... --reviewed 46d8d8f5c79b --status weakened --confidence 0.3 \
  --evidence E-...,E-... --reason "Why these observations lead here"
```

- `--reviewed` takes the review token of the state you reviewed: the 12
  hex digits after `review` on line 1 of `hyp show H-...` (or 12 or more of
  `.state.review_token` with `--json`). Take it from the output you actually
  reviewed, never from a second read.
- Cite only evidence linked to the hypothesis or its criteria or predictions.
  For other evidence, `hyp link E-... H-...` first, then review again.
- Every judgment except `untested` needs `--evidence`; `falsified` also
  needs `--criterion F-...` and a cited observation that meets it (recorded
  with `hyp evidence add F-...`; evidence against H alone does not). See
  `hyp assess --help`. `--reason` is always required; `--confidence` 0.0-1.0.
- The token covers the basis (`.basis` in `hyp --json show`: claim, scope,
  assumptions, criteria, predictions, links, linked evidence with source,
  locator and data, runs, archiving any of them) and the current assessments,
  not closing, tags, experiment status or gaps. `.state.needs_review`: the
  basis changed since; review and assess again. Every assessment changes
  the token, so read again before the next one.

When the investigation is over, `hyp set H-... --lifecycle closed`; that
does not mean true, the assessment is the judgment. An untestable hypothesis
gets `--untestable-reason "..."` instead of a criterion.

## Exit codes

- `0` success, also for a write that changed nothing.
- `1` the input or project is wrong. Fix the cause; retrying unchanged will
  not help. `kind`: `invalid_input`, `not_found` (`ids` those that match
  nothing), `ambiguous_id` (use a longer prefix), `blocked` (repair files
  first, see Files), `unsupported_schema` (ask the user to upgrade hyp) or `io`.
- `2` invalid arguments (`hyp list` filters included); see `--help`.
- `3` conflict (`kind` `conflict`, `ids` the records that changed, when
  known): something the write depended on changed since you read it, so
  nothing was written. Re-read, reconsider, retry with fresh values. For
  `hyp assess`: `hyp show H-...` again and compare its basis with what you
  reviewed; never copy a new token without reviewing what changed.
- `141` stdout was closed early (`| head`). A write may be on disk: check
  before retrying, and never truncate the output of a write.

Errors go to stderr (`--json`: `{"error", "kind"}`); decide by `kind`, not text.

## Files

Never edit files under `hyp/` directly; use `hyp set ID` (`--title`, `--body`,
`--tags`, ...); leave `hyp edit` to people. After an editor, merge or sync
touched `hyp/`, run `hyp check`. A diagnostic has a `code` and may carry a
repair (`.repair.note`; `.repair.commands`, argv arrays to run in the project
directory). Read the note first: prefer restoring a missing record (a sync may
bring it) over deleting what refers to it. `malformed`, `attachment` and
`invalid` block every write: fix them by hand. `changed_bytes` (found only by
`hyp check`) blocks writes and assessments that rely on those bytes.

## Example

IDs are illustrative; use those hyp prints. A multi-record step can be one batch.

```bash
hyp add "Flaky login test is caused by a shared temp dir" --scope "tests/login.rs on CI"
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
