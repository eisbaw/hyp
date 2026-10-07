# Using hyp

The command line in detail. The [README](../README.md) has a short first session; [agents-contract.md](agents-contract.md) has the `--json`, exit-code and precondition contract.

## Install and run

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
nix flake check                # package tests, formatting, clippy, the jsdom and (not on macOS) browser UI tests
nix run . -- --help
```

The flake pins nixpkgs and supports `x86_64-linux`, `aarch64-linux`, `x86_64-darwin` and `aarch64-darwin`. All Cargo dependencies are pinned in `Cargo.lock`. A first build requires network access or a populated Nix cache; the installed application works offline.

Or build directly with a recent Rust toolchain:

```bash
cargo build --release --locked
./target/release/hyp --help
```

Point the executable at another project with `--project /path/to/project`. It also discovers the nearest ancestor containing `hyp/config.toml`.

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

## Evidence, links and reuse

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

## Starting from an observation

An investigation can also start from an observation: something seen before anyone suspects why. `hyp observe` records it as evidence without links and prints its `E-` ID; `hyp add --explains E-…` then creates a hypothesis and, in the same write, a `supports` link from each named observation to it, so the observation is in its basis and citable. Add each serious explanation this way and name earlier ones with `--competes-with H-…` (a `competes-with` link from the new hypothesis; its reason names the observations both explain). Then continue as above: a criterion for each, the experiment that could falsify one.

```bash
hyp observe "Login test fails 23/200 on CI" --source "CI job 4402" \
  --locator "test login" --observed-at 2026-09-12   # prints E-…
hyp add "A shared temp dir causes it" --explains E-…  # H-…, then its link L-…
hyp add "Clock skew causes it" --explains E-… --competes-with H-… \
  --reason "Timestamps jump between the failing steps"
hyp show E-…                 # the hypotheses it bears on, and how
```

`--observed-at` (on `observe`, `evidence add` and `hyp set E-…`) takes an RFC 3339 timestamp or a date (`YYYY-MM-DD`) and is stored as given, or `''` for unknown (stored empty); anything else is an argument error (exit `2`). Without it, `observe` and `evidence add` store the time of recording. Every writer (`hyp apply`, the WebUI too) is held to the same rule when it stores a new or changed value, and a value more than a day in the future is refused as well: kind `invalid_input` (exit `1`), with a `bad_observed_at` diagnostic. A value already stored is never refused: older versions stored the WebUI's free text as given ("last tuesday night", a `datetime-local` value without an offset), and nothing hyp derives depends on it. `hyp check` reports such a value as the warning `bad_observed_at` (it does not block writes; `--strict` fails on it), whose repair command keeps the text in the body and clears the field in one write: `hyp set E-… --body=<the body, a blank line, "Observed at (as recorded): last tuesday night"> --observed-at=` (exact as the argv of `--json`; plain `hyp check` prints it shell-quoted). It holds the body as that check read it and states no revision, so run `hyp check` again right before running it; if you know the time, `hyp set E-… --observed-at 2026-09-12` instead. The body is in the review basis, so running the repair changes the review token of every hypothesis that evidence is in the basis of (those `hyp show E-…` lists under *Bears on*, and those with an experiment whose run cites it): each one with an assessment then needs review, and a `--reviewed` token taken before the repair conflicts. `--explains` takes evidence IDs and `--competes-with` hypothesis IDs, comma-separated or repeated; an ID given twice gets one link. Without `--reason`, each `--explains` link's reason is "Proposed as an explanation of this observation". `hyp show E-…` lists under *Bears on* each hypothesis the observation is linked to, directly or through an active criterion or prediction (a hypothesis whose experiment's run cites it also has it in its basis, but is not listed), with its stance (`for`, `against`, `qualifies`, `mixed`), the hypothesis's current judgment and lifecycle, and each link's meaning, and says when it is unexplained; `hyp --json show E-…` gives the same as `bears_on: [{"hypothesis", "archived", "judgment", "stance", "bearings"}]` and `unexplained` (both `null` for other kinds).

An observation is unexplained until a live hypothesis (not archived, current judgment not falsified) accounts for it: it counts for or qualifies that hypothesis through an active link, to the hypothesis or to an active criterion or prediction of it. So an observation is unexplained again once every explanation of it is falsified or archived; one that only counts against live hypotheses, or only falsified one, is unexplained too (link it to the hypothesis it bears on); and so is evidence that only a run or a gap names. The observation an explanation was made to fit cannot tell rival explanations apart: decide between them with the experiment that could falsify one.

## Data records

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

`hyp capture FILE` (or `-` for stdin, since hyp never runs commands itself) stores the bytes once per SHA-256 at `hyp/assets/<sha256>`, at most 32 MiB; empty input is an error unless `--allow-empty` (an empty pipe usually means the command before it failed). It records a data record: `title` (default: the file name, for stdin the origin's first line), `origin` (required, free text: a path, URL, host or the command that produced the bytes), `captured_at` (set by hyp), `media_type` (`--media-type`, else guessed: a few common file extensions, then `text/plain` for UTF-8 text, else `application/octet-stream`), `size`, `sha256`, and the body as an optional note (`--body`). A data record cannot be changed afterwards (`set`, `edit`, update and patch are refused); it can be archived, and deleted once archived and no record references it (the error lists the referrers). Capturing the same bytes again makes a new record that shares the stored file; repeating a capture with the same bytes, title, origin, media type and note (a retried command, or the same capture run twice at once) prints the existing ID and writes nothing. `hyp data get --output FILE` writes the file atomically and refuses a path inside `hyp/` or `.hyp/`. `--data` (comma-separated or repeated) is accepted by `observe`, `evidence add`, `add` (on the hypothesis), `predict`, `falsify-if`, `gap`, `run` and `assess`, and by `hyp set` for records that can change; in `hyp apply` every record takes `"data": ["D-…"]`, batch-local references included. `hyp evidence attach E-… FILE` is capture plus reference: it reuses a data record the evidence references, the one the [schema-3 migration](storage.md#schema-versions) makes when a legacy attachment holds the same bytes, or any with the same bytes, before capturing a new one (origin: the path as given), and prints the evidence's ID, then the data record's. Evidence that already holds the bytes is left unchanged (`no changes`); when it holds them as a legacy attachment, only the evidence's ID is printed, as no data record exists yet.

## Relations, stdin and batches

`depends-on`, `competes-with`, and `supersedes` are CLI relation values, and plain output (`list`, `show`, `graph`, Markdown export, error messages) spells them so. Files and JSON use the serialized form with underscores (`competes_with`). Dependencies and supersession cannot form cycles. Competing hypotheses may be linked in either direction; the relation does not imply mutual exclusivity.

Use `-` for a text argument to read stdin. Flags accept literal multiline values. A title is one line: with `-`, the first line of stdin is the title and the rest is appended to the body (after a blank line if the body is not empty); a given title with a newline is an error, and `hyp check` reports a stored one. All ordinary commands support `--json`. `hyp apply` accepts a JSON array of create/patch/update/archive/delete changes on stdin, with optional `--expected-revision` for a whole-project precondition; `hyp apply --help` shows each change with examples. A create needs only what the matching command asks for: omitted fields get the command's defaults (a hypothesis is a draft, an experiment is planned and targets its hypothesis, a run is observed, a gap open), and the server sets the timestamps. A patch (`{"op": "patch", "id", "expected_revision", "set": {field: value}}`) changes only the fields in `set`, validated as an update; it cannot change `id`, `kind` or `created_at`, and assessments, runs and data records cannot be patched. A data record can also be created in a batch (`"kind": "data"`, `title`, `origin`, `sha256`, optional `media_type`) for bytes hyp already stores (as `hyp capture` stored them); the server sets `captured_at` and `size`. A create may give `"id": "@name"`: later changes in the same batch write `"@name"` in the record fields that take an ID (`hypothesis`, `from`, `to`, `experiment`, `evidence`, `criterion`, `targets[].id`, `resolved_by`, `data`) and in the keys of `expected`, and hyp replaces it with the full ID it generates, so one batch can create a hypothesis, its criterion, an experiment, evidence, links and a run. An unknown or twice-defined reference is an ordinary error, and so is updating, patching, archiving or deleting a record the same batch creates (put its values in the create). Only one argument of a command can be `-` (stdin is read once); more is an error (exit `1`).

## At a terminal

For people at a terminal: when stderr is a terminal, a write command also prints a one-line summary there (`created evidence E-… (+ link L-…: E-… supports H-…)`), and a `-` argument read from a terminal prints how to end the input (Ctrl-D). A write that changes nothing (`hyp set H-…` without flags, restoring a record that is not archived, saving `hyp edit` unchanged) prints `no changes` to stderr, whether or not it is a terminal (not with `--json`), and exits `0`. When `hyp edit` rejects the edited record and stdin and stderr are terminals, the editor reopens with the error as `# hyp:` comment lines under the opening `---`; line numbers in it count lines of that file. Saving it unchanged, or emptying it, aborts (exit `1`) with the last error and names a copy of the text. Without a terminal (a scripted editor) the first error fails the command the same way, without reopening.

## Status and list

`hyp status` answers where the investigation stands in one read: a project line (how many open hypotheses, how many need review, whether writes are blocked and by which files, other `hyp check` findings; with none open, that `hyp list` shows the conclusions), then two lines per open hypothesis (not archived, not closed) and per closed one that needs review, those needing review first: its judgment and lifecycle, its criteria (or `no criterion`, or its untestable reason), how much evidence is linked, open gaps (and resolved ones, with the evidence that resolved them) and experiments without runs. Last come unexplained observations: evidence, not archived, that no live hypothesis accounts for (see the rule above). The project line counts them and the plain text lists the first five, then how many more. `hyp --json status` gives the same as `{"hypotheses": [row], "not_shown": {"closed", "archived", "needs_review"}, "unexplained_observations": [{"id", "title"}], "writes_blocked", "blocking": [diagnostics], "errors", "warnings", "revision"}`, each row `{"id", "title", "lifecycle", "judgment", "confidence", "needs_review", "criteria": [IDs], "untestable_reason", "missing_criterion", "linked_evidence": [IDs], "open_gaps": [{"id"}], "resolved_gaps": [{"id", "resolved_by": [evidence IDs]}], "experiments_without_runs": [IDs]}`. A row lists what it has by full ID, never as a count (the plain text counts them): `criteria` the active criteria, `linked_evidence` the evidence an assessment may cite, leaving out evidence whose file did not load (`hyp check` reports it). `not_shown.needs_review` counts archived hypotheses that need review (a closed one that needs review is listed instead); while it is not `0`, or an observation is unexplained, the project line does not call the investigation finished. It prints no review token: take that from the `hyp show` output you reviewed.

`hyp list` lists hypotheses, each with its judgment, lifecycle and a `needs-review` marker; `--kind KIND` lists another kind and `--all` every kind. A `--status` or `--needs-review` the listed kind cannot have (`--status planned` without `--kind experiment`) is an argument error (exit `2`), not an empty list.
