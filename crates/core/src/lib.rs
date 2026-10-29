//! fandhe-silicon の共通型 crate。
//!
//! device・メモリ・実行・同期・能力問合せ・診断について、下-1（`fandhe-silicon-contract`）・
//! 下-2（`fandhe-silicon-exec`）・上（`fandhe-silicon-upper`）の 3 段で受け渡す型を定義する。
//! チップ固有の型はここへ持ち込まない。
//!
//! 外部依存なしの SHA-256（`hash` モジュール）は feature の条件なしで公開され、製品の経路から使える。
//! 実行まわりの共通の型（`execution` モジュール。パイプライン生成の入力の型・引数の宣言の型・キャッシュキーの型・引数の宣言の C4〜C8 を判定する純粋関数）も feature の条件なしで公開される。
//! バッファの用途・ハンドル・メモリ操作のトレイトとバッファの引数検査の純粋関数（`memory` モジュール）も feature の条件なしで公開される。
//! ただし `MemoryOps::download_blocking` は feature `native-blocking-wait` の裏にある。
//! 成果物キャッシュのエントリの型・読み込み時の照合・台帳（`artifact_cache` モジュール）も feature の条件なしで公開される。
//! エラー・完了状態・理由の型（`error` モジュール）と、能力問合せの型（`capability` モジュール）、device の要求・所有形態の型（`device` モジュール）も feature の条件なしで公開される。
//! 検証レイヤーの不正を利用側へ届ける受け口（`validation` モジュール）も同じく条件なしで公開される。
//!
//! feature `test-support` を有効にしたときだけ、背後実装の出力を比べるための検証用の道具
//! （`parity` モジュール）と、契約の試験の共通の枠（`test_frame` モジュール。宣言と実行の可否、結果のラベルの決定、組み合わせごとの集計）と、
//! 流し直しの照合の型と実行の口（`rerun` モジュール）と、能力の問合せの共通の規則の試験の部品（`capability_rule` モジュール）が公開される。
//! これら `test-support` の裏のモジュールは製品の経路からは使わない。
//!
//! feature `fault-injection` を有効にしたときだけ、テスト専用の故障の差し込み口（`fault` モジュール）が公開される。
#![forbid(unsafe_code)]

pub mod artifact_cache;
pub mod capability;
pub mod device;
pub mod error;
pub mod execution;
pub mod hash;
pub mod memory;
pub mod validation;

#[cfg(feature = "test-support")]
pub mod capability_rule;

#[cfg(feature = "test-support")]
pub mod parity;

#[cfg(feature = "test-support")]
pub mod test_frame;

#[cfg(feature = "test-support")]
pub mod rerun;

#[cfg(feature = "fault-injection")]
pub mod fault;
