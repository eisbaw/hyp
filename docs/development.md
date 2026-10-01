# Development

No account, cloud service, database server, telemetry, CDN or JavaScript build step. One executable serves its embedded HTML/CSS/JavaScript assets. The application and all validation/storage logic are Rust.

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

`scripts/browser-test.cjs` runs the UI in headless Chromium with Playwright: `just browser-test`, and the `e2e-browser` flake check. The flake provides the browser (`PLAYWRIGHT_BROWSERS_PATH`); the npm `playwright` in `scripts/package.json` must match the nixpkgs `playwright-driver` version, so pin both together after a nixpkgs update. Set `HYP_BIN` to test a packaged executable.

`tests/properties/` holds property tests (proptest, stable Rust) of the record format, validation, commits, crash recovery and the review fingerprint. `just test` and the flake check run a capped number of cases per property (8 to 256) with a fixed seed, so they are reproducible; `just fuzz [SECONDS] [CASES]` runs many cases with fresh seeds. A failing case is saved under `tests/proptest-regressions/` and replayed first; commit it with the fix.
