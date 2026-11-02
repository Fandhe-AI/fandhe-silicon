//! fandhe-silicon の共通型 crate。
//!
//! device・メモリ・実行・同期・能力問合せ・診断について、下-1（`fandhe-silicon-contract`）・
//! 下-2（`fandhe-silicon-exec`）・上（`fandhe-silicon-upper`）の 3 段で受け渡す型を定義する。
//! チップ固有の型はここへ持ち込まない。
//!
//! 能力の段階・データ規模・常駐位置・数値型から経路の候補を返す決定表の純関数と型（`decision` モジュール）も feature の条件なしで公開される。
//!
//! 外部依存なしの SHA-256（`hash` モジュール）は feature の条件なしで公開され、製品の経路から使える。
//! 実行まわりの共通の型（`execution` モジュール。パイプライン生成の入力の型・引数の宣言の型・キャッシュキーの型・引数の宣言の C4〜C8 を判定する純粋関数・間接実行の引数の型・その範囲と整列の検査・`first_instance` の判定・パイプライン統計のフラグの組と問合せの結果の大きさ・問合せの組の個数と解決先の検査）も feature の条件なしで公開される。
//! バッファの用途・ハンドル・メモリ操作のトレイトとバッファの引数検査の純粋関数（`memory` モジュール）も feature の条件なしで公開される。
//! ただし `MemoryOps::download_blocking` は feature `native-blocking-wait` の裏にある。
//! 束縛の宣言・束縛の組・束縛の組の配置・パイプラインの配置の型（`binding` モジュール）も feature の条件なしで公開される。
//! 成果物キャッシュのエントリの型・読み込み時の照合・台帳（`artifact_cache` モジュール）も feature の条件なしで公開される。
//! エラー・完了状態・理由の型（`error` モジュール）と、能力問合せの型（`capability` モジュール）、device の要求・所有形態の型（`device` モジュール）も feature の条件なしで公開される。
//! 描画先の handle の型と、device の識別子の照合の関数（`render_target` モジュール）も feature の条件なしで公開される。
//! テクスチャの形式の型と、機材に依らない性質を返す関数（`texture_format` モジュール）も feature の条件なしで公開される。
//! テクスチャの形式の問合せの鍵の一覧と、機能一覧の行・形式の値との対応の表（`texture_format_key` モジュール）も feature の条件なしで公開される。
//! 形式ごとの能力の値の型と組み立ての口（`texture_format_features` モジュール）も feature の条件なしで公開される。
//! 機能の問合せの鍵の定数と一覧（`feature_key` モジュール）も feature の条件なしで公開される。
//! 能力の表の行の型と、鍵で行を引く関数と行の無い鍵に返す値（`capability_table` モジュール）も feature の条件なしで公開される。
//! 上限と downlevel の問合せの鍵の定数と一覧（`limit_key`・`downlevel_key` モジュール）も feature の条件なしで公開される。
//! 値を指す鍵の型と、上限の値の鍵の定数と一覧（`value_key` モジュール）も feature の条件なしで公開される。
//! テクスチャの次元・大きさ・用途・記述子の型と、テクスチャを指す handle とその問い合わせ（`texture` モジュール）も feature の条件なしで公開される。テクスチャの作成の記述子を照らす純粋関数と、操作の時点の用途と view の形式を照らす純粋関数も同じモジュールにある。
//! SPIR-V のバイト列から語の列を作る型と関数（`spirv` モジュール）と、`const` の文脈で語の列を作るマクロ `spirv_words_const!`（crate の根）も feature の条件なしで公開される。
//! 検証レイヤーの不正を利用側へ届ける受け口（`validation` モジュール）と、失敗の種類を返す入口（`error` モジュールの `FailureClass`）も同じく条件なしで公開される。
//!
//! feature `ext-interop` を有効にしたときだけ、相互運用の公開の型と `ExternalInterop` の trait（`interop` モジュール）が公開される。
//!
//! feature `test-support` を有効にしたときだけ、背後実装の出力を比べるための検証用の道具
//! （`parity` モジュール）と、契約の試験の共通の枠（`test_frame` モジュール。宣言と実行の可否、結果のラベルの決定、組み合わせごとの集計）と、
//! 流し直しの照合の型と実行の口（`rerun` モジュール）と、能力の問合せの共通の規則の試験の部品（`capability_rule` モジュール）と、能力の表の鍵と値の検査の道具と差分の台帳の行の型（`capability_table_check` モジュール）と、形式の問合せの鍵を能力の問合せの口へ流して戻り値を検査する関数と、形式ごとの能力の値の形式の単位の一致を確かめる関数（`texture_format_query_check` モジュール）と、GPU も背後実装も使わない試験用の device と provider（`stub_device` モジュール）が公開される。
//! これら `test-support` の裏のモジュールは製品の経路からは使わない。
//!
//! feature `fault-injection` を有効にしたときだけ、テスト専用の故障の差し込み口（`fault` モジュール）が公開される。
//!
//! feature `graphics` を有効にしたときだけ、描画の公開の型と操作の trait（`graphics` モジュール）が公開される。描画先の大きさとバイト数、描画先からバッファへの写しの書き込み先の検査に関わる純粋関数も同じモジュールにある。
#![forbid(unsafe_code)]

pub mod artifact_cache;
pub mod binding;
pub mod capability;
pub mod capability_table;
pub mod decision;
pub mod device;
pub mod device_lost;
pub mod downlevel_key;
pub mod error;
pub mod execution;
pub mod feature_key;
pub mod hash;
#[cfg(feature = "ext-interop")]
pub mod interop;
pub mod limit_key;
pub mod memory;
pub mod render_target;
pub mod spirv;
pub mod texture;
pub mod texture_format;
pub mod texture_format_features;
pub mod texture_format_key;
pub mod validation;
pub mod value_key;

#[cfg(feature = "test-support")]
pub mod capability_rule;

#[cfg(feature = "test-support")]
pub mod capability_table_check;

#[cfg(feature = "test-support")]
pub mod texture_format_query_check;

#[cfg(feature = "test-support")]
pub mod parity;

#[cfg(feature = "test-support")]
pub mod test_frame;

#[cfg(feature = "test-support")]
pub mod rerun;

#[cfg(feature = "test-support")]
pub mod stub_device;

#[cfg(feature = "fault-injection")]
pub mod fault;

#[cfg(feature = "graphics")]
pub mod graphics;
