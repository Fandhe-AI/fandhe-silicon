# `unsafe` の局所化・不変条件の書式・個数テストの枠の形

`unsafe` を置ける場所、`// SAFETY:` に書く内容、背後実装ごとの `unsafe` の個数テストの枠の形、個数が変わるときの手順を記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-9。ポインタのみ。関連: AP-8）。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

記入例は実在しないダミー。実在の FFI 呼び出しや個数の値は本書に書かない（個数の正は各 `backend_<名前>_unsafe_count.rs` の表）。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| `unsafe` の禁止 | `crates/core`・`crates/exec`・`crates/upper` の `lib.rs` に `#![forbid(unsafe_code)]` | 各 `src/lib.rs` |
| 記述の強制 | ルート `Cargo.toml` の `[workspace.lints]` で `unsafe_op_in_unsafe_fn`・`clippy::undocumented_unsafe_blocks`・`clippy::missing_safety_doc` を deny | ルート `Cargo.toml` |
| 背後実装のモジュール | `crates/contract/src/backend_{metal,vulkan,cuda}.rs`。Vulkan は `crates/contract/src/backend_vulkan/` に子モジュールを持つ | `ls crates/contract/src` |
| `contract` の crate 直下 | `lib.rs` に `#![deny(unsafe_code)]`、`pub mod backend_{metal,vulkan,cuda};` の宣言に `#[allow(unsafe_code)]`（backend 以外に `unsafe` を書くとビルドが失敗する） | `crates/contract/src/lib.rs` |
| cfg ゲート | Metal は feature のみ、Vulkan・CUDA は feature かつ `not(target_arch = "wasm32")` | `crates/contract/src/lib.rs`、`crates/contract/tests/backend_cfg_gate.rs` |
| 個数テスト | `crates/contract/tests/backend_{metal,vulkan,cuda}_unsafe_count.rs` と `crates/contract/tests/common/mod.rs`。道具を見本で確かめるテストは `common_unsafe_count_samples.rs` と `common_scanner_regression.rs` | `ls crates/contract/tests` |
| 外部依存 | 0 件。追加はユーザー承認制（[dependency-policy](../.claude/rules/dependency-policy.md)） | `Cargo.lock` |

## `unsafe` を置ける場所

- 置けるのは `crates/contract/src/backend_<名前>.rs` と `crates/contract/src/backend_<名前>/` 配下だけ（[coding-rust](../.claude/rules/coding-rust.md)、AP-8 のポインタ）
- `core`・`exec`・`upper` は `#![forbid(unsafe_code)]` でコンパイラが拒否する
- `contract` の中の守り
    - 一次の守り: `crates/contract/src/lib.rs` の crate 直下に `#![deny(unsafe_code)]` を置き、`pub mod backend_<名前>;` の宣言にだけ `#[allow(unsafe_code)]` を付ける。backend 以外のファイルに `unsafe` を書くとビルドが失敗する（`forbid` は `allow` で外せないので `deny` を使う）
    - 二次の守り: 個数テストの枠（後述）が、`backend_<名前>/` 配下で期待値の表に無いファイルと、backend 以外のファイルの `unsafe` を検出する

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
| 数える道具 | `crates/contract/tests/common/mod.rs` に純粋関数（`strip_comments_and_literals`・`count_unsafe`）と、期待値との照合（`compare_counts`。不一致で理由付きのエラー）を置く。外部依存 0 |
| 前処理 | コメント（行コメント・doc コメント・入れ子のブロックコメント）と文字列リテラル（通常・raw）を除く。文字リテラルとライフタイムを区別する |
| 数え方 | 前処理のあと、識別子の境界で切った `unsafe` キーワードをすべて拾う。`unsafe_code` のような識別子の一部と raw 識別子 `r#unsafe` は拾わない。`pub unsafe fn`・`pub(crate) unsafe fn` は行頭でなくても拾う。種別はキーワードの後ろを見て決め、`extern` が続く場合は省略可能な ABI の文字列を読み飛ばして `fn` か `{` かで分ける。マクロ定義・呼び出しの中の `unsafe` もトークンとして数える |
| 種別 | ブロック（`unsafe {`）/ `unsafe impl` / `unsafe fn`（`unsafe extern "…" fn` を含む）/ `unsafe extern` ブロック / その他（`unsafe trait`・`#[unsafe(...)]` など）。期待値はこの 5 種別を個別に持つ（`UnsafeCounts`）。全種別の合計が `unsafe` キーワードの総数に一致する |
| 背後実装ごとのテスト | `crates/contract/tests/backend_<名前>_unsafe_count.rs` を背後実装ごとに 1 ファイル。対象のソースを `include_str!` で読み、ファイルごとの期待値と `check_backend` で照合する |
| 期待値の置き場 | 各テストファイルの中の表 `EXPECTED`（ファイルの相対パスから種別ごとの個数）だけ。文書・README に数値を二重に持たない |
| 登録点 | `common` の `BACKENDS`。登録した全背後実装のテストファイルの存在も検査する |
| cfg ゲート | 個数テストは feature・target でゲートしない。数える対象はソースの文字列でビルドの構成に依らず、どの feature の組でも走る |
| 走る場所 | `make verify` と `make build-profiles` の `cargo test`。GPU の実機は要らない |
| 背後実装を足すとき | `BACKENDS` への登録、同じ形のテストファイルを 1 つ、`lib.rs` の mod 宣言に `#[allow(unsafe_code)]` |

`check_backend` が行う検査: 登録済みの名前か／表の各行がその背後実装の持ち場か・重複が無いか／`include_str!` の中身が実ファイルと一致するか／個数の照合／持ち場の `.rs` が表に載っているか／backend 以外のファイルに `unsafe` が無いか／`src/` 配下にシンボリックリンクが無いか／登録済みの全背後実装にテストファイルがあるか。失敗は集めて 1 回で返す。

## 振る舞いとテストの対応

テストは `crates/contract/tests/` の関数。実物の `unsafe` をコミットせず、道具に見本の文字列を与えて増減の失敗を確かめる。

| 振る舞い | テスト |
| -------- | ------ |
| 各背後実装の実ファイルの個数が表どおり | `backend_metal_unsafe_count_matches_expected`・`backend_vulkan_unsafe_count_matches_expected`・`backend_cuda_unsafe_count_matches_expected`（各 `backend_<名前>_unsafe_count.rs`） |
| コメントの中・文字列リテラルの中の `unsafe` を数えない。その後ろの実物は数える | `unsafe_inside_comments_is_not_counted`・`real_block_after_comment_is_still_counted`・`unsafe_inside_string_literals_is_not_counted`・`real_block_after_literal_is_still_counted` |
| `unsafe` を含む識別子（`forbid(unsafe_code)` や `r#unsafe`）を数えない | `identifiers_containing_unsafe_are_not_counted` |
| `unsafe extern` ブロックと `unsafe extern "C" fn` の区別 | `unsafe_extern_block_and_fn_are_distinguished` |
| `pub unsafe fn`・`unsafe impl` の形を数える | `unsafe_fn_and_impl_forms_are_counted` |
| ブロックの形・その他の種別を数える。種別ごとに数える | `block_forms_and_other_kinds_are_counted`・`mixed_kinds_are_counted_per_kind` |
| 前処理が行数を保ち、コメントとリテラルの中身を除く | `strip_keeps_line_count_and_removes_comment_and_literal_bodies` |
| 期待値と一致なら通り、実際が多い・少ない・種別だけが違うときは失敗 | `compare_counts_passes_when_equal`・`compare_counts_fails_when_actual_exceeds_expected`・`compare_counts_fails_when_actual_is_below_expected`・`compare_counts_fails_when_only_kinds_differ` |
| 実ファイルの表を 1 ずらすと背後実装ごとに失敗する | `check_backend_fails_when_expected_is_off_by_one` |
| 文字リテラルのエスケープ・結合文字を含む識別子で取りこぼさない | `escaped_quote_char_literal_does_not_hide_unsafe`・`escaped_quote_byte_literal_does_not_hide_unsafe`・`hex_and_unicode_escapes_are_skipped_whole`・`backslash_char_literal_is_skipped`・`identifier_with_combining_mark_is_not_the_keyword`（`common_scanner_regression.rs`） |
| feature を有効にした背後実装のモジュールが見える | `backend_metal_module_is_visible`・`backend_vulkan_module_is_visible`・`backend_cuda_module_is_visible`（`backend_cfg_gate.rs`） |

公開テストから到達させていない検査（コードにはある）: 表に無い `.rs` の検出、backend 以外のファイルの `unsafe` の検出、シンボリックリンクの拒否、表の重複・取り違えの検出、`BACKENDS` に無い名前の拒否。

## 個数が変わるときの扱い

期待値の表（`crates/contract/tests/backend_<名前>_unsafe_count.rs` の `EXPECTED`）を変える PR の手順。承認の要否は小節「承認」に書く。

### 対象になる変更

個数が変わらない変更（表に触れない変更）は対象外。

| 変更 | 個数テストの反応 | 表に対して行うこと |
| ---- | ---------------- | ------------------ |
| 既存のファイルで `unsafe` が増える・減る | `<path>: <種別>: expected <期待>, actual <実際>` で失敗 | その行の `expected` を種別ごとに直す |
| `backend_<名前>/` 配下に `.rs` を足す | `not listed in the expected table` で失敗（`unsafe` が 0 でも） | `path`・`include_str!`・`expected` の行を足す |
| 表にあるファイルを消す・改名する | `include_str!` のコンパイルエラー | 行を消す・直す |
| 背後実装を足す | 登録したがテストファイルが無いと失敗 | `BACKENDS` への登録、テストファイル 1 つ、`lib.rs` の mod 宣言の `#[allow(unsafe_code)]` |
| 数える道具（`tests/common/mod.rs`）の数え方を変える | 見本のテスト（`common_unsafe_count_samples.rs`）が失敗しうる | 見本を同じ PR で直す。表が変わるなら増減として扱う |

- backend 以外のファイルに `unsafe` を置く変更は、この手順では通せない（`lib.rs` の `deny` でビルドが失敗する）

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

### 承認

本リポの運用の規則。

- オーナーの明示の承認が要る変更: 個数の増加、背後実装の追加、数える道具の変更、`#[allow(unsafe_code)]` の範囲の変更。承認が出るまで自動でマージしない
- レビュー（手順 7）で足りる変更: 個数の減少、個数 0 の行の追加
- 記録は PR 本文の増減の表と PR 上の承認に置く。数値を別のファイルに二重に持たない
- 検査の限界: 個数テストは表と実物の一致だけを見る。承認が本物かは判定できない。期待値のテストファイルに所有者レビューを必須にする設定はリポジトリ設定で、本書では変えない
- この規則を機械で守る仕組みは無い

## 確かめ方

worktree のルートで実行する。

- `cargo test -p fandhe-silicon-contract --test backend_metal_unsafe_count`（`vulkan`・`cuda` も同様）: 背後実装ごとの個数
- `cargo test -p fandhe-silicon-contract --test common_unsafe_count_samples --test common_scanner_regression`: 数える道具
- `make verify`・`make build-profiles`: どの feature の組でも個数テストが走ること
- `make check`・`make deny`: 整形・依存

## 関連

- [coding-rust](../.claude/rules/coding-rust.md)
- [code-comment-style](../.claude/rules/code-comment-style.md)
- [security](../.claude/rules/security.md)
- [dependency-policy](../.claude/rules/dependency-policy.md)
- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)
- [contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md)
- [workspace-boundary](workspace-boundary.md)
