# `unsafe` の局所化・不変条件の書式・個数テストの枠の形

`unsafe` を置ける場所、`// SAFETY:` に書く内容、背後実装ごとの `unsafe` の個数テストの枠の形、個数が変わるときの手順、完了の条件の確かめを記録する。
ポインタ: TASK-9（`docs/spec/05-tasks.md`）・AP-8。

- 本書は案である。オーナーの確認を経るまで、確定した方針として扱わない。
- 形を選んだ理由・経緯・判断は private spec 側に置く。本リポにはポインタだけを書く。
- 本書は本リポで公開済みのコード・lint 設定・規約と Rust の言語仕様から書く。spec の転記ではない。
- 記入例は実在しないダミー。実在の FFI 呼び出しや個数の値は本書に書かない。

## 現状と前提

| 項目 | 現状 |
| ---- | ---- |
| 前提 | TASK-4（骨格）・TASK-5 は完了済み（TASK-5 の親 Issue は closed） |
| `unsafe` の禁止 | `crates/core`・`crates/exec`・`crates/upper` の `lib.rs` に `#![forbid(unsafe_code)]` |
| 記述の強制 | ルート `Cargo.toml` の `[workspace.lints]` で `unsafe_op_in_unsafe_fn`・`clippy::undocumented_unsafe_blocks`・`clippy::missing_safety_doc` を deny |
| 背後実装のモジュール | `crates/contract/src/backend_{metal,vulkan,cuda}.rs`。doc コメントだけの骨格で、コードとしての `unsafe` は無い（`//!` コメントの中に `unsafe` という語はある） |
| `contract` の crate 直下 | `lib.rs` に `#![deny(unsafe_code)]`、`pub mod backend_{metal,vulkan,cuda};` の宣言に `#[allow(unsafe_code)]`（backend 以外に `unsafe` を書くとビルドが失敗する） |
| cfg ゲート | Metal は feature のみ、Vulkan・CUDA は feature かつ `not(target_arch = "wasm32")`（`crates/contract/src/lib.rs`、`crates/contract/tests/backend_cfg_gate.rs`） |
| 個数テスト | `crates/contract/tests/backend_{metal,vulkan,cuda}_unsafe_count.rs` と `crates/contract/tests/common/mod.rs`。道具を見本で確かめるテストは `crates/contract/tests/common_unsafe_count_samples.rs`。期待値は全背後実装・全種別で 0 |
| 外部依存 | 0 件。追加はユーザー承認制（[dependency-policy](../.claude/rules/dependency-policy.md)） |

## `unsafe` を置ける場所

- 置けるのは `crates/contract/src/backend_<名前>.rs` と `crates/contract/src/backend_<名前>/` 配下だけ（[coding-rust](../.claude/rules/coding-rust.md)、AP-8 のポインタ）
- `core`・`exec`・`upper` は `#![forbid(unsafe_code)]` でコンパイラが拒否する。上の層の検査は TASK-15・TASK-262 のポインタ
- `contract` の中の守り（#52 で実装済み）
    - 一次の守り: `crates/contract/src/lib.rs` の crate 直下に `#![deny(unsafe_code)]` を置き、`pub mod backend_<名前>;` の宣言にだけ `#[allow(unsafe_code)]` を付ける。backend 以外のファイルに `unsafe` を書くとビルドが失敗する（`forbid` は `allow` で外せないので `deny` を使う）
    - 二次の守り: 個数テストの枠（後述）が、`backend_<名前>/` 配下で期待値の表に無いファイルを検出する

## 不変条件の書式

lint が強制するのは「書いてあること」まで。本書は中身を決める。

| 対象 | 書く場所 | 書く内容 |
| ---- | -------- | -------- |
| `unsafe { }` ブロック | 直前の `// SAFETY:` | 成立させている不変条件、それを保証しているもの（呼び出し元の検証・型・FFI の契約）、該当する項目 ID のポインタ（あれば） |
| `unsafe impl` | 直前の `// SAFETY:` | 同上（`Send` / `Sync` なら共有・移動が安全である根拠） |
| `unsafe fn` | `/// # Safety` | 呼び出し側が満たす条件。本体の `unsafe` 操作は個別のブロックと `// SAFETY:` を持つ |

- ブロックは呼び出し箇所ごとに分ける（1 ブロック = 1 組の不変条件。safe なコードを巻き込んで広げない）
- 理由・経緯・個数の変化の履歴はコメントに積まない

記入例（ダミー）:

```rust
/// 確保済みのデバイスバッファの先頭ポインタを返す。
///
/// # Safety
///
/// `handle` は `create_buffer` が返した、未解放のハンドルであること。
unsafe fn raw_ptr(handle: RawHandle) -> *mut u8 {
    // SAFETY: 呼び出し側が `handle` の生存を保証している（上の `# Safety`）。
    // 返すポインタは `handle` が解放されるまで有効。
    unsafe { ffi_buffer_contents(handle) }
}
```

## 個数テストの枠の形

| 要素 | 形 |
| ---- | -- |
| 数える道具 | `crates/contract/tests/common/mod.rs` に純粋関数（ソース文字列から種別ごとの個数）と、期待値との照合（不一致で理由付きのエラー）を置く。外部依存 0・公開 API に出さない |
| 前処理 | コメント（行コメント・doc コメント・入れ子のブロックコメント）と文字列リテラル（通常・raw）を除く。現在の骨格の `backend_*.rs` と `lib.rs` は `//!` コメントに `unsafe` の語を含むため、除かないと今の時点で 0 にならない |
| 数え方 | 前処理のあと、識別子の境界で切った `unsafe` キーワードをすべて拾う。`unsafe_code` のような識別子の一部は拾わない。`pub unsafe fn`・`pub(crate) unsafe fn` は行頭でなくても拾う。種別はキーワードの後ろを見て決め、`extern` が続く場合は省略可能な ABI の文字列を読み飛ばして `fn` か `{` かで分ける |
| 種別 | ブロック（`unsafe {`）/ `unsafe impl` / `unsafe fn`（`unsafe extern "…" fn` を含む）/ `unsafe extern` ブロック / その他（`unsafe trait`・`#[unsafe(...)]` など）。全種別の合計が `unsafe` キーワードの総数に一致する（どの種別にも入らないものを作らない） |
| 背後実装ごとのテスト | `crates/contract/tests/backend_<名前>_unsafe_count.rs` を背後実装ごとに 1 ファイル。対象のソースを `include_str!` で読み、ファイルごとの期待値と照合する |
| 期待値の置き場 | テストファイルの中の表（ファイルの相対パスから種別ごとの個数）だけ。文書・README に数値を二重に持たない。現状の期待値は全背後実装・全種別で 0 |
| 一覧に無いファイルの検出 | `env!("CARGO_MANIFEST_DIR")` から `src/` を走査し、(a) `backend_<名前>/` 配下に期待値の表に無い `.rs` がある (b) backend 以外のファイルに `unsafe` がある、のどちらでも失敗する |
| cfg ゲート | 個数テストは feature・target でゲートしない。数える対象はソースの文字列でビルドの構成に依らず、どの feature の組でも走り、`common` の未使用コードの警告も出ない |
| 走る場所 | `make verify` と `make build-profiles` の `cargo test`。GPU の実機は要らず、GitHub ホステッドの CI で走る。新しい make ターゲット・CI ジョブは足さない |
| 背後実装を足すとき | 同じ形のテストファイルを 1 つ足し、`lib.rs` の mod 宣言に `#[allow(unsafe_code)]` を付ける（DX12・GL の置き場のポインタ: `crates/contract/Cargo.toml` のコメント） |

- #52 の実装で足した形: `BACKENDS`（登録点。登録した背後実装のテストファイルの存在も検査）、表の `path` と `include_str!` の中身の一致の検査、`src/` 配下のシンボリックリンクの拒否、文字リテラルとライフタイムの区別、raw 識別子 `r#unsafe` を数えないこと、検査の失敗を集めて 1 回で返すこと
- `clippy::undocumented_unsafe_blocks` は `unsafe impl` も対象にする（rustc 1.99.0 / clippy 0.1.99 で確認）
- 単純な文字列の一致だけで数えない理由: コメントや文字列の中の語、`forbid(unsafe_code)` のような識別子、edition 2024 で `unsafe` を伴う `extern` ブロックと属性を、取りこぼさず数えすぎないため
- 「増減で失敗すること」は、道具に見本の文字列を与えるテストで確かめる。実物の `unsafe` をコミットせずに、増えた場合・減った場合の両方の失敗を示す（`scripts/test-deps-check.sh` と同じ考え方）
- #53 の実装で `crates/contract/tests/common_unsafe_count_samples.rs` に足した見本（必須の場合）
    - コメントの中の `unsafe {`
    - 文字列リテラルの中の `unsafe {`
    - `forbid(unsafe_code)` の識別子
    - `unsafe extern` ブロックと `unsafe extern "C" fn` の区別
    - `pub unsafe fn`
    - `unsafe impl`
    - 期待より 1 多い場合・1 少ない場合（`compare_counts` への見本。実ファイルでは `check_backend` が、期待値を 1 ずらした表で背後実装ごとに失敗すること）

## 個数が変わるときの扱い

期待値の表（`crates/contract/tests/backend_<名前>_unsafe_count.rs` の `EXPECTED`）を変える PR の手順。
承認者と記録の置き場は未決（下の論点 4）で、決まるまでの扱いは小節「承認」に書く。

### 対象になる変更

個数が変わらない変更（表に触れない変更）は対象外。

| 変更 | 個数テストの反応 | 表に対して行うこと |
| ---- | ---------------- | ------------------ |
| 既存のファイルで `unsafe` が増える・減る | `<path>: <種別>: expected <期待>, actual <実際>` で失敗 | その行の `expected` を種別ごとに直す |
| `backend_<名前>/` 配下に `.rs` を足す | `not listed in the expected table` で失敗（`unsafe` が 0 でも） | `path`・`include_str!`・`expected` の行を足す |
| 表にあるファイルを消す・改名する | `include_str!` のコンパイルエラー | 行を消す・直す |
| 背後実装を足す | 登録したがテストファイルが無いと失敗 | `BACKENDS` への登録、テストファイル 1 つ、`lib.rs` の mod 宣言の `#[allow(unsafe_code)]` |
| 数える道具（`tests/common/mod.rs`）の数え方を変える | 見本のテスト（`common_unsafe_count_samples.rs`）が失敗しうる | 見本を同じ PR で直す。表が変わるなら増減として扱う |

- backend 以外のファイルに `unsafe` を置く変更は、この手順では通せない（`lib.rs` の `deny` でビルドが失敗する。範囲を広げる話は論点 3）

### 手順

1. `unsafe` が要るかを確かめる。safe な書き方で足りるなら足さない。置き場は `backend_<名前>` の中だけ
2. `// SAFETY:`・`/// # Safety` を「不変条件の書式」どおりに書く（ブロックは呼び出し箇所ごと）
3. `cargo test -p fandhe-silicon-contract --test backend_<名前>_unsafe_count` を走らせ、失敗の出力からファイル・種別ごとの期待と実際の差を読む
4. 差の 1 件ずつを、足した・消した箇所と突き合わせる。説明できない差があれば表を直さず原因を調べる
    - 数え方の注意: `unsafe extern` ブロックは extern block、`#[unsafe(...)]` は other、マクロ定義・呼び出しの中の `unsafe` も数える
5. 同じ PR で、該当する背後実装のテストファイルの表だけを直す。出力の `actual` を写すだけにしない（4 を済ませてから）
6. `make verify`・`make build-profiles`・`make check` を通す
7. レビューを受ける
    - security-auditor は必須（[delegation-impl](../.claude/rules/delegation-impl.md)）。新しい箇所ごとの `// SAFETY:` の中身、FFI から受けた値の検証、safe API から未定義動作に届かないことを見る（[security](../.claude/rules/security.md)）
    - reviewer は表の差と PR 本文の一致を見る
8. PR 本文に増減の表を書く（次の小節）
9. 小節「承認」を経てマージする

### PR 本文に書くこと

- 増減の表。列は 背後実装／ファイル／種別／変更前 → 変更後／`// SAFETY:` の所在（関数名など）
- 減る場合は、消した箇所を書く
- `unsafe` が要る理由は、コードから説明できる範囲で 1〜2 行。private spec の本文・内部判断は書かず、ID のポインタだけにする
- 得ていない承認を書かない

記入例（実在しないダミー）:

| 背後実装 | ファイル | 種別 | 変更前 → 変更後 | `// SAFETY:` の所在 |
| -------- | -------- | ---- | --------------- | ------------------- |
| example | `backend_example/buffer.rs` | block | 0 → 1 | `Buffer::map` |

### してはいけないこと

- 期待値だけを先に、または後から変える PR を出す
- `#[allow(unsafe_code)]` を backend 以外に付ける。`deny(unsafe_code)`・`forbid(unsafe_code)`・workspace の lints を緩める
- 個数テストを `#[ignore]` や cfg で外す、検査を弱める
- `git commit --no-verify` を使う
- 文書・README・コメントに個数の値や変化の履歴を持つ

### 承認（未決。論点 4）

- 決まるまでの扱いは未承認の仮置き: 期待値の表を変える PR は自動でマージせず、PR 本文に「オーナー判断待ち」と書いて、オーナーの明示の判断を待つ（[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md)）
- 決まるまでの記録は、PR 本文の増減の表と PR 上のレビュー・コメントだけ。別の記録ファイルは作らない
- 検査の限界: 個数テストは表と実物の一致だけを見る。承認が本物かは判定できない。期待値のテストファイルに所有者レビューを必須にする設定はリポジトリ設定で、オーナー判断（本書では変えない）

## オーナー判断待ち

推奨は確定ではなく、承認を得たものでもない。

| # | 論点 | 選択肢 | 推奨 | 止まるもの |
| - | ---- | ------ | ---- | ---------- |
| 1 | 数え方の厳密さ | (a) 依存 0 の自前の走査 (b) 構文木を使う外部 crate（依存の承認が要る） | (a) | #52 の実装の方式（#52 は (a) を未承認の仮置きとして実装した） |
| 2 | 種別の分け方と「その他」の扱い | (a) 本書の 5 種別を個別に固定 (b) ブロック・impl・fn の 3 種別だけ固定し残りは合計で固定 | (a) | #52 の期待値の表の形（#52 は (a) を未承認の仮置きとして実装した。(b) へは `common` の `UnsafeCounts`・`compare_counts` と 3 つの表の書き換えで畳める） |
| 3 | `#[allow(unsafe_code)]` を付けてよいモジュールの範囲（CPU の ISA 別の実装など、背後実装以外に広げるか） | (a) `backend_*` だけ (b) 専用のモジュールにも付け、同じ枠で数える | 現状は (a) のまま据え置き、必要になった時点で伺う | cpu-isa の実装（本 TASK の範囲外） |
| 4 | 期待値の変更の承認者と記録の置き場（TASK-9・AP-8） | (a) PR のレビュー（reviewer と security-auditor）で足りる。記録は PR 本文。自動運転が `unsafe` の増加を人の判断なしでマージできる (b) 表のすべての変更にオーナーの明示承認を要る。記録は PR 上の承認。背後実装の実装 PR のほぼ全部がオーナー待ちで止まる (c) 増加・背後実装の追加・数える道具の変更・`allow` の範囲の変更だけオーナーの明示承認を要る。減少と個数 0 の行の追加はレビューで足りる。記録は PR 本文の表と PR 上の承認 | (c)。増加はメモリ安全性に直結し、依存の承認と同じ重さで扱える。減少まで止めると整理が進まない。記録は PR に置き、数値を別ファイルに二重に持たない。機械可読の記録と検査（`deps-approved.toml` のような形）は、採るなら別 Issue | 期待値を 0 から変える最初の PR（背後実装の実装タスク）。#55 は止まらない。回答は「推奨で」または「(a)」「(b)」 |
| 5 | 数える道具を `core`・`exec`・`upper` にも当てるか | (a) 当てない（`forbid` に任せる） (b) TASK-15・TASK-262 の側で使い回す | (a)。使い回しはそれぞれのタスクで決める | TASK-15・TASK-262 |

## 後続の分担

| 作業 | 担当 |
| ---- | ---- |
| `lib.rs` の `deny` / `allow` の指定、数える道具、背後実装ごとのテストファイルの実装、`unsafe impl` の確認 | #52（実装済み。論点 1・2 は未承認の仮置き） |
| 見本の文字列で増えた場合・減った場合の失敗を確かめるテスト（入口は `common` の `strip_comments_and_literals`・`count_unsafe`・`compare_counts`） | #53（実装済み） |
| 個数が変わるときの承認の手順書 | #54（本書の「個数が変わるときの扱い」に案を記載。論点 4 は判断待ち） |
| 完了の条件の確かめの記録 | #55（本書の「完了の条件の確かめ」に記録済み） |
| `.claude/rules/coding-rust.md`・`AGENTS.md` から本書を参照する | 未実施（書式の確定後。オーナーの確認待ち） |

## 完了の条件の確かめ

本リポで観測できる事実による確かめの記録（ポインタ: TASK-9・AP-8）。
spec の完了の条件との対応づけはオーナーが確認する。条件の本文は転記しない。
実物を変える確かめは、コミットしない一時コピーで行った（本リポに `unsafe` を足していない）。

| 確かめる項目 | 方法 | 現時点の結果 |
| ------------ | ---- | ------------ |
| 背後実装ごとに個数テストがある | `crates/contract/tests/backend_{metal,vulkan,cuda}_unsafe_count.rs` と `common` の `BACKENDS` の登録 | 3 背後実装とも有り |
| 個数テストがどの feature の組でも走る | `make verify`、`make build-profiles` | 通過 |
| 数える道具が増減を数え、食い違いで失敗する | `cargo test -p fandhe-silicon-contract --test common_unsafe_count_samples` | 通過 |
| 実物の `unsafe` を 1 つ足すと、その背後実装の個数テストが失敗する | 一時コピーで `src/backend_<名前>.rs` に足し、`cargo test -p fandhe-silicon-contract --test backend_<名前>_unsafe_count` | 3 背後実装とも失敗（`expected` と `actual` の食い違い）。戻すと通過 |
| 期待値が実際と 1 ずれると失敗する | 一時コピーで表の `expected` を 1 ずらす | 3 背後実装とも失敗。戻すと通過 |
| `backend_<名前>/` 配下の表に無い `.rs` で失敗する | 一時コピーで `.rs` を 1 つ足す | 3 背後実装とも失敗（`not listed in the expected table`） |
| backend 以外に `unsafe` を書くとビルドが失敗する | 一時コピーで `src/lib.rs` に足し、`cargo build -p fandhe-silicon-contract` | 失敗（`deny(unsafe_code)`） |
| `core`・`exec`・`upper` の `#![forbid(unsafe_code)]` | 各 `src/lib.rs` の宣言 | 3 crate とも有り |
| 不変条件の記述の強制 | ルート `Cargo.toml` の `[workspace.lints]` の deny 3 件。書式の中身は本書「不変条件の書式」（案） | 有り（書式は案） |
| 個数が変わるときの手順 | 本書「個数が変わるときの扱い」（案） | 記載済み（論点 4 は判断待ち） |
| 品質ゲート | `make check`、`make deny` | 通過 |

- 判定: 本リポで観測できる範囲では、背後実装ごとに個数テストが動き、増減で失敗することを確かめた。**TASK-9 の完了は本書では宣言しない**（spec との対応づけと論点 1・2・4 はオーナーの確認・判断待ち）
- 再実行の契機: 論点 1・2・4 への回答、背後実装の追加、数える道具の変更、期待値を 0 から変える最初の PR
- 確かめた日付: 2026-10-11（base: ad22662）

## 関連

- [coding-rust](../.claude/rules/coding-rust.md)
- [code-comment-style](../.claude/rules/code-comment-style.md)
- [security](../.claude/rules/security.md)
- [dependency-policy](../.claude/rules/dependency-policy.md)
- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)
- [contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md)
- [workspace-boundary](workspace-boundary.md)
