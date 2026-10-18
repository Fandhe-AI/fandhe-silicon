//! fandhe-silicon の共通型 crate。
//!
//! device・メモリ・実行・同期・能力問合せ・診断について、下-1（`fandhe-silicon-contract`）・
//! 下-2（`fandhe-silicon-exec`）・上（`fandhe-silicon-upper`）の 3 段で受け渡す型を定義する。
//! チップ固有の型はここへ持ち込まない。
//!
//! 外部依存なしの SHA-256（`hash` モジュール）は feature の条件なしで公開され、製品の経路から使える。
//! 実行まわりの共通の型（`execution` モジュール。パイプライン生成の入力の型・引数の宣言の型・キャッシュキーの型）も feature の条件なしで公開される。
//! 成果物キャッシュのエントリの型（`artifact_cache` モジュール）も feature の条件なしで公開される。
//!
//! feature `test-support` を有効にしたときだけ、背後実装の出力を比べるための検証用の道具
//! （`parity` モジュール）と、契約の試験の共通の枠（`test_frame` モジュール。宣言と実行の可否、結果のラベルの決定、組み合わせごとの集計）と、
//! 流し直しの照合の型と実行の口（`rerun` モジュール）が公開される。
//! これら `test-support` の裏のモジュールは製品の経路からは使わない。
#![forbid(unsafe_code)]

pub mod artifact_cache;
pub mod execution;
pub mod hash;

#[cfg(feature = "test-support")]
pub mod parity;

#[cfg(feature = "test-support")]
pub mod test_frame;

#[cfg(feature = "test-support")]
pub mod rerun;
