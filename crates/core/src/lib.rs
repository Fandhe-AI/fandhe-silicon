//! fandhe-silicon の共通型 crate。
//!
//! device・メモリ・実行・同期・能力問合せ・診断について、下-1（`fandhe-silicon-contract`）・
//! 下-2（`fandhe-silicon-exec`）・上（`fandhe-silicon-upper`）の 3 段で受け渡す型を定義する。
//! チップ固有の型はここへ持ち込まない。
#![forbid(unsafe_code)]
