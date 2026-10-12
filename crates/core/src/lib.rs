//! fandhe-silicon の共通型 crate。
//!
//! device・メモリ・実行・同期・能力問合せ・診断について、下-1（`fandhe-silicon-contract`）・
//! 下-2（`fandhe-silicon-exec`）・上（`fandhe-silicon-upper`）の 3 段で受け渡す型を定義する。
//! チップ固有の型はここへ持ち込まない。
//!
//! feature `test-support` を有効にしたときだけ、背後実装の出力を比べるための検証用の道具
//! （`parity` モジュール）と、契約の試験の共通の枠（`test_frame` モジュール。宣言と実行の可否、結果のラベルの決定、組み合わせごとの集計）が公開される。
//! 製品の経路からは使わない。
#![forbid(unsafe_code)]

#[cfg(feature = "test-support")]
pub mod parity;

#[cfg(feature = "test-support")]
pub mod test_frame;
