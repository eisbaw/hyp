# Validation — 0.3.0

This file records what was checked for hyp 0.3.0 (the version in `Cargo.toml`) and, as importantly, what was not. The code it covers is commit `9a62a67` (lock-light reads, HYPO-0004) on top of `18c2a3f` (0.3.0: data records, HYPO-0090). Commit `9a62a67` changed the read locking without raising the version, so builds of both commits report 0.3.0; the README's warning that "hyp 0.3.0 and earlier" must not share a notebook with this version refers to builds before `9a62a67`.

Each code commit's gate is recorded in Git notes, where they are available: `git log --notes=verification --oneline`, or `git notes --ref=verification show <commit>`.

All checks ran on x86_64 Linux with the flake's pinned toolchain (rustc 1.95.0, Node 24 for jsdom).

## Checked

- `just fmt-check` and `just lint` (clippy on all targets, warnings as errors): pass.
- `just e2e`: passes. It runs every Rust test (domain and storage workflows; the real binary through the CLI, including agent-skill installation in a plain directory and the skill's example flow; HTTP/SSE and request guards; captured data and the schema-3 migration of attachments, including two copies of a notebook migrating to identical files; concurrent reads and writes against the new locks) and then `scripts/dom-test.cjs`, which drives the real UI forms, server and CLI in jsdom: form creation, CLI-to-UI SSE updates, editors, stale-form rejection and draft preservation, criteria, evidence links, falsification assessments, the navigation views, malformed-file recovery and the self-contained offline export. For `9a62a67` it ran twice in the final round, plus extra runs of the read-concurrency tests. On a heavily loaded machine (load average about 20 on 14 cores, other builds running) the jsdom test later failed in about half of its runs, each time at one of its two recovery waits ("recovery" after a malformed file is restored, "recovery from unavailable project" after `hyp/` comes back), which give up after 10 seconds; the same code passed in the other runs and in `nix flake check`. The cause is not yet known.
- `nix flake check -L --option fallback true`: the package build and its tests, clippy, rustfmt and the jsdom test (`e2e-dom`) against the packaged binary pass on x86_64-linux.
- By hand: the observation-first flow (`hyp observe`, falsify, explain again); raising a notebook's schema, the refusal of a newer schema, `not_found` and the stricter `falsified` rule; the guard against writing output into `.hyp/`.
- Review: for each of these commits, QA and architecture review agents gave a go after fix rounds. For `9a62a67` an adversarial test agent found writer starvation and a stall behind long reads in the new locking; both were fixed and confirmed before the commit.

## Not checked

- **Cross-model review.** No Codex review ran for 0.2.0, `hyp observe`, 0.3.0 or the read locking: the configured Codex model was unavailable to the account. All reviews of these versions were by Claude agents.
- **aarch64-linux.** The flake declares packages, app, dev shell and checks for it, but nothing was ever built or run on ARM; `nix flake check` builds only the current system.
- **A real browser.** `scripts/browser-test.cjs` (Playwright/Chromium) has not run since 0.1.0, where Chromium could not start in the sandbox, and it is not part of the flake (HYPO-0017). No claim is made about layout, responsive rendering or real-browser behavior; the jsdom test has no rendering engine and does not replace that check.
  TODO(HYPO-0017): replace this item with the browser suite's result once it runs in the flake.
- **NFS.** The documented behavior of reads on a read-only NFS mount (no `gate.lock`, so no writer precedence) is untested.
- **Mixed versions.** Running a build before `9a62a67` alongside this one on one notebook is documented as unsafe and was not exercised.

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
- `hyp observe --observed-at` accepts dates in the future (HYPO-0093).
- Existing experiment targets and runs are preserved. hyp keeps no history of manual edits; use any version control, e.g. Git, for that.
