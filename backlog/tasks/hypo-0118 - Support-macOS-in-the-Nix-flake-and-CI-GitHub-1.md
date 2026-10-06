---
id: HYPO-0118
title: 'Support macOS in the Nix flake and CI (GitHub #1)'
status: In Progress
assignee:
  - '@claude'
created_date: '2026-10-06 13:02'
updated_date: '2026-10-06 14:06'
labels:
  - github
  - portability
dependencies: []
references:
  - 'https://github.com/eisbaw/hyp/issues/1'
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Captured verbatim from GitHub issue #1 (https://github.com/eisbaw/hyp/issues/1), opened 2026-10-06 by @bebopsan:

---

Description:
I’d like to install and use Hyp on macOS with Nix, including Apple Silicon Macs. At present, the flake only declares Linux package outputs, so nix profile install github:eisbaw/hyp fails on Apple Silicon with a missing packages.aarch64-darwin.default.
Cargo can build the CLI on macOS, but several tests assume Linux-specific behavior. I tested a patch that addresses the flake support and those test assumptions.
Proposed changes
- In flake.nix (around line 6), add x86_64-darwin and aarch64-darwin to the systems list. The package, app, dev shell, and check outputs are already generated from that list.
- In tests/cli.rs (around lines 2359–2405), use libc::openpty on macOS to create the PTY master/slave pair, and set FD_CLOEXEC on both descriptors to preserve the existing child-process behavior. Keep the current /dev/ptmx and ptsname_r implementation for Linux.
- In tests/cli.rs (around line 448), read the umask by invoking sh -c umask instead of reading /proc/self/status.
- In tests/cli.rs (around lines 165 and 2587–2588), replace GNU-only sed -i test scripts with portable sed output redirected to a temporary file, followed by mv.
- In tests/reads.rs (around lines 479–483), compare the error path with hyp_dir.canonicalize(). On macOS, a temporary path under /var may be reported by the OS under /private/var.
- In .github/workflows/ci.yml (around lines 19–25), run the existing nix flake check -L job on ubuntu-latest, macos-15 (ARM64), and macos-15-intel.
Validation
On an Apple Silicon Mac, the full Rust test suite passed, with 198 tests passing and one existing property test ignored. The native nix flake check also passed its five checks: package, Clippy, formatting, DOM, and browser. The flake outputs evaluated for both Darwin architectures.

---

Triage note (not part of the issue): current master has two more GNU `sed -i` uses in tests/cli.rs than the issue lists (the "Kept edit" / "Fixed" editor scripts); grep `sed -i` in tests/ for the full set. Darwin outputs also include the browser check, which is unproven on macOS CI runners.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 flake.nix exposes packages, apps, devShells and checks for x86_64-darwin and aarch64-darwin
- [x] #2 umask test reads the umask without /proc/self/status
- [x] #3 No test script relies on GNU-only sed -i
- [x] #4 Path assertions in tests/reads.rs pass when the temp dir is behind a symlink (/var -> /private/var)
- [ ] #5 PTY tests work on Linux and macOS without ptsname_r (absent for Apple in the libc crate), keeping the descriptors close-on-exec from the moment they are opened
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Replace GNU-only test assumptions: sed -i, /proc/self/status umask, ptsname_r, non-canonical temp path
2. Reproduce macOS /var -> /private/var on Linux by running tests with TMPDIR behind a symlink
3. Add both Darwin systems to flake.nix; check Darwin outputs evaluate
4. CI matrix on macOS runners (HYPO-0118.01)
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- PTY: kept /dev/ptmx (std opens it O_CLOEXEC atomically on both OSes) and replaced ptsname_r (not in libc for Apple) with ptsname under a static Mutex. Chose this over the issue's openpty + fcntl to avoid a cfg split and the window in which another test thread's spawn could inherit the fds before FD_CLOEXEC is set.
- sed_in_place helper replaces all four GNU `sed -i` uses (the issue listed two places).
- With TMPDIR behind a symlink, only tests/reads.rs failed (hyp canonicalizes the project root). It now passes.
- nixpkgs 26.05 warns it is the last release supporting x86_64-darwin; flake.nix has a comment saying so.

- Architect review fixes: AC #2 rewritten to match the ptsname approach; sed_in_place writes back with cat (keeps inode and mode) and refuses scripts containing a single quote; the PTY lock is scoped to the libc calls; the reader comment covers macOS EOF; docs/usage.md lists the Darwin systems.
- QA follow-up: e2e-dom and e2e-browser set __darwinAllowLocalNetworking, since both serve on loopback.
- AC #1 and #5 stay open until the macOS CI jobs pass.
<!-- SECTION:NOTES:END -->
