//! `src/cpu_ops/neon.rs` の `unsafe` の個数を固定する（TASK-1351 のポインタ）。
mod common;

use common::{ExpectedFile, UnsafeCounts, check_backend};

const EXPECTED: &[ExpectedFile] = &[ExpectedFile {
    path: "src/cpu_ops/neon.rs",
    source: include_str!("../src/cpu_ops/neon.rs"),
    expected: UnsafeCounts {
        block: 2,
        ..UnsafeCounts::ZERO
    },
}];

#[test]
fn cpu_ops_neon_unsafe_count_matches_expected() {
    if let Err(msg) = check_backend("cpu_ops_neon", EXPECTED) {
        panic!("unsafe count check failed:\n{msg}");
    }
}
