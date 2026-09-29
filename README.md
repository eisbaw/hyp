# hyp

**A place to change your mind.** A local, Git-native hypothesis notebook with a Rust CLI and a live WebUI. Capture claims, define what would falsify them, plan experiments, preserve observations, and record explicit assessments.

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
hyp assess H-… --status weakened --confidence 0.2 --evidence E-… \
  --reason "Check the timing confound before rejecting the hypothesis."
hyp assess H-… --status falsified --criterion F-… --evidence E-… \
  --reason "Controlled replication satisfies the rejection criterion."
hyp show H-…
hyp list --kind hypothesis --needs-review
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

Use `-` for a text argument to read stdin. Flags accept literal multiline values. All ordinary commands support `--json`. Exit codes: `0` success, `1` domain/I/O failure, `2` invalid CLI arguments, `3` revision conflict. JSON errors go to stderr. `hyp apply` accepts a JSON array of transactions on stdin, with optional `--expected-revision` for a whole-project precondition; update/archive/delete operations always require an object revision.

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
- Falsification requires a rationale, evidence and a criterion belonging to that hypothesis. The human judges whether the observation actually satisfies it.
- Confidence is optional, subjective, and in `[0, 1]`. Evidence counts never calculate it.
- An assessment fingerprints the hypothesis's relevant records, interpretations and cited observations. Changes show **needs review** without rewriting the judgment.
- A new assessment supersedes the current assessment heads. Divergent heads after a Git merge require explicit reconciliation; neither silently wins by timestamp.
- Experiments freeze complete target content and revisions at creation. Runs freeze the complete experiment plan at execution-record creation. `hyp run` records an execution; it does not execute commands.
- Historic assessments and runs cannot be edited or deleted through the tool. Git is still needed to retain _all_ manual file-edit history. There is no claim of tamper-proof auditing.

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
- Malformed files show diagnostics and block writes. An already open browser preserves the last readable state and marks it stale.

Notes are displayed as escaped, pre-wrapped text. Markdown is retained in files and exports; the UI does not execute raw HTML. Binary attachments are copied through the CLI and can be inspected as metadata in the WebUI. They are not served as executable browser content.

## Files and concurrency

All authoritative content is under `hyp/`:

- `config.toml` — project name and schema version.
- `hypotheses/`, `predictions/`, `criteria/`, `evidence/`, `links/`, `experiments/`, `runs/`, `assessments/`, `gaps/` — one `<full-id>.md` per record.
- `assets/` — optional SHA-256-addressed attachments (maximum 32 MiB per imported file).
- `.hyp/` at the project root — ignored write lock and temporary recovery journal.

Markdown files have YAML front matter and an ordinary notes body. IDs are UUIDs with readable type prefixes. Renaming files independently of IDs is rejected. Unknown schema fields, broken references and invalid states are reported by `hyp check`. `hyp check --strict` also fails on warnings such as a missing falsification criterion.

A process-shared advisory lock serializes tool writes. Optimistic object/project hashes reject stale updates. Atomic file replacements and an fsynced roll-forward journal recover interrupted multi-file operations. Reads through hyp recover pending transactions under the same lock. Manual editors and Git do not honour that lock: ordinary overlapping saves are detected where possible, but arbitrary simultaneous external writes cannot be made transactional. Avoid Git checkout/merge during a tool write. Run `hyp check` after merges.

Git operations are entirely yours. The app never commits, pushes, fetches or resolves Git conflicts. Commit the complete `hyp/` directory when you want a history checkpoint. Archive is recoverable; deletion is explicit and limited to unreferenced archived records. Unreferenced assets are retained rather than garbage-collected automatically.

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
- `web/` — dependency-free browser interface. No npm runtime dependency.

Development recipes live in the `Justfile` and run inside the flake dev shell, which provides `just`, the Rust toolchain, Node and `jsdom`:

```bash
nix develop -c just          # list recipes
nix develop -c just e2e      # Rust tests plus the jsdom UI test
```

The Rust tests cover semantic workflows, stale writes, concurrent writers, invalid transaction rollback, crash recovery, immutable histories, frozen experiment plans, source-change review tracking, symlinks, attachments, export escaping and HTTP guards.

`scripts/dom-test.cjs` drives the real UI forms, HTTP server and SSE in `jsdom`, without a rendering engine; it does not verify visual layout. It runs in `just e2e` and as the `e2e-dom` flake check. Its npm dependencies are pinned in `scripts/package-lock.json` and built by the flake; do not `npm install` them into the tree.

An optional browser suite is in `scripts/browser-test.cjs`. It is not yet wired into the flake (HYPO-0017): install Playwright/Chromium separately and run `node scripts/browser-test.cjs`. Set `HYP_BIN` to test a packaged executable and `CHROMIUM_PATH` to use a system Chromium.

## Scope of this release

This is a local, single-worktree tool. It supports multiple CLI processes and browser tabs, not networked multi-user collaborative editing. It reads the notebook into memory and rescans files; it is intended for small and medium research/debugging notebooks, not millions of evidence records. Full snapshot refreshes favour correctness and simplicity over incremental-index complexity.

No AI-generated judgments, automated experiment execution, Bayesian scoring, MCP server, remote hosting, user accounts or statistical-analysis engine are included.

See [VALIDATION.md](VALIDATION.md) for the checks run on this release.

Licensed under MIT.
