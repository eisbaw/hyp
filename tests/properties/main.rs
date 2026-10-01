//! Property tests (HYPO-0013): generated inputs against the invariants of
//! the parser, the validator and the transaction engine.
//!
//! - `codec`: `decode(encode(r))` is `r`, and `decode` never panics.
//! - `snapshot`: validation and derivation never panic on any record set.
//! - `commit`: a commit is all or nothing, and a successful one leaves a
//!   notebook `hyp check` accepts.
//! - `journal`: rolling a crashed write's journal forward is idempotent.
//! - `fingerprint`: the review basis ignores cosmetic fields and sees
//!   content changes.
//!
//! Stable Rust with proptest, not cargo-fuzz (which needs nightly, which the
//! pinned nixpkgs does not provide). Each property runs few cases with a
//! fixed seed by default, so `just test`, `just e2e` and the flake check
//! are fast and reproducible: a run fails only on a change, not on an input
//! it happened to draw. `PROPTEST_CASES` and `PROPTEST_RNG_SEED` override
//! both; `just fuzz` runs rounds with many cases and fresh seeds for a set
//! time. A failure prints the shrunk input and is saved under
//! `tests/proptest-regressions/`, which is replayed first on the next run:
//! commit those files along with the fix.
mod codec;
mod commit;
mod fingerprint;
mod generate;
mod journal;
mod snapshot;

use proptest::test_runner::{Config, RngSeed};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

/// The seed of every property unless `PROPTEST_RNG_SEED` gives one.
const SEED: u64 = 1013;

/// `default` cases per property and the fixed `SEED`, unless
/// `PROPTEST_CASES` or `PROPTEST_RNG_SEED` say otherwise (proptest reads
/// those, and warns about values it cannot parse); everything else as
/// proptest configures it from the environment.
pub fn cases(default: u32) -> Config {
    let mut config = Config::default();
    if std::env::var_os("PROPTEST_CASES").is_none() {
        config.cases = default;
    }
    if std::env::var_os("PROPTEST_RNG_SEED").is_none() {
        config.rng_seed = RngSeed::Fixed(SEED);
    }
    config
}

/// Every file under `dir`, recursively, with its bytes.
pub fn files(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for entry in fs::read_dir(&d).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                todo.push(entry.path());
            } else {
                out.insert(entry.path(), fs::read(entry.path()).unwrap());
            }
        }
    }
    out
}
