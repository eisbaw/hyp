# hyp

**A place to change your mind.** hyp is [backlog.md](https://github.com/MrLesk/Backlog.md), but for hypotheses: a notebook in which coding and research agents work in a structured way with tentative, unconfirmed information. When an agent suspects a cause, hyp gives it a place to record the claim, state what would falsify it, plan experiments, cite what it observed and record an explicit judgment, instead of declaring victory early. An investigation can also start from an observation nobody can explain yet and find its explanation later (`hyp observe`, see [Typical investigation](#typical-investigation)). Humans inspect and steer through a live WebUI.

Agents produce hypotheses all the time ("the bug is probably X") and readily report one as the root cause with nothing to back it. hyp turns the discipline into rules the tool enforces, because agents follow tool errors more reliably than advice in a prompt:

- An investigation needs a falsification criterion, or a stated reason why there can be none.
- Every judgment needs a rationale, and every one except `untested` cites linked evidence.
- `falsified` names a criterion and cites evidence that meets it.

Agents are the primary users, so the CLI's `--json` output, exit codes and error kinds are a [stable contract](#machine-contract) ([decision-0002](backlog/decisions/decision-0002%20-%20hyp-is-primarily-a-tool-for-agents-to-work-in-a-structured-way-with-tentative-unconfirmed-information-humans-inspect-and-steer.md)). For what hyp is not, see [When not to use hyp](#when-not-to-use-hyp).

Like backlog.md, it works in any directory: records are Markdown files, with captured bytes stored beside them. Git is not needed and hyp never runs it, but the files are Git-friendly ([decision-0001](backlog/decisions/decision-0001%20-%20hyp-does-not-require-or-depend-on-Git-but-is-Git-friendly.md); see [Using hyp with Git](#using-hyp-with-git-optional)).

No account, cloud service, database server, telemetry, CDN or JavaScript build step. One executable serves its embedded HTML/CSS/JavaScript assets. The application and all validation/storage logic are Rust.

## Start with Nix

From this source directory:

```bash
nix develop
nix run . -- init --demo
nix run . -- web
# Open http://127.0.0.1:7432
```

Omit `--demo` for an empty notebook. `init` creates a `hyp/` data directory and refuses to overwrite an existing one. The example observations are explicitly synthetic.

```bash
nix build                      # result/bin/hyp
nix flake check                # package tests, formatting, clippy and the jsdom UI test
nix run . -- --help
```

The flake pins nixpkgs and supports `x86_64-linux` and `aarch64-linux`. All Cargo dependencies are pinned in `Cargo.lock`. A first build requires network access or a populated Nix cache; the installed application works offline.

Or build directly with a recent Rust toolchain:

```bash
cargo build --release --locked
./target/release/hyp --help
```

Point the executable at another project with `--project /path/to/project`. It also discovers the nearest ancestor containing `hyp/config.toml`.

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

## Typical investigation

Commands print the IDs they create. Copy the full ID or use an unambiguous prefix. The `H-…`, `F-…` and `E-…` values below are placeholders for those returned IDs.

```bash
hyp init
hyp add "DMA timeout is caused by cache coherency" \
  --scope "Board revision C, firmware 0.8" --tags firmware,dma
hyp falsify-if H-… "Timeout reproduces with D-cache disabled"
hyp predict H-… "Clean + invalidate eliminates failures" \
  --conditions "10,000 transfers, 80 MHz"
hyp set H-… --lifecycle investigating
hyp experiment add H-… "Run 10,000 transfers with cache disabled" \
  --targets F-…,P-… --body "Keep clock and bus load constant."
hyp evidence add F-… "Timeout at transfer 8,142, cache disabled" \
  --source logs/run-142.txt --locator "lines 81–96" \
  --reason "A failure with the cache disabled is what the criterion describes."
hyp run X-… "Run 142" --outcome observed --evidence E-…
hyp --json show H-…          # review .related, .runs, .evidence; note .state.review_token
hyp assess H-… --reviewed <token> --status weakened --confidence 0.2 \
  --evidence E-… --reason "Check the timing confound before rejecting the hypothesis."
hyp assess H-… --reviewed <token> --status falsified --criterion F-… \
  --evidence E-… --reason "Controlled replication satisfies the rejection criterion."
hyp show H-…                 # a summary; line 1 ends with its review token; --json for everything
hyp status                   # where each open hypothesis stands; start here when resuming
hyp list --needs-review      # hypotheses by default; --kind KIND or --all for others
hyp search "cache"
hyp check
```

Use `hyp evidence add --qualifies` for an observation that limits the claim. Without either `--against` or `--qualifies`, it creates a supporting interpretation. Evidence linked to an active criterion or prediction counts toward its hypothesis like evidence linked to the hypothesis itself: it is in the hypothesis's basis and its assessments may cite it. A link's relation is to its target, so evidence that *supports* a falsification criterion meets it and counts *against* the hypothesis; `--against` on a criterion records that it was not met. `hyp show H-…`, `hyp --json show H-…` (`.evidence[].stance`: `for`, `against`, `qualifies` or `mixed`, and `.evidence[].bearings`, one per link with `via`, `relation`, `stance` and a `meaning` for people) and the WebUI present each observation once, by what it means for the hypothesis: "meets criterion F-… (counts against H)", "does not meet criterion F-… (counts for H)", "matches prediction P-…", "supports H". An observation is `mixed` only when some of its links count for the hypothesis and some against; qualifying links do not change the direction of the others, and it is `qualifies` only when all its links qualify. Evidence can be reused:

```bash
hyp link E-… H-… --relation supports --reason "Why this observation matters"
hyp link H-… H-… --relation competes-with --reason "Alternative explanation"
hyp gap H-… "Does disabling cache alter DMA timing?"
hyp set G-… --resolved true --by E-…   # --by: the evidence that answered it (optional)
hyp evidence attach E-… ./capture.txt   # prints E-…, then the D- data record holding the file
hyp experiment add H-… "Replication" --targets P-…,F-…
hyp set X-… --experiment-status running
hyp edit H-…                 # $VISUAL, then $EDITOR, then vi; not assessments, runs or data
hyp archive H-…
hyp restore H-…
hyp delete H-…               # only archived and unreferenced records
```

An investigation can also start from an observation: something seen before anyone suspects why. `hyp observe` records it as evidence without links and prints its `E-` ID; `hyp add --explains E-…` then creates a hypothesis and, in the same write, a `supports` link from each named observation to it, so the observation is in its basis and citable. Add each serious explanation this way and name earlier ones with `--competes-with H-…` (a `competes-with` link from the new hypothesis; its reason names the observations both explain). Then continue as above: a criterion for each, the experiment that could falsify one.

```bash
hyp observe "Login test fails 23/200 on CI" --source "CI job 4402" \
  --locator "test login" --observed-at 2026-09-12   # prints E-…
hyp add "A shared temp dir causes it" --explains E-…  # H-…, then its link L-…
hyp add "Clock skew causes it" --explains E-… --competes-with H-… \
  --reason "Timestamps jump between the failing steps"
hyp show E-…                 # the hypotheses it bears on, and how
```

`--observed-at` (on `observe`, `evidence add` and `hyp set E-…`) takes an RFC 3339 timestamp or a date (`YYYY-MM-DD`) and is stored as given, or `''` for unknown (stored empty); anything else is an argument error (exit `2`). Without it, `observe` and `evidence add` store the time of recording. Every writer (`hyp apply`, the WebUI too) is held to the same rule when it stores a new or changed value, and a value more than a day in the future is refused as well: kind `invalid_input` (exit `1`), with a `bad_observed_at` diagnostic. A value already stored is never refused: older versions stored the WebUI's free text as given ("last tuesday night", a `datetime-local` value without an offset), and nothing hyp derives depends on it. `hyp check` reports such a value as the warning `bad_observed_at` (it does not block writes; `--strict` fails on it), whose repair command keeps the text in the body and clears the field in one write: `hyp set E-… --body=<the body, a blank line, "Observed at (as recorded): last tuesday night"> --observed-at=` (exact as the argv of `--json`; plain `hyp check` prints it shell-quoted). It holds the body as that check read it and states no revision, so run `hyp check` again right before running it; if you know the time, `hyp set E-… --observed-at 2026-09-12` instead. `--explains` takes evidence IDs and `--competes-with` hypothesis IDs, comma-separated or repeated; an ID given twice gets one link. Without `--reason`, each `--explains` link's reason is "Proposed as an explanation of this observation". `hyp show E-…` lists under *Bears on* each hypothesis the observation is in the basis of, with its stance (`for`, `against`, `qualifies`, `mixed`), the hypothesis's current judgment and lifecycle, and each link's meaning, and says when it is unexplained; `hyp --json show E-…` gives the same as `bears_on: [{"hypothesis", "archived", "judgment", "stance", "bearings"}]` and `unexplained` (both `null` for other kinds).

An observation is unexplained until a live hypothesis (not archived, current judgment not falsified) accounts for it: it counts for or qualifies that hypothesis through an active link, to the hypothesis or to an active criterion or prediction of it. So an observation is unexplained again once every explanation of it is falsified or archived; one that only counts against live hypotheses, or only falsified one, is unexplained too (link it to the hypothesis it bears on); and so is evidence that only a run or a gap names. The observation an explanation was made to fit cannot tell rival explanations apart: decide between them with the experiment that could falsify one.

The raw data behind an observation (a log excerpt, command output, a capture file, a measurement table) can be kept as a **data record** (`D-…`, decision-0005): `hyp capture` copies the bytes into the project with where they came from, and any other record names it with `--data D-…`. One data record can back many observations, hypotheses and later investigations.

```bash
hyp capture logs/run-142.txt --origin "scp rig3:/var/log/dma.log"   # prints D-…
journalctl -u dma --since today | hyp capture - --origin "journalctl -u dma --since today" \
  --title "DMA service log, 30 Sep" --body "Taken right after the third timeout"
hyp observe "Timeout at transfer 8,142" --source logs/run-142.txt --data D-…
hyp add "The DMA descriptor ring overflows" --explains E-… --data D-…
hyp set E-… --data D-…,D-…     # replaces the list; not for assessments, runs or data records
hyp show D-…                   # metadata and every record that references it
hyp data get D-… > copy.txt    # the bytes, exactly; --output FILE writes a file
```

`hyp capture FILE` (or `-` for stdin, since hyp never runs commands itself) stores the bytes once per SHA-256 at `hyp/assets/<sha256>`, at most 32 MiB; empty input is an error unless `--allow-empty` (an empty pipe usually means the command before it failed). It records a data record: `title` (default: the file name, for stdin the origin's first line), `origin` (required, free text: a path, URL, host or the command that produced the bytes), `captured_at` (set by hyp), `media_type` (`--media-type`, else guessed: a few common file extensions, then `text/plain` for UTF-8 text, else `application/octet-stream`), `size`, `sha256`, and the body as an optional note (`--body`). A data record cannot be changed afterwards (`set`, `edit`, update and patch are refused); it can be archived, and deleted once archived and no record references it (the error lists the referrers). Capturing the same bytes again makes a new record that shares the stored file; repeating a capture with the same bytes, title, origin, media type and note (a retried command, or the same capture run twice at once) prints the existing ID and writes nothing. `hyp data get --output FILE` writes the file atomically and refuses a path inside `hyp/` or `.hyp/`. `--data` (comma-separated or repeated) is accepted by `observe`, `evidence add`, `add` (on the hypothesis), `predict`, `falsify-if`, `gap`, `run` and `assess`, and by `hyp set` for records that can change; in `hyp apply` every record takes `"data": ["D-…"]`, batch-local references included. `hyp evidence attach E-… FILE` is capture plus reference: it reuses a data record the evidence references, the one the migration below makes when a legacy attachment holds the same bytes, or any with the same bytes, before capturing a new one (origin: the path as given), and prints the evidence's ID, then the data record's. Evidence that already holds the bytes is left unchanged (`no changes`); when it holds them as a legacy attachment, only the evidence's ID is printed, as no data record exists yet.

`depends-on`, `competes-with`, and `supersedes` are CLI relation values, and plain output (`list`, `show`, `graph`, Markdown export, error messages) spells them so. Files and JSON use the serialized form with underscores (`competes_with`). Dependencies and supersession cannot form cycles. Competing hypotheses may be linked in either direction; the relation does not imply mutual exclusivity.

Use `-` for a text argument to read stdin. Flags accept literal multiline values. A title is one line: with `-`, the first line of stdin is the title and the rest is appended to the body (after a blank line if the body is not empty); a given title with a newline is an error, and `hyp check` reports a stored one. All ordinary commands support `--json`. `hyp apply` accepts a JSON array of create/patch/update/archive/delete changes on stdin, with optional `--expected-revision` for a whole-project precondition; `hyp apply --help` shows each change with examples. A create needs only what the matching command asks for: omitted fields get the command's defaults (a hypothesis is a draft, an experiment is planned and targets its hypothesis, a run is observed, a gap open), and the server sets the timestamps. A patch (`{"op": "patch", "id", "expected_revision", "set": {field: value}}`) changes only the fields in `set`, validated as an update; it cannot change `id`, `kind` or `created_at`, and assessments, runs and data records cannot be patched. A data record can also be created in a batch (`"kind": "data"`, `title`, `origin`, `sha256`, optional `media_type`) for bytes hyp already stores (as `hyp capture` stored them); the server sets `captured_at` and `size`. A create may give `"id": "@name"`: later changes in the same batch write `"@name"` in the record fields that take an ID (`hypothesis`, `from`, `to`, `experiment`, `evidence`, `criterion`, `targets[].id`, `resolved_by`, `data`) and in the keys of `expected`, and hyp replaces it with the full ID it generates, so one batch can create a hypothesis, its criterion, an experiment, evidence, links and a run. An unknown or twice-defined reference is an ordinary error, and so is updating, patching, archiving or deleting a record the same batch creates (put its values in the create). Only one argument of a command can be `-` (stdin is read once); more is an error (exit `1`).

For people at a terminal: when stderr is a terminal, a write command also prints a one-line summary there (`created evidence E-… (+ link L-…: E-… supports H-…)`), and a `-` argument read from a terminal prints how to end the input (Ctrl-D). A write that changes nothing (`hyp set H-…` without flags, restoring a record that is not archived, saving `hyp edit` unchanged) prints `no changes` to stderr, whether or not it is a terminal (not with `--json`), and exits `0`. When `hyp edit` rejects the edited record and stdin and stderr are terminals, the editor reopens with the error as `# hyp:` comment lines under the opening `---`; line numbers in it count lines of that file. Saving it unchanged, or emptying it, aborts (exit `1`) with the last error and names a copy of the text. Without a terminal (a scripted editor) the first error fails the command the same way, without reopening.

`hyp status` answers where the investigation stands in one read: a project line (how many open hypotheses, how many need review, whether writes are blocked and by which files, other `hyp check` findings; with none open, that `hyp list` shows the conclusions), then two lines per open hypothesis (not archived, not closed) and per closed one that needs review, those needing review first: its judgment and lifecycle, its criteria (or `no criterion`, or its untestable reason), how much evidence is linked, open gaps (and resolved ones, with the evidence that resolved them) and experiments without runs. Last come unexplained observations: evidence, not archived, that no live hypothesis accounts for (see the rule above). The project line counts them and the plain text lists the first five, then how many more. `hyp --json status` gives the same as `{"hypotheses": [row], "not_shown": {"closed", "archived", "needs_review"}, "unexplained_observations": [{"id", "title"}], "writes_blocked", "blocking": [diagnostics], "errors", "warnings", "revision"}`, each row `{"id", "title", "lifecycle", "judgment", "confidence", "needs_review", "criteria": [IDs], "untestable_reason", "missing_criterion", "linked_evidence": [IDs], "open_gaps": [{"id"}], "resolved_gaps": [{"id", "resolved_by": [evidence IDs]}], "experiments_without_runs": [IDs]}`. A row lists what it has by full ID, never as a count (the plain text counts them): `criteria` the active criteria, `linked_evidence` the evidence an assessment may cite, leaving out evidence whose file did not load (`hyp check` reports it). `not_shown.needs_review` counts archived hypotheses that need review (a closed one that needs review is listed instead); while it is not `0`, or an observation is unexplained, the project line does not call the investigation finished. It prints no review token: take that from the `hyp show` output you reviewed.

`hyp list` lists hypotheses, each with its judgment, lifecycle and a `needs-review` marker; `--kind KIND` lists another kind and `--all` every kind. A `--status` or `--needs-review` the listed kind cannot have (`--status planned` without `--kind experiment`) is an argument error (exit `2`), not an empty list.

### Machine contract

Agents depend on these; they are kept stable.

| CLI exit code | Meaning                                                                                                                                             |
| ------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| `0`           | Success.                                                                                                                                            |
| `1`           | Any other error: invalid input, an unknown or ambiguous ID, a missing or malformed precondition, project errors, I/O (`kind` tells them apart). Retrying unchanged will not help. |
| `2`           | Invalid command-line arguments.                                                                                                                     |
| `3`           | Conflict: something the write depended on changed since it was read. Nothing was written. Re-read, review, retry.                                  |
| `141`         | Killed by SIGPIPE: stdout was closed early (`hyp list \| head`). A write command prints after writing, so its write may already be on disk.           |

Write commands (`add`, `evidence add`, `assess`, `set`, `apply`, `capture`, `evidence attach` and the rest) print the full ID of each record their changes named, one per line and in order (`evidence add`: the evidence, then its link; `add`: the hypothesis, then one link per `--explains`, then one per `--competes-with`; `observe`: the evidence; `capture`: the data record; `evidence attach`: the evidence, then the data record, since 0.3.0). With `--json` they print `{"written": [{"id": "H-…", "kind": "hypothesis", "revision": "…"}], "revision": "…"}`: each record's revision after the write (`null` once deleted), usable as the `expected_revision` of a next change, and the project revision (what `apply --expected-revision` takes). `hyp --json init` prints the same shape, listing every record of the new project. A write that converted legacy attachments also has `"migrated"` (see [Schema versions](#schema-versions)). In `hyp apply` output, an entry whose create gave a batch-local reference also has `"ref": "@name"`. Stdout is the same when a change turned out to change nothing (the revision stays as it was). Stderr carries a human summary only when it is a terminal, `no changes` only without `--json`.

Plain output is for people and may change, except the first line of `hyp show ID`: the full ID and the kind separated by two spaces, then for a hypothesis two spaces, `review` and the first 12 hex digits of its review token (`H-…  hypothesis  review 46d8d8f5c79b`), for any other record two spaces, `revision` and the first 12 hex digits of its revision (`X-…  experiment  revision 0f3ac2d19e7b`; hyp 0.3.0 printed only ID and kind). These are what `--reviewed` takes (see [Preconditions](#preconditions)). Scripts may read it; the skill's flow script does. Everything else is in `--json`.

Errors go to stderr, with `--json` as `{"error": "…", "kind": "…"}`. `kind` is decided by the error's type where it arises, not by its text, and is one of:

| `kind`          | Meaning                                                                                              |
| --------------- | ---------------------------------------------------------------------------------------------------- |
| `conflict`      | Exit `3`. Something the write depended on changed. `ids` lists the records whose statement failed, when known (not for `--expected-revision`). |
| `invalid_input` | Input the rules reject: a malformed value or batch, a missing statement, a write that would add a `hyp check` error. For the last, the message names each new error's path and code, and `diagnostics` lists them (with `repair` null: nothing was stored to repair). |
| `not_found`     | A named record (or the project) does not exist, including one a change states that was deleted since it was read (see [Preconditions](#preconditions)). `ids` lists the given IDs that matched nothing, when known. |
| `ambiguous_id`  | An ID prefix matches more than one record; the message lists them.                                   |
| `blocked`       | A file that `hyp check` reports as blocking (`blocks_writes`) must be repaired before any write; `diagnostics` lists the blocking ones. An invalid record can be repaired through hyp (see [Files and concurrency](#files-and-concurrency)); a repair that leaves it invalid is `blocked` too, its `diagnostics` naming what is still wrong. |
| `check_failed`  | `hyp check` found errors (with `--strict`, also warnings); stdout lists them. hyp 0.3.0 gave `invalid_input` ("validation failed"). |
| `unsupported_schema` | The notebook uses a newer schema than this hyp reads; the message names the hyp version to upgrade to. Nothing is read or written. See [Schema versions](#schema-versions). |
| `io`            | Reading or writing a file failed.                                                                    |

`diagnostics` has the form `hyp --json check` prints: `[{"path", "code", "message", "severity", "blocks_writes", "repair"}]`, where `(path, code)` identifies each (see [Files and concurrency](#files-and-concurrency)). In the error of a rejected write, or of one blocked by invalid records, a diagnostic about a record one of the write's changes names also has `"change"`, that change's index (from 0, in the order given; a CLI command's changes are in the order it prints their IDs; when several changes name one record, the last), and `"ref"` when the change gave a batch-local reference: a record the write would create has a path that names no file yet. (A write blocked by a malformed file or missing bytes is refused before its changes are read, so its diagnostics have no `change`.) `severity` and `blocks_writes` always describe the code as `hyp check` reports it, not the write: a refused `observed_at` comes as `bad_observed_at`, a warning that does not block writes, although this write was refused. Decide by `kind` and `code`.

The set of kinds may grow: treat a kind you do not know by the exit code. A conflict's message starts with `conflict:`. Argument errors (exit `2`) are clap's usage text, not JSON. The WebUI's HTTP API answers errors with the same JSON body, and `403` for a missing or invalid request token or an untrusted `Host`/`Origin`, `409` for a conflict, `422` for input the domain rules reject, and another `4xx` for other rejected input (malformed JSON, an oversized body, a wrong content type).

### Preconditions

Every change states what it depends on, as read from a snapshot. `hyp --json show ID` gives a record's `entry.revision` and the records that refer to it (`related`); for a hypothesis also its `state` and `basis`, what its fingerprint covers (see [The model](#the-model)), with the `runs` and `evidence` in it in full; for evidence the hypotheses it `bears_on` and whether it is `unexplained`. `hyp export --format json` gives everything. A change that depended on something that changed is rejected; a change that did not is unaffected by other writers.

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

An assessment may cite only evidence with an active link to its hypothesis, or to one of its active criteria or predictions (`hyp link E-… H-… --relation supports --reason "…"`). Citing other evidence, a judgment other than `untested` without evidence, or `falsified` without cited evidence that meets its criterion (below), is an ordinary error. In an `apply` batch, the assessment's basis may grow only by records the batch creates: to bring in evidence that already existed, link it in an earlier write, re-read, then assess.

The CLI commands state preconditions from their own read, so they protect only the moment between that read and the write. `hyp assess` is the exception: it requires `--reviewed` with the review token of the state you reviewed: the 12 hex digits after `review` on the first line of `hyp show H-…`, or `.state.review_token` of `hyp --json show H-…` or `hyp --json list` (its first 12 or more hex digits suffice). If the hypothesis's basis or its current assessments changed since, it writes nothing and exits `3`; `hyp show H-…` then lists the basis to compare with what you reviewed. A malformed token, or a `--confidence` outside 0.0 to 1.0, exits `1`. `hyp apply` takes only the full token.

The commands that freeze content take the same statement, optionally: `hyp experiment add H-… --reviewed TOKEN`, the hypothesis's review token as for `assess`. The token covers the claim, criteria and predictions the experiment freezes as its targets (not their lifecycle or tags), but just as much the rest of the basis and the current assessments: new or changed linked evidence, links, runs or an assessment since your review also exit `3`, though the experiment would freeze nothing different; re-read and retry. (Precise per-target statements are `expected.revisions` in `hyp apply`, which compare raw revisions, so a lifecycle or tag change does conflict there.) And `hyp run X-… --reviewed REVISION`, the experiment's revision as line 1 of `hyp show X-…` prints it (12 or more hex digits of `.entry.revision`), so the run freezes the plan you executed. A change since then exits `3` and writes nothing; any change to the experiment counts, its status too, so review it after setting it running. Without `--reviewed` they freeze what their own read finds. They are optional because, unlike a judgment, the frozen content is kept in the record itself: what the experiment or run rests on is never lost, only possibly newer than what you saw. A run's cited evidence is named, not frozen, so `--reviewed` does not cover it. To protect a longer window for other records, use `hyp apply`. The WebUI states everything as of the moment a form was opened.

## The model

| Record     | Purpose                                                                        |
| ---------- | ------------------------------------------------------------------------------ |
| Hypothesis | Claim, scope, assumptions, lifecycle, tags                                     |
| Prediction | Expected observable result, conditions, owning hypothesis                      |
| Criterion  | Observation that would falsify the scoped claim                                |
| Evidence   | Observation, source, locator, date                                             |
| Link       | Interpretation: supports, contradicts, qualifies; or a hypothesis relationship |
| Experiment | Procedure, status, frozen hypothesis/prediction/criterion references           |
| Run        | Immutable snapshot of the experiment plan, outcome and evidence IDs            |
| Assessment | Immutable judgment, subjective confidence, rationale and evidence IDs          |
| Gap        | Missing information, open or resolved, optionally by cited evidence            |
| Data       | Immutable captured bytes: title, origin, captured_at, media type, size, sha256 |

Every record except a data record may reference data records (`data: [D-…]`).

Predictions and criteria are separate Markdown records, which makes them individually addressable and avoids rewriting the hypothesis every time one is added.

Lifecycle is **draft / investigating / paused / closed**. Assessment is **untested / inconclusive / supported / weakened / falsified**. Closing an investigation never declares its hypothesis true.

- Drafts may be incomplete. Investigating requires an active criterion or an explicit `untestable_reason`.
- Every assessment needs a rationale. Every judgment except untested must cite evidence linked to the hypothesis (or its active criteria or predictions). Falsification also names an active criterion belonging to it, and at least one cited observation must meet that criterion: have an active `supports` link to it (`hyp evidence add F-…`, or `hyp link E-… F-… --relation supports`). Evidence merely against the hypothesis does not falsify it. `hyp assess --help` lists each judgment's requirements. These rules apply when an assessment is created; a stored one stays valid if a link it relied on is archived later. Whoever records the assessment, agent or human, judges whether the observation actually satisfies the criterion; hyp checks only that it was recorded as meeting it.
- Meeting a criterion and not meeting it are not symmetric. Evidence that meets a falsification criterion is decisive: it is what `falsified` rests on. Evidence that a criterion was not met (`--against` on it) counts for the hypothesis only as mild corroboration, one refutation it survived; it proves nothing, and `supported` never means proven.
- Confidence is optional, subjective, and in `[0, 1]`. Evidence counts never calculate it.
- An assessment records the hypothesis's fingerprint: the SHA-256 of its `basis` (`hyp --json show`) as compact JSON with sorted keys. The basis holds content only: the claim (title, body, scope, assumptions, archived); its criteria and predictions (title, body, conditions, archived); links touching the hypothesis or those (ends, relation, reason, archived), so another hypothesis counts only through its link; the evidence with an active link to the hypothesis or an active criterion or prediction; and runs of its experiments (title, body, outcome, cited evidence) with the evidence they cite. Evidence counts with its provenance (title, body, source, locator, attachment hashes, archived). Each of these records, the hypothesis included, that references data records counts with their SHA-256s, in order: under `data`, and for evidence after its attachment hashes under `attachments` (so converting an attachment into a data record changes no fingerprint); the data records' other metadata does not count. A change to it shows **needs review** without rewriting the judgment. Lifecycle, tags, the untestable reason, experiments, gaps and timestamps are not part of it, so closing a hypothesis does not flag it; archiving it does.
- A new assessment supersedes the current assessment heads. Divergent heads after any merge or sync require explicit reconciliation; neither silently wins by timestamp.
- Experiments freeze complete target content and revisions at creation. Runs freeze the complete experiment plan at execution-record creation. `hyp run` records an execution; it does not execute commands.
- A gap can name the evidence that resolved it (`resolved_by`, `hyp set G-… --resolved true --by E-…`); `--resolved false` forgets it. Gaps stay outside the basis (decision-0003), so resolving one never flags an assessment. The first `resolved_by` raises the notebook to schema 2 (see [Schema versions](#schema-versions)).
- Historic assessments and runs cannot be edited or deleted through the tool. To keep a history of all file edits, use any version control, e.g. Git. There is no claim of tamper-proof auditing.

## WebUI

Run `hyp web [--port 7432]`. The server binds to IPv4 loopback only. Browse manually to the printed URL; it does not automatically launch a browser.

- Hypothesis overview: search, assessment/tag filters, needs-review and archived records.
- Hypothesis detail: falsification criteria, predictions, evidence by what it means for the hypothesis (for, against, qualifying; evidence that meets a falsification criterion counts against), each observation once with all its links, experiments, gaps and assessment history.
- Experiment queue and immutable runs.
- Reusable evidence and interpretations.
- Evidence matrix to compare alternative hypotheses, each cell by what the observation means for that hypothesis.
- Focused relationship graph with navigable nodes.
- Create/edit/archive/restore/delete forms, plus advanced JSON editing for mutable records.
- Incoming changes preserve dirty forms; conflicting saves are rejected and the draft remains available to copy/reconcile.
- External editor and CLI saves update open tabs using filesystem notifications and SSE. A two-second reconciliation scan catches missed notifications. Reconnects fetch a full snapshot.
- Malformed files show diagnostics and block writes. An already open browser preserves the last readable state and marks it stale. Errors between records (see "Files and concurrency") show as a notice with their repair; saving still works unless it adds a new error.

- Data records with their metadata and the records that reference them; for `text/*` media types a preview of the first 4 KiB, as escaped text.

Notes are displayed as escaped, pre-wrapped text. Markdown is retained in files and exports; the UI does not execute raw HTML. Data is captured through the CLI; the WebUI shows metadata and text previews and never serves the stored bytes themselves, so nothing captured can run as browser content.

## Files and concurrency

All authoritative content is under `hyp/`:

- `config.toml` — project name and schema version (`schema_version`, see below).
- `hypotheses/`, `predictions/`, `criteria/`, `evidence/`, `links/`, `experiments/`, `runs/`, `assessments/`, `gaps/`, `data/` — one `<full-id>.md` per record.
- `assets/` — the bytes of data records (and legacy attachments), one file per SHA-256, named by it (maximum 32 MiB each).
- `.hyp/` at the project root — lock files (`write.lock`, `gate.lock`, `apply.lock`) and the temporary recovery journal; not project data. It contains a `.gitignore` that ignores it.

### Schema versions

Files are read strictly: a field hyp does not know makes a record `malformed`, which catches typos and corruption. So a field or value that older hyp versions cannot read raises the notebook's schema (decision-0004):

| Schema | First hyp version | Adds                                      |
| ------ | ----------------- | ----------------------------------------- |
| 1      | `0.1.0`           | The record fields of hyp 0.1.0.           |
| 2      | `0.2.0`           | A gap's `resolved_by` (`hyp set G-… --by`). |
| 3      | `0.3.0`           | Data records (`D-`, `hyp capture`) and `data` references; evidence attachments become data records. |

`hyp init` creates schema 1. A write raises `schema_version` when it first stores something only a later schema holds, in the same journaled write as the records, so an interrupted write leaves both or neither; it is never lowered. Reading never raises it. So a notebook stays readable by older hyp versions until a newer feature is used, and then it is not, for anyone who has not upgraded (mixed versions sharing a notebook through Git or a sync must upgrade together).

Raising a notebook to schema 3 (the first capture, `--data` reference or `hyp evidence attach`) also converts every evidence attachment into a data record in the same journaled write. The conversion depends only on the notebook, not on the clock, so two copies of a notebook (parallel worktrees, clones) convert to identical files and merge cleanly: one data record per distinct hash, with an ID derived from the hash (a UUID version 5 in hyp's namespace; a record with that ID is reused), title `Migrated attachment <first 8 hex digits>`, origin `migrated from evidence attachment <path>`, and as its times the earliest `created_at` of the evidence holding the bytes. The evidence references them via `data` instead, first and in attachment order, loses `attachments`, and keeps its `updated_at`. Stored bytes stay where they are, and no fingerprint changes (see [The model](#the-model)), so no assessment needs review because of it. A write to a schema-3 notebook converts any attachment a merge from an older branch brings in the same way. With `--json` the write lists the records it converted as `"migrated": [{"id", "kind", "revision"}]` (the data records it created and the evidence it changed; left out when there are none); without, it names them on stderr. Reading never converts anything.

A hyp that meets a newer schema than it reads refuses the whole notebook with one error, kind `unsupported_schema` (exit `1`), naming the version to upgrade to, and reads and writes nothing: "this notebook uses schema 3 (…/hyp/config.toml), but this hyp 0.2.0 reads schemas 1 to 2: upgrade hyp to >= 0.3.0". From schema 3 on, the write that raises a notebook also stores that version as `min_hyp_version` in its `config.toml`; without it the message asks for a version newer than the running one. hyp 0.1.0 predates this and reports a schema-2 notebook as `unsupported schema version 2`. An unknown key in `config.toml` of a schema hyp reads, or text that is not TOML, is invalid input as before.

Markdown files have YAML front matter and an ordinary notes body. IDs are UUIDs with readable type prefixes. Renaming files independently of IDs is rejected. Unknown schema fields, broken references and invalid states are reported by `hyp check`. `hyp check --strict` also fails on warnings such as a hypothesis with neither a falsification criterion nor an untestable reason.

Stored bytes are checked by metadata on every read: each file a record names exists, is a regular file inside `hyp/assets/`, and has the length the data record states (a length that differs is then hashed, to tell changed bytes from a changed record). Only `hyp check` hashes every stored file, so bytes changed in place to others of the same length (and any change to a legacy attachment, which records no length) show only there, as `changed_bytes`. hyp still refuses to rely on such bytes: a write that newly cites stored bytes (a capture, `--data`, `hyp evidence attach`) hashes them first, so does an assessment for the stored bytes its hypothesis's basis cites, and `hyp data get` hashes what it returns. Other writes are not blocked by them, before or after `hyp check` reports them.

`hyp check` reports every rule a record breaks, one diagnostic each. Each has a stable `code` (`hyp --json check`); `(path, code)` identifies it, and the message may change. The path is relative to the project root and always under `hyp/`: the record's file (`hyp/hypotheses/H-….md`), for every code including the warnings (`no_criterion`; `bad_observed_at`, see [Typical investigation](#typical-investigation)), or for stored bytes that no record file stands for (a legacy attachment, a stray symlink) the file under `hyp/assets/`. (hyp 0.3.0 gave `no_criterion` the bare ID and a legacy attachment a path relative to `hyp/`.) Two kinds of error differ in what they block:

- `malformed` (a file hyp cannot load: bad front matter, a filename or directory that does not match the record, a duplicate ID), `attachment` (stored bytes of a data record, or of a legacy attachment, that are missing, changed or not a regular file, and a symlink in `hyp/assets/` named like a SHA-256 (hyp ignores other entries there); for a data record the repair note says to restore `hyp/assets/<sha256>`, or to capture the original again, which stores the same bytes at the same path) and `invalid` (including a data record whose `size` does not match its intact stored bytes: the record file changed, not the bytes) (a record breaking rules of its own fields, including a reference by short ID or to a record of the wrong kind, which the ID prefix gives) block every write (`blocks_writes: true`). Fix the file by hand, or restore it. An `invalid` record can also be repaired through the CLI (`hyp edit ID`, `hyp set ID …`, a patch in `hyp apply`; the WebUI keeps showing the last readable state while writes are blocked): while only `invalid` records block, a write is accepted whose every change updates, patches, archives or deletes an invalid record, and that leaves each of them valid (or deleted); the repair may change other fields of such a record too; one that leaves a record it changes invalid, a write that changes nothing included, is `blocked`, naming what is still wrong. Any other write is `blocked` before its other checks (preconditions included). Assessments, runs and data records cannot be changed through hyp, so for them only the file can be fixed.
- `changed_bytes` (stored bytes changed in place to bytes of the same length, or a changed legacy attachment; found only by `hyp check`, see above) is an error with `blocks_writes: false`: it blocks only writes that newly cite those bytes and assessments whose basis cites them. Its repair note is the same as for `attachment`.
- `dangling_reference` (a referenced record does not exist), `cycle` (`depends-on` or `supersedes` links, or assessment supersession) and `inconsistent` (other rules between records, such as an investigating hypothesis without an active criterion) are errors between loaded records, as a merge, sync or hand edit leaves them. They do not block writes: a write is rejected if, for any record, it adds a `(path, code)` the project did not already have, and the error names each one it would add (`diagnostics` with `--json`), on whichever record: archiving a criterion can add one to its investigating hypothesis. So an unrelated write still works, and so does the write that repairs them.

Where hyp knows a repair, the diagnostic carries it: `--json` as `"repair": {"note": "...", "commands": [["hyp", "archive", "L-..."], ...]}` (`note` may be null, `commands` empty), and plain `hyp check` as `note:` and `repair:` lines. Run the commands in order, as argv arrays, in the project directory (they carry no `--project`). Plain `hyp check` prints each on one line with its arguments shell-quoted where needed (`$'…'` for a newline), so a body in a command cannot break out of it. A cycle's repair archives the link. For a dangling reference the note comes first: restore the missing record from the source of the merge or sync. That loses nothing, and after a partial sync the record may simply not have arrived yet. Only a link also gets commands (archive, then delete) for when the record is gone for good; a delete cannot be undone without version control. So does a gap resolved by evidence that is gone: `hyp set G-… --by` the evidence that remains, or with none `hyp set G-… --resolved false`, which reopens it. Other records whose referenced record is missing get only the note. An `invalid` record's note says how to fix it through hyp, or that its kind cannot be changed.

A process-shared advisory lock (`.hyp/write.lock`) serializes tool writes. Each change carries an optimistic precondition on what it depends on (see above), so stale writes are rejected without turning unrelated concurrent writes into conflicts. Atomic file replacements and an fsynced roll-forward journal recover interrupted multi-file operations. Reads do not take the write lock: they share `.hyp/apply.lock`, which a write holds exclusively only while it writes, applies and removes its journal. So a read waits only for that (milliseconds), never for a whole write, and sees each write whole or not at all. A write that is ready to apply first takes `.hyp/gate.lock`, which every read passes through before it takes its shared lock, so new reads queue behind it and it waits only for the reads already in progress: reads that never stop (the WebUI polling, several tabs) cannot starve it. While a writer waits like that, new reads wait for it, so a read can be held up by the longest read in progress; `hyp check` therefore hashes stored bytes after releasing its lock (they are named by their hash and never rewritten in place, and a file removed meanwhile is reported missing). The lock order is write, gate, apply; a read that finds the journal of a crashed write lets go and rolls it forward as a writer does, under the write lock. The lock files are created with `.hyp/` whenever hyp can write there. Where it cannot (read-only media, another user's notebook), reads open them read-only, which `flock` allows; only when they do not exist and cannot be created (a copy without `.hyp/` on read-only media, where nothing writes through hyp either) do reads go without a lock. A lock file that is not a regular file (a symlink, a FIFO) is refused with the fix. On a read-only NFS mount, where an exclusive lock needs write access, reads skip `gate.lock` and lose only writer precedence. A pending journal there is an error, as it cannot be rolled forward. hyp 0.3.0 and earlier do not know `gate.lock` and `apply.lock`, so a read of this version can see half of a write of theirs: do not run them alongside this version on one notebook. Manual editors, sync tools and version control do not honour that lock: ordinary overlapping saves are detected where possible, but arbitrary simultaneous external writes cannot be made transactional. Avoid a checkout, merge or sync during a tool write. Run `hyp check` after one.

Archive is recoverable; deletion is explicit and limited to unreferenced archived records. Stored bytes are retained when their data record is deleted, rather than garbage-collected automatically.

## Using hyp with Git (optional)

hyp never runs `git` and does not need a repository; a plain directory is the normal case. It is built to sit well in one:

- One file per record, named by its ID, so concurrent work touches different files.
- Deterministic serialization, and a write rewrites only the records it changes, so diffs show real changes.
- IDs are UUIDs, so records created on different branches or machines do not collide.
- No generated index or cache files under `hyp/`; everything there is authoritative.
- `.hyp/` (lock and journal) ignores itself.

Committing, pushing and resolving conflicts are yours. Commit the complete `hyp/` directory when you want a history checkpoint. After a merge, checkout or sync, run `hyp check`: it reports broken references and malformed files, with how to repair a broken reference or a dependency cycle, and `hyp list --needs-review` shows hypotheses whose current assessment was based on records that have since changed. Two assessments of the same hypothesis made on different branches both remain current heads after the merge; record a new assessment to reconcile them.

## Exports

```bash
hyp export --format markdown --output notebook.md
hyp export --format json --output notebook.json
hyp export --format html --output notebook.html
hyp graph --focus H-… > graph.mmd
```

The HTML export embeds the current dataset and all assets and is an offline, read-only copy of the WebUI. It contains sources, notes and observations; share it deliberately. The graph command emits Mermaid source. Exported reports do not embed the stored bytes of data records; the JSON and HTML exports include the text previews the WebUI shows (`previews`, the first 4 KiB of each `text/*` data record).

## Development

The Cargo workspace currently has one package, with clear library modules rather than four separately versioned crates:

- `model` — types, constraints, relationships and derived assessment state.
- `store` — Markdown persistence, locking, transactions and recovery; stored bytes of data records and the schema-3 migration of attachments.
- `data` — pure helpers for data records: size limit, media types, previews.
- `cli` — clap commands, JSON output and exports.
- `show`, `status` — the plain text of `hyp show`, and `hyp status` (its report and text).
- `web` — Axum API, embedded assets, local-request guards and SSE.
- `agents` — installs the agent skill, whose text is `agents/hyp/SKILL.md` (embedded at build time). The bash blocks of its `## Example` section run as a test (`skill_example_runs_as_written_and_ends_assessed_and_closed` in `tests/cli.rs`): keep them runnable, with IDs captured in shell variables rather than placeholders.
- `web/` — dependency-free browser interface. No npm runtime dependency.

Development recipes live in the `Justfile` and run inside the flake dev shell, which provides `just`, the Rust toolchain, Node and `jsdom`:

```bash
nix develop -c just          # list recipes
nix develop -c just e2e      # Rust tests plus the jsdom UI test
```

The Rust tests cover semantic workflows, stale writes, concurrent writers, invalid transaction rollback, crash recovery, immutable histories, frozen experiment plans, source-change review tracking, symlinks, captured data and the migration of attachments, export escaping, HTTP guards and agent-skill installation.

`scripts/dom-test.cjs` drives the real UI forms, HTTP server and SSE in `jsdom`, without a rendering engine; it does not verify visual layout. It runs in `just e2e` and as the `e2e-dom` flake check. Its npm dependencies are pinned in `scripts/package-lock.json` and built by the flake; do not `npm install` them into the tree.

An optional browser suite is in `scripts/browser-test.cjs`. It is not yet wired into the flake (HYPO-0017): install Playwright/Chromium separately and run `node scripts/browser-test.cjs`. Set `HYP_BIN` to test a packaged executable and `CHROMIUM_PATH` to use a system Chromium.

## Scope of this release

This is a local, single-worktree tool. It supports multiple CLI processes and browser tabs, not networked multi-user collaborative editing. It reads the notebook into memory and rescans the record files on each read (stored bytes only by metadata; `hyp check` hashes them); it is intended for small and medium research/debugging notebooks, not millions of evidence records. Full snapshot refreshes favour correctness and simplicity over incremental-index complexity.

hyp itself makes no judgments: agents and humans record them. No automated experiment execution, Bayesian scoring, MCP server, remote hosting, user accounts or statistical-analysis engine are included.

See [VALIDATION.md](VALIDATION.md) for the checks run on this release.

## When not to use hyp

hyp records an investigation; it does not carry one out, and it holds tentative claims, not settled ones. Its technical limits (local and single-worktree, small and medium notebooks, no experiment execution, scoring or accounts) are in [Scope of this release](#scope-of-this-release). It is also the wrong tool for:

- **Statistical analysis.** Use a statistics environment and cite its output as evidence (`hyp capture` keeps the bytes). A confidence in hyp is a number someone entered, not a computed one.
- **Automated hypothesis testing.** Agentic falsification frameworks design and run the experiments; hyp can record their results.
- **Literature search or hypothesis generation.** hyp neither searches sources nor proposes claims.
- **Debate and argument mapping.** Argument-mapping tools model premises, objections and the structure of an argument; hyp models claims, evidence and judgments in one investigation.
- **A shared knowledge base or agent memory.** hyp has no retrieval and no access control. A claim that is no longer tentative belongs in documentation, a specification or a test (see [Conceptual model](#conceptual-model)).
- **Publishing a one-off evidence map.** When one question is settled in one sitting and the result is a document for readers, an evidence-map tool fits better than a notebook with a lifecycle and review tracking.
- **Plans and tasks.** Use a task tracker such as backlog.md. A hyp experiment is the procedure for testing a claim, not a work item.

## Conceptual model

Records kept beside code differ by *direction of fit*: when record and world disagree, which one must change? Zave and Jackson draw this line for software between indicative statements, the environment as it is, and optative ones, the environment as we want it (“Four Dark Corners of Requirements Engineering”, ACM TOSEM 6(1), 1997); philosophers call the same distinction direction of fit, after Anscombe's shopping list (*Intention*, 1957) and Searle's words-to-world and world-to-words directions ("A Taxonomy of Illocutionary Acts", 1975; in *Expression and Meaning*, 1979).

| Kind         | Holds                               | Direction of fit                         | Example                   |
| ------------ | ----------------------------------- | ---------------------------------------- | ------------------------- |
| Definitional | What there is: ontology, vocabulary | A definition fixes the terms             | glossary, schema          |
| Descriptive  | Observations, hypotheses, facts     | Record to world: the record is corrected | **hyp**                   |
| Normative    | Specifications, invariants          | World to record, for as long as it holds | specs, tests, type checks |
| Imperative   | Tasks: what to do                   | World to record, until it is done        | backlog.md                |

Normative and imperative records share the world-to-record direction and differ by persistence: a specification keeps holding, a task closes.

Descriptive claims also differ in epistemic status:

```text
observation -> hypothesis -> corroborated -> fact
                          \-> refuted
any of these -> stale, when what it rested on changes
```

An observation is a dated record of something seen. A hypothesis is a claim that would explain observations or predict new ones. Checks move it: each is an evidential event (an experiment run, an observation linked to a criterion or prediction) followed by a judgment that cites it. A hypothesis that survives checks that could have refuted it is corroborated; one whose check meets a falsification criterion is refuted; a claim corroborated well enough that people stop questioning it is treated as a fact.

hyp covers the descriptive kind up to corroboration. Observations are evidence (`hyp observe`; captured bytes are data records), hypotheses carry their criteria and predictions, experiments and runs are the checks, and assessments are the judgments: `supported` for corroborated, which never means proven, and `falsified` for refuted. Staleness shows as **needs review** when anything an assessment's basis holds changes; a world that changes without a new record goes unnoticed. Facts are out of scope: hyp has no "true" state and closing an investigation declares nothing, so a claim that has become a fact moves out, into documentation or into a test, where its direction of fit turns normative.

## Related tools

Checked against each source in October 2026. [docs/related-tools.md](docs/related-tools.md) compares these and other tools, with strengths and weaknesses against hyp.

- [backlog.md](https://github.com/MrLesk/Backlog.md) (MIT), the model for hyp's shape: Markdown files in any directory, a CLI for agents, a browser view for humans, agent instructions installed by the tool. It is imperative (what to do); hyp is descriptive (what is believed, and on what evidence).
- Analysis of Competing Hypotheses (Richards J. Heuer Jr., *Psychology of Intelligence Analysis*, CIA Center for the Study of Intelligence, 1999, chapter 8), the methodological precedent: weigh every observation against every rival hypothesis and look for the evidence that refutes, not the evidence that fits. hyp's evidence matrix compares hypotheses the same way; hyp adds explicit falsification criteria and records the judgments.
- [POPPER](https://github.com/snap-stanford/POPPER) (Huang et al., *Automated Hypothesis Validation with Agentic Sequential Falsifications*, 2025), an agentic falsification framework: LLM agents design and run falsification experiments under statistical error control. It performs the testing; hyp records an investigation and executes nothing.
- [Doubt](https://github.com/alsoleg89/doubt) (MIT), the closest evidence model: an agent skill and CLI that turn one contested question into a source-grounded evidence map with supporting, contradicting, qualifying and missing evidence. A map is a document per question; hyp is a notebook kept through an investigation, with falsification criteria, experiments and judgments that are flagged when their basis changes.
- [Argdown](https://github.com/argdown/argdown) (MIT), argument mapping: a plain-text syntax and tools that turn pros, cons and premise-conclusion structures into argument maps. It models the structure of an argument; hyp models evidence and judgments about a tentative claim.

## License

MIT, see [LICENSE](LICENSE).
