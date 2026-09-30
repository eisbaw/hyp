# hyp

**A place to change your mind.** A hypothesis notebook for coding and research agents. When an agent suspects a cause, hyp gives it a place to record the claim, state what would falsify it, plan experiments, cite what it observed and record an explicit judgment, instead of declaring victory early. Humans inspect and steer through a live WebUI.

It keeps everything as Markdown files in a plain directory. Git is not needed, but the files are Git-friendly (see [Using hyp with Git](#using-hyp-with-git-optional)).

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

hyp ships a skill that teaches coding agents the method (record the hypothesis before acting on it, write the falsification criterion first, cite evidence, assess only with evidence and a rationale) and the commands, exit codes and conflict handling:

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
hyp evidence add H-… "Timeout at transfer 8,142" \
  --against --source logs/run-142.txt --locator "lines 81–96" \
  --reason "A failure with cache disabled contradicts the cache explanation."
hyp run X-… "Run 142" --outcome observed --evidence E-…
hyp --json show H-…          # review .related, .runs, .evidence; note .state.review_token
hyp assess H-… --reviewed <token> --status weakened --confidence 0.2 \
  --evidence E-… --reason "Check the timing confound before rejecting the hypothesis."
hyp assess H-… --reviewed <token> --status falsified --criterion F-… \
  --evidence E-… --reason "Controlled replication satisfies the rejection criterion."
hyp show H-…                 # a summary with the review token; --json for everything
hyp list --needs-review      # hypotheses by default; --kind KIND or --all for others
hyp search "cache"
hyp check
```

Use `hyp evidence add --qualifies` for an observation that limits the claim. Without either `--against` or `--qualifies`, it creates a supporting interpretation. Evidence can be reused:

```bash
hyp link E-… H-… --relation supports --reason "Why this observation matters"
hyp link H-… H-… --relation competes-with --reason "Alternative explanation"
hyp gap H-… "Does disabling cache alter DMA timing?"
hyp set G-… --resolved true
hyp evidence attach E-… ./capture.txt
hyp experiment add H-… "Replication" --targets P-…,F-…
hyp set X-… --experiment-status running
hyp edit H-…                 # $VISUAL, then $EDITOR, then vi
hyp archive H-…
hyp restore H-…
hyp delete H-…               # only archived and unreferenced records
```

`depends-on`, `competes-with`, and `supersedes` are CLI relation values. Serialized files use underscores. Dependencies and supersession cannot form cycles. Competing hypotheses may be linked in either direction; the relation does not imply mutual exclusivity.

Use `-` for a text argument to read stdin. Flags accept literal multiline values. All ordinary commands support `--json`. `hyp apply` accepts a JSON array of create/update/archive/delete changes on stdin, with optional `--expected-revision` for a whole-project precondition.

`hyp list` lists hypotheses, each with its judgment, lifecycle and a `needs-review` marker; `--kind KIND` lists another kind and `--all` every kind. A `--status` or `--needs-review` the listed kind cannot have (`--status planned` without `--kind experiment`) is an argument error (exit `2`), not an empty list.

### Machine contract

Agents depend on these; they are kept stable.

| CLI exit code | Meaning                                                                                                                                             |
| ------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| `0`           | Success.                                                                                                                                            |
| `1`           | Any other error: invalid input, an unknown or ambiguous ID, a missing or malformed precondition, project errors, I/O. Retrying unchanged will not help. |
| `2`           | Invalid command-line arguments.                                                                                                                     |
| `3`           | Conflict: something the write depended on changed since it was read. Nothing was written. Re-read, review, retry.                                  |
| `141`         | Killed by SIGPIPE: stdout was closed early (`hyp list \| head`). A write command prints after writing, so its write may already be on disk.           |

Write commands (`add`, `evidence add`, `assess`, `set`, `apply`, `evidence attach` and the rest) print the full ID of each record their changes named, one per line and in order (`evidence add`: the evidence, then its link; `evidence attach`: the evidence). With `--json` they print `{"written": [{"id": "H-…", "kind": "hypothesis", "revision": "…"}], "revision": "…"}`: each record's revision after the write (`null` once deleted), usable as the `expected_revision` of a next change, and the project revision (what `apply --expected-revision` takes). `hyp --json init` prints the same shape, listing every record of the new project.

Errors go to stderr, with `--json` as `{"error": "…"}`. A conflict's message starts with `conflict:`. The WebUI's HTTP API answers `403` for a missing or invalid request token or an untrusted `Host`/`Origin`, `409` for a conflict, `422` for input the domain rules reject, and another `4xx` for other rejected input (malformed JSON, an oversized body, a wrong content type).

### Preconditions

Every change states what it depends on, as read from a snapshot. `hyp --json show ID` gives a record's `entry.revision` and the records that refer to it (`related`); for a hypothesis also its `state` and `basis`, what its fingerprint covers (see [The model](#the-model)), with the `runs` and `evidence` in it in full. `hyp export --format json` gives everything. A change that depended on something that changed is rejected; a change that did not is unaffected by other writers.

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

A statement that no longer holds (the record changed or was deleted) is a conflict: exit `3`, HTTP 409, and the message names what changed. Re-read, review what changed, then retry. A missing or malformed statement is an ordinary error (exit `1`, HTTP 422) naming the field; retrying it unchanged will not help. Records created earlier in the same batch need no statement.

An assessment may cite only evidence with an active link to its hypothesis, or to one of its active criteria or predictions (`hyp link E-… H-… --relation supports --reason "…"`). Citing other evidence, or a judgment other than `untested` without evidence, is an ordinary error. In an `apply` batch, the assessment's basis may grow only by records the batch creates: to bring in evidence that already existed, link it in an earlier write, re-read, then assess.

The CLI commands state preconditions from their own read, so they protect only the moment between that read and the write. `hyp assess` is the exception: it requires `--reviewed` with the `review_token` from the `state` you reviewed (`hyp --json show H-…` or `hyp --json list`). If the hypothesis's basis or its current assessments changed since, it writes nothing and exits `3`; a malformed token exits `1`. To protect a longer window for other records, use `hyp apply`. The WebUI states everything as of the moment a form was opened.

## The model

| Record     | Purpose                                                                        |
| ---------- | ------------------------------------------------------------------------------ |
| Hypothesis | Claim, scope, assumptions, lifecycle, tags                                     |
| Prediction | Expected observable result, conditions, owning hypothesis                      |
| Criterion  | Observation that would falsify the scoped claim                                |
| Evidence   | Observation, source, locator, date, hashed attachments                         |
| Link       | Interpretation: supports, contradicts, qualifies; or a hypothesis relationship |
| Experiment | Procedure, status, frozen hypothesis/prediction/criterion references           |
| Run        | Immutable snapshot of the experiment plan, outcome and evidence IDs            |
| Assessment | Immutable judgment, subjective confidence, rationale and evidence IDs          |
| Gap        | Missing information, open or resolved                                          |

Predictions and criteria are separate Markdown records, which makes them individually addressable and avoids rewriting the hypothesis every time one is added.

Lifecycle is **draft / investigating / paused / closed**. Assessment is **untested / inconclusive / supported / weakened / falsified**. Closing an investigation never declares its hypothesis true.

- Drafts may be incomplete. Investigating requires an active criterion or an explicit `untestable_reason`.
- Every assessment needs a rationale. Every judgment except untested must cite evidence linked to the hypothesis, and falsification also a criterion belonging to it. Whoever records the assessment, agent or human, judges whether the observation actually satisfies it.
- Confidence is optional, subjective, and in `[0, 1]`. Evidence counts never calculate it.
- An assessment records the hypothesis's fingerprint: the SHA-256 of its `basis` (`hyp --json show`) as compact JSON with sorted keys. The basis holds content only: the claim (title, body, scope, assumptions, archived); its criteria and predictions (title, body, conditions, archived); links touching the hypothesis or those (ends, relation, reason, archived), so another hypothesis counts only through its link; the evidence with an active link to the hypothesis or an active criterion or prediction; and runs of its experiments (title, body, outcome, cited evidence) with the evidence they cite. Evidence counts with its provenance (title, body, source, locator, attachment hashes, archived). A change to it shows **needs review** without rewriting the judgment. Lifecycle, tags, the untestable reason, experiments, gaps and timestamps are not part of it, so closing a hypothesis does not flag it; archiving it does.
- A new assessment supersedes the current assessment heads. Divergent heads after any merge or sync require explicit reconciliation; neither silently wins by timestamp.
- Experiments freeze complete target content and revisions at creation. Runs freeze the complete experiment plan at execution-record creation. `hyp run` records an execution; it does not execute commands.
- Historic assessments and runs cannot be edited or deleted through the tool. To keep a history of all file edits, use any version control, e.g. Git. There is no claim of tamper-proof auditing.

## WebUI

Run `hyp web [--port 7432]`. The server binds to IPv4 loopback only. Browse manually to the printed URL; it does not automatically launch a browser.

- Hypothesis overview: search, assessment/tag filters, needs-review and archived records.
- Hypothesis detail: falsification criteria, predictions, positive/negative/qualifying evidence, experiments, gaps and assessment history.
- Experiment queue and immutable runs.
- Reusable evidence and interpretations.
- Evidence matrix to compare alternative hypotheses.
- Focused relationship graph with navigable nodes.
- Create/edit/archive/restore/delete forms, plus advanced JSON editing for mutable records.
- Incoming changes preserve dirty forms; conflicting saves are rejected and the draft remains available to copy/reconcile.
- External editor and CLI saves update open tabs using filesystem notifications and SSE. A two-second reconciliation scan catches missed notifications. Reconnects fetch a full snapshot.
- Malformed files show diagnostics and block writes. An already open browser preserves the last readable state and marks it stale. Errors between records (see "Files and concurrency") show as a notice with their repair; saving still works unless it adds a new error.

Notes are displayed as escaped, pre-wrapped text. Markdown is retained in files and exports; the UI does not execute raw HTML. Binary attachments are copied through the CLI and can be inspected as metadata in the WebUI. They are not served as executable browser content.

## Files and concurrency

All authoritative content is under `hyp/`:

- `config.toml` — project name and schema version.
- `hypotheses/`, `predictions/`, `criteria/`, `evidence/`, `links/`, `experiments/`, `runs/`, `assessments/`, `gaps/` — one `<full-id>.md` per record.
- `assets/` — optional SHA-256-addressed attachments (maximum 32 MiB per imported file).
- `.hyp/` at the project root — write lock and temporary recovery journal; not project data. It contains a `.gitignore` that ignores it.

Markdown files have YAML front matter and an ordinary notes body. IDs are UUIDs with readable type prefixes. Renaming files independently of IDs is rejected. Unknown schema fields, broken references and invalid states are reported by `hyp check`. `hyp check --strict` also fails on warnings such as a hypothesis with neither a falsification criterion nor an untestable reason.

`hyp check` reports every rule a record breaks, one diagnostic each. Each has a stable `code` (`hyp --json check`); `(path, code)` identifies it, and the message may change. Two kinds of error differ in what they block:

- `malformed` (a file hyp cannot load: bad front matter, a filename or directory that does not match the record, a duplicate ID), `attachment` (missing, unsafe or changed) and `invalid` (a record breaking rules of its own fields, including a reference by short ID or to a record of the wrong kind, which the ID prefix gives) block every write (`blocks_writes: true`). Fix the file by hand, or restore it.
- `dangling_reference` (a referenced record does not exist), `cycle` (`depends_on` or `supersedes` links, or assessment supersession) and `inconsistent` (other rules between records, such as an investigating hypothesis without an active criterion) are errors between loaded records, as a merge, sync or hand edit leaves them. They do not block writes: a write is rejected if, for any record, it adds a `(path, code)` the project did not already have. So an unrelated write still works, and so does the write that repairs them.

Where hyp knows a repair, the diagnostic carries it: `--json` as `"repair": {"note": "...", "commands": [["hyp", "archive", "L-..."], ...]}` (`note` may be null, `commands` empty), and plain `hyp check` as `note:` and `repair:` lines. Run the commands in order, as argv arrays, in the project directory (they carry no `--project`). A cycle's repair archives the link. For a dangling reference the note comes first: restore the missing record from the source of the merge or sync. That loses nothing, and after a partial sync the record may simply not have arrived yet. Only a link also gets commands (archive, then delete) for when the record is gone for good; a delete cannot be undone without version control. Other records whose referenced record is missing get only the note.

A process-shared advisory lock serializes tool writes. Each change carries an optimistic precondition on what it depends on (see above), so stale writes are rejected without turning unrelated concurrent writes into conflicts. Atomic file replacements and an fsynced roll-forward journal recover interrupted multi-file operations. Reads through hyp recover pending transactions under the same lock. Manual editors, sync tools and version control do not honour that lock: ordinary overlapping saves are detected where possible, but arbitrary simultaneous external writes cannot be made transactional. Avoid a checkout, merge or sync during a tool write. Run `hyp check` after one.

Archive is recoverable; deletion is explicit and limited to unreferenced archived records. Unreferenced assets are retained rather than garbage-collected automatically.

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

The HTML export embeds the current dataset and all assets and is an offline, read-only copy of the WebUI. It contains sources, notes and observations; share it deliberately. The graph command emits Mermaid source. Exported reports do not embed binary evidence attachments.

## Development

The Cargo workspace currently has one package, with clear library modules rather than four separately versioned crates:

- `model` — types, constraints, relationships and derived assessment state.
- `store` — Markdown persistence, locking, transactions and recovery.
- `cli` — clap commands, JSON output and exports.
- `web` — Axum API, embedded assets, local-request guards and SSE.
- `agents` — installs the agent skill, whose text is `agents/hyp/SKILL.md` (embedded at build time).
- `web/` — dependency-free browser interface. No npm runtime dependency.

Development recipes live in the `Justfile` and run inside the flake dev shell, which provides `just`, the Rust toolchain, Node and `jsdom`:

```bash
nix develop -c just          # list recipes
nix develop -c just e2e      # Rust tests plus the jsdom UI test
```

The Rust tests cover semantic workflows, stale writes, concurrent writers, invalid transaction rollback, crash recovery, immutable histories, frozen experiment plans, source-change review tracking, symlinks, attachments, export escaping, HTTP guards and agent-skill installation.

`scripts/dom-test.cjs` drives the real UI forms, HTTP server and SSE in `jsdom`, without a rendering engine; it does not verify visual layout. It runs in `just e2e` and as the `e2e-dom` flake check. Its npm dependencies are pinned in `scripts/package-lock.json` and built by the flake; do not `npm install` them into the tree.

An optional browser suite is in `scripts/browser-test.cjs`. It is not yet wired into the flake (HYPO-0017): install Playwright/Chromium separately and run `node scripts/browser-test.cjs`. Set `HYP_BIN` to test a packaged executable and `CHROMIUM_PATH` to use a system Chromium.

## Scope of this release

This is a local, single-worktree tool. It supports multiple CLI processes and browser tabs, not networked multi-user collaborative editing. It reads the notebook into memory and rescans files; it is intended for small and medium research/debugging notebooks, not millions of evidence records. Full snapshot refreshes favour correctness and simplicity over incremental-index complexity.

hyp itself makes no judgments: agents and humans record them. No automated experiment execution, Bayesian scoring, MCP server, remote hosting, user accounts or statistical-analysis engine are included.

See [VALIDATION.md](VALIDATION.md) for the checks run on this release.

Licensed under MIT.
