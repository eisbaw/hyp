# Development recipes for hyp.
#
# Run them from inside the flake dev shell, which provides just, the Rust
# toolchain, node and jsdom (NODE_PATH). Recipes run in the repository root,
# so relative paths passed to them resolve from there.
#
#   nix develop            # then: just test, just e2e, ...
#   nix develop -c just e2e

# List available recipes.
default:
    @just --list

# Build the debug binary (target/debug/hyp).
[group('build')]
build:
    cargo build --locked

# Serve the WebUI; args go to `hyp web`, e.g. `just web --project DIR --port 8000`.
[group('run')]
[positional-arguments]
web *args:
    cargo run --locked -- web "$@"

# Serve the synthetic demo notebook from a throwaway directory, deleted on exit.
[group('run')]
demo:
    #!/usr/bin/env bash
    set -euo pipefail
    dir=$(mktemp -d -t hyp-demo.XXXXXX)
    trap 'rm -rf "$dir"' EXIT
    cargo run --locked -- --project "$dir" init --demo
    cargo run --locked -- --project "$dir" web

# Run all Rust tests (integration tests in tests/).
[group('test')]
test:
    cargo test --locked

# End-to-end gate: Rust tests plus the jsdom UI test against the real built binary.
[group('test')]
e2e: test build
    HYP_BIN=target/debug/hyp node scripts/dom-test.cjs

# Playwright UI test in headless Chromium from nixpkgs (the dev shell sets PLAYWRIGHT_BROWSERS_PATH).
[group('test')]
browser-test: build
    @test -n "${PLAYWRIGHT_BROWSERS_PATH:-}" || { echo "PLAYWRIGHT_BROWSERS_PATH is not set: run this inside nix develop" >&2; exit 1; }
    HYP_BIN=target/debug/hyp node scripts/browser-test.cjs

# Property tests (tests/properties) in rounds of `cases` per property, each with a fresh seed, until `seconds` pass (at least one round; a round started finishes). Failures are kept in tests/proptest-regressions/.
[group('test')]
fuzz seconds="300" cases="200":
    #!/usr/bin/env bash
    set -euo pipefail
    cargo test --locked --test properties --no-run
    end=$((SECONDS + {{ seconds }}))
    round=0
    while ((round == 0 || SECONDS < end)); do
        round=$((round + 1))
        seed=$(od -An -N8 -tu8 /dev/urandom | tr -d ' ')
        echo "fuzz round $round: {{ cases }} cases per property, PROPTEST_RNG_SEED=$seed"
        PROPTEST_CASES={{ cases }} PROPTEST_RNG_SEED=$seed cargo test --locked --test properties --quiet
    done
    echo "fuzz: $round rounds in $SECONDS s, no failures"

# Format Rust sources in place.
[group('quality')]
fmt:
    cargo fmt

# Fail if Rust sources are not formatted.
[group('quality')]
fmt-check:
    cargo fmt --check

# Run clippy on all targets, warnings as errors.
[group('quality')]
lint:
    cargo clippy --locked --all-targets -- -D warnings

# Run every flake check (package tests, clippy, formatting, e2e-dom, and e2e-browser except on macOS) on the Git-tracked tree.
[group('quality')]
check:
    nix flake check -L
