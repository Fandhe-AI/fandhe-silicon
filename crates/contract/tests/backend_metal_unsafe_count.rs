//! Metal の背後実装（`src/backend_metal.rs` と `src/backend_metal/` 配下）の `unsafe` の個数を固定する。
//!
//! 期待値の表は本ファイルだけに置く。変えてよいのは `unsafe` の増減を入れる同じ PR の中に限る
//! （入口: `docs/unsafe-policy.md`、spec TASK-9 のポインタ）。数える道具は `common` にある。
//! 対象はソースの文字列なので feature・target ではゲートしない（どの組でも同じ結果になる）。

mod common;

use common::{ExpectedFile, UnsafeCounts, check_backend};

const EXPECTED: &[ExpectedFile] = &[ExpectedFile {
    path: "src/backend_metal.rs",
    source: include_str!("../src/backend_metal.rs"),
    expected: UnsafeCounts::ZERO,
}];

#[test]
fn backend_metal_unsafe_count_matches_expected() {
    if let Err(msg) = check_backend("metal", EXPECTED) {
        panic!("unsafe count check failed:\n{msg}");
    }
}
