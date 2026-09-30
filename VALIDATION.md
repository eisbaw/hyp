# Validation — 0.1.0

Checked on x86_64 Linux.

- `cargo test --locked`: passes (domain/storage workflows, real-binary CLI tests including agent-skill installation in a plain directory, HTTP/SSE tests).
- `cargo clippy --all-targets -- -D warnings`: passes.
- `cargo fmt --check`: passes.
- `nix flake check`: package build and tests, clippy, and formatting pass using the pinned nixpkgs Rust toolchain (1.95.0).
- `scripts/dom-test.cjs`: passes against the real Rust server and CLI. Covers form creation, CLI-to-UI SSE updates, editor changes, two views, stale-form rejection and draft preservation, criteria, evidence links, falsification assessments, all navigation views, malformed-file recovery, and self-contained offline export.
- `scripts/browser-test.cjs`: included but **not completed in this environment**. Chromium failed at startup because the execution environment denied a required socket. No claim is made that pixel layout, responsive rendering, or real-browser behavior has been visually verified. The DOM test does not replace that check.
- `aarch64-linux`: the flake supplies packages, app, development shell and checks; no native ARM build was run here.

The offline HTML export can be generated from the synthetic demo for visual inspection:

```bash
mkdir /tmp/hyp-example
nix run . -- --project /tmp/hyp-example init --demo
nix run . -- --project /tmp/hyp-example export --format html --output /tmp/hyp-example.html
```

## Meaningful limits

- Manual editors, sync tools and version control do not participate in the process-shared lock. Hyp detects stale revisions and ordinary overlapping saves, but cannot guarantee atomicity against arbitrary simultaneous external file writes.
- Records are re-read into memory. There is no persistent database or distributed synchronization protocol.
- An assessment is the judgment of whoever records it, agent or human; the tool validates references and required rationale, not the scientific correctness of a conclusion.
- Existing experiment targets and runs are preserved. hyp keeps no history of manual edits; use any version control, e.g. Git, for that.
