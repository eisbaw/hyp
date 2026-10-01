# Validation — as of 9a62a67

As of commit `9a62a67` (lock-light reads, HYPO-0004): what was checked for hyp at that commit and the 0.2.0 and 0.3.0 commits before it, and what was not. Later commits are not covered here. The crate version at `9a62a67` is 0.3.0, as at `18c2a3f`, although the read locking changed in between.

Each code commit's gate is recorded in Git notes, where they are available: `git log --notes=verification --oneline`, or `git notes --ref=verification show <commit>`.

All checks ran on x86_64 Linux with the flake's pinned toolchain (rustc 1.95.0, Node 24 for jsdom).

## Checked

- `just fmt-check` and `just lint` (clippy on all targets, warnings as errors): pass at `9a62a67`.
- `just e2e` at `9a62a67`: passed in the final review round, twice, plus extra runs of the read-concurrency tests. It runs every Rust test (domain and storage workflows; the real binary through the CLI, including agent-skill installation in a plain directory and the skill's example flow; HTTP/SSE and request guards; captured data and the schema-3 migration of attachments, including two copies of a notebook migrating to identical files; concurrent reads and writes against the new locks) and then `scripts/dom-test.cjs`, which drives the real UI forms, server and CLI in jsdom: form creation, CLI-to-UI SSE updates, editors, stale-form rejection and draft preservation, criteria, evidence links, falsification assessments, the navigation views, malformed-file recovery and the self-contained offline export.
- `nix flake check -L --option fallback true` at `9a62a67`: the package build and its tests, clippy, rustfmt and the jsdom test (`e2e-dom`) against the packaged binary pass on x86_64-linux.
- By hand, on earlier commits: raising a notebook's schema, the refusal of a newer schema, `not_found` and the stricter `falsified` rule (`25fde81`, 0.2.0); the observation-first flow, `hyp observe` then falsify then explain again (`ad113d5`); the guard against writing output into `.hyp/` (`18c2a3f`, 0.3.0).
- Review of `25fde81`, `ad113d5`, `18c2a3f` and `9a62a67`: QA and architecture review agents gave a go after fix rounds. For `9a62a67` an adversarial test agent found writer starvation and a stall behind long reads in the new locking; both were fixed and confirmed before the commit.

## Not checked

- **Cross-model review.** No Codex review ran for `25fde81`, `ad113d5`, `18c2a3f` or `9a62a67`: the configured Codex model was unavailable to the account. All reviews of these commits were by Claude agents.
- **aarch64-linux.** The flake declares packages, app, dev shell and checks for it, but no ARM build is recorded; `nix flake check` builds only the current system.
- **A real browser.** `scripts/browser-test.cjs` (Playwright/Chromium) has never completed: under 0.1.0 Chromium could not start in the sandbox, and it has not been run since. No claim is made about layout, responsive rendering or real-browser behavior; the jsdom test has no rendering engine and does not replace that check.
- **jsdom recovery flake.** After `9a62a67` was committed, the jsdom test on the `9a62a67` binary failed intermittently under heavy parallel load (several worktrees building at once), at one of its two recovery waits: after a malformed file is restored, or after `hyp/` comes back. It passed in other runs and in `nix flake check`. The cause is being investigated in the WebUI work.
- **NFS.** The documented behavior of reads on a read-only NFS mount (no `gate.lock`, so no writer precedence) is untested.
- **Mixed versions.** Running a build before `9a62a67` alongside one from `9a62a67` on one notebook is documented as unsafe and was not exercised.

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
