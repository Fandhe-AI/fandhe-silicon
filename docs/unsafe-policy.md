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
| `contract` の crate 直下 | `lib.rs` に `#![deny(unsafe_code)]`、`pub mod backend_{metal,vulkan,cuda};` の宣言と `cpu_ops.rs` の `mod neon;` の宣言に `#[allow(unsafe_code)]`（backend と `cpu_ops/neon.rs` の外に `unsafe` を書くとビルドが失敗する） | `crates/contract/src/lib.rs` |
| cfg ゲート | Metal は feature のみ、Vulkan・CUDA は feature かつ `not(target_arch = "wasm32")` | `crates/contract/src/lib.rs`、`crates/contract/tests/backend_cfg_gate.rs` |
| 個数テスト | `crates/contract/tests/backend_{metal,vulkan,cuda}_unsafe_count.rs`・`crates/contract/tests/cpu_ops_neon_unsafe_count.rs` と `crates/contract/tests/common/mod.rs`。道具を見本で確かめるテストは `common_unsafe_count_samples.rs` と `common_scanner_regression.rs` | `ls crates/contract/tests` |
| 外部依存 | `crates/contract` の背後実装だけが外部依存を持つ。Metal は macOS 向けに `objc2` 系の 4 件、CUDA は wasm32 以外に `cudarc` の 1 件（`deps-approved.toml`）。ほかの crate は 0 件。追加はユーザー承認制（[dependency-policy](../.claude/rules/dependency-policy.md)） | `Cargo.lock`・`deps-approved.toml` |

## `unsafe` を置ける場所

- 置けるのは `crates/contract/src/backend_<名前>.rs` と `crates/contract/src/backend_<名前>/` 配下と、`crates/contract/src/cpu_ops/neon.rs`（aarch64 の NEON の組み込み関数を呼ぶ橋）と、後述の比較の腕の crate だけ（[coding-rust](../.claude/rules/coding-rust.md)、AP-8 のポインタ）
- `core`・`exec`・`upper` は `#![forbid(unsafe_code)]` でコンパイラが拒否する
- `contract` の中の守り
    - 一次の守り: `crates/contract/src/lib.rs` の crate 直下に `#![deny(unsafe_code)]` を置き、`pub mod backend_<名前>;` の宣言と `cpu_ops.rs` の `mod neon;` の宣言にだけ `#[allow(unsafe_code)]` を付ける。backend と `cpu_ops/neon.rs` の外のファイルに `unsafe` を書くとビルドが失敗する（`forbid` は `allow` で外せないので `deny` を使う）
    - 二次の守り: 個数テストの枠（後述）が、`backend_<名前>/` 配下で期待値の表に無いファイルと、backend と `cpu_ops/neon.rs` の外のファイルの `unsafe` を検出する
    - `cpu_ops/neon.rs` は上の 2 つの守りの対象で、個数の期待値の表を `tests/cpu_ops_neon_unsafe_count.rs` に持つ。`#[allow(unsafe_code)]` は `cpu_ops.rs` の `mod neon;` の宣言だけに付ける。この module の変更は小節「承認」の PR ごとの承認を要する
- 比較の腕の crate（`baseline-wgpu/crates/`・`measure/crates/` の下で、下位の API〔Metal・MPS・wgpu の実験的な機能〕を呼ぶ橋。TASK-1008〜1011 のポインタ）にも置ける。製品の workspace（`crates/`）の範囲は変えない
    - `baseline-wgpu/`・`measure/` は製品の workspace の外の別 workspace（ルート `Cargo.toml` の `exclude`）で、腕の crate は `publish = false`。製品の crate（`crates/`）は腕の crate に依存しない（向きは測定・比較 → 製品の一方向。[poc-carry-over](../.claude/rules/poc-carry-over.md)）。したがって製品の crate の `#![forbid(unsafe_code)]`・置き場の制限・公開 API の safe の約束は変わらず、上位ライブラリの依存グラフに腕の crate の `unsafe` は入らない
    - 腕の crate は workspace の lints の `forbid(unsafe_code)` を継がず、自前の `[lints.rust]`・`[lints.clippy]` で `unsafe_code = "deny"` とし、`unsafe_op_in_unsafe_fn`・`clippy::undocumented_unsafe_blocks`・`clippy::missing_safety_doc` の deny と、その workspace のほかの lints も明記して保つ。`src/` の中の `#[allow(unsafe_code)]` は下位の API を呼ぶ 1 つの module（または関数）にだけ付ける（`tests/` の device の試験ファイルの扱いは下の箇条）
    - すべての `unsafe` に `// SAFETY:` を書く（`unsafe fn` は `/// # Safety`）。公開 API は safe なラッパーにする。ただし、実行時に受けたカーネル・シェーダーを走らせる入口は、橋の module の中の `unsafe fn` として公開してよい。走らせる前の照合・照らしが保証するのはソースの内容や形までで、カーネル・シェーダーがバッファを宣言どおりにだけ使うことは型や検査で確かめられないので、その条件を `/// # Safety` に呼び出し側の条件として書く（例: `measure/crates/metal-gemm-arm` の `src/metal.rs` の `run_g5`、`baseline-wgpu/crates/gemm-arm` の `src/bridge.rs` の `run_g2`・`run_g3prime`）
    - 前の検査を safe な関数に分けた crate では、誤用の試験はその safe な関数を呼び、`unsafe` を持たない（例: `baseline-wgpu/crates/gemm-arm` の `check_g2_request`・`check_g3prime_request`）
    - その `unsafe fn` を呼ぶ `#[ignore]` 付きの device の試験ファイル（`tests/` の下）に限り、ファイルの先頭に `#![allow(unsafe_code)]` を置ける。呼び出しの前提は、そのファイルの `unsafe { }` ブロックの `// SAFETY:` が担う。この `allow` は橋の module の外への `allow` に当たるので、下の箇条のとおり、置く PR とその範囲を変える PR ごとに小節「承認」の PR ごとの承認を要し、包括の承認の対象にしない（例: `measure/crates/metal-gemm-arm/tests/gate_g5.rs` は #8193、`baseline-wgpu/crates/gemm-arm/tests/gate_external.rs` は #8196、`baseline-wgpu/crates/gemm-arm/tests/gate_prime.rs` は #8200 のコメントに承認の記録がある）
    - 腕の crate ごとに `unsafe` の個数の試験を持ち、個数が変わるときは製品と同じ小節「個数が変わるときの扱い」に従う。個数の試験が数えるのは `src/` の下のファイルで、`tests/` の device の試験ファイルの `unsafe` は数えない
    - 比較の腕の crate で `#[allow(unsafe_code)]` を付ける PR（橋の module を 1 つ置く最初の PR を含む）と、その範囲を変える PR は、小節「承認」の PR ごとの承認を要し、包括の承認の対象にしない。承認のコメントは、その PR か、その PR が閉じる子の Issue に置いてよい（実装の前に置ける）。コメントには対象の crate と module（または関数）と承認日を書き、PR 本文から承認のコメントへリンクする
    - 腕の crate ごとに `allow` を付けるのは、下位の API を呼ぶ橋の module 1 つだけにする。2 つ目の module や橋の外の関数への `allow`、上の device の試験ファイルの先頭の `#![allow(unsafe_code)]`、腕の crate の外への置き場の変更、数える道具の変更は、この制限を超える変更として同じ PR ごとの承認を要する
    - 腕の crate の `unsafe` の個数だけが増える PR（`allow` の範囲が変わらないもの）のマージは、小節「承認」の既存の仕組み（PR ごとの承認、または包括の承認）に従う

## 不変条件の書式

lint が強制するのは「書いてあること」まで。本書は中身を決める。

| 対象 | 書く場所 | 書く内容 |
| ---- | -------- | -------- |
| `unsafe { }` ブロック | 直前の `// SAFETY:` | 成立させている不変条件、それを保証しているもの（呼び出し元の検証・型・FFI の契約）、該当する項目 ID のポインタ（あれば） |
| `unsafe impl` | 直前の `// SAFETY:` | 同上（`Send` / `Sync` なら共有・移動が安全である根拠） |
| `unsafe fn` | `/// # Safety` | 呼び出し側が満たす条件。本体の `unsafe` 操作は個別のブロックと `// SAFETY:` を持つ |
| `unsafe extern` ブロック | 直前の `// SAFETY:` | 宣言の署名（引数・戻りの型）が外部の関数の C の宣言と合う根拠と、リンクの根拠（どの framework・library に入っているか）。呼び出しの前提は、呼ぶ側の `unsafe { }` ブロックの `// SAFETY:` が担う |

- ブロックは呼び出し箇所ごとに分ける（1 ブロック = 1 組の不変条件。safe なコードを巻き込んで広げない）
- `unsafe extern` ブロックの `// SAFETY:` は clippy の `undocumented_unsafe_blocks` の対象の外なので、lint では強制されない。レビューで確かめる
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
| 登録点 | `common` の `BACKENDS` と、背後実装の外で `unsafe` を持てるファイルの表 `UNSAFE_MODULES`（`(名前, src/ からの相対パス)`）。登録した全背後実装と `UNSAFE_MODULES` の全ファイルのテストファイルの存在も検査する |
| cfg ゲート | 個数テストは feature・target でゲートしない。数える対象はソースの文字列でビルドの構成に依らず、どの feature の組でも走る |
| 走る場所 | `make verify` と `make build-profiles` の `cargo test`。GPU の実機は要らない |
| 背後実装を足すとき | `BACKENDS` への登録、同じ形のテストファイルを 1 つ、`lib.rs` の mod 宣言に `#[allow(unsafe_code)]` |

`check_backend` が行う検査: 登録済みの名前か／表の各行がその背後実装の持ち場か・重複が無いか／`include_str!` の中身が実ファイルと一致するか／個数の照合／持ち場の `.rs` が表に載っているか／持ち場の外のファイルに `unsafe` が無いか／`src/` 配下にシンボリックリンクが無いか／登録済みの全背後実装と `UNSAFE_MODULES` の全ファイルにテストファイルがあるか。`UNSAFE_MODULES` に載せたファイルは、その名前の `check_backend` の持ち場になる。失敗は集めて 1 回で返す。

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
| `cpu_ops/neon.rs` の行を照合し、登録の無いファイルを持ち場にしない | `check_backend_covers_only_the_registered_module`（`common_unsafe_count_samples.rs`）・`cpu_ops_neon_unsafe_count_matches_expected`（`cpu_ops_neon_unsafe_count.rs`） |
| feature を有効にした背後実装のモジュールが見える | `backend_metal_module_is_visible`・`backend_vulkan_module_is_visible`・`backend_cuda_module_is_visible`（`backend_cfg_gate.rs`） |

公開テストから到達させていない検査（コードにはある）: 表に無い `.rs` の検出、持ち場の外のファイルの `unsafe` の検出、シンボリックリンクの拒否、表の重複の検出、`BACKENDS` と `UNSAFE_MODULES` に無い名前の拒否。

## 個数が変わるときの扱い

期待値の表（`crates/contract/tests/backend_<名前>_unsafe_count.rs` と `crates/contract/tests/cpu_ops_neon_unsafe_count.rs` の `EXPECTED`）を変える PR の手順。承認の要否は小節「承認」に書く。

### 対象になる変更

個数が変わらない変更（表に触れない変更）は対象外。

| 変更 | 個数テストの反応 | 表に対して行うこと |
| ---- | ---------------- | ------------------ |
| 既存のファイルで `unsafe` が増える・減る | `<path>: <種別>: expected <期待>, actual <実際>` で失敗 | その行の `expected` を種別ごとに直す |
| `backend_<名前>/` 配下に `.rs` を足す | `not listed in the expected table` で失敗（`unsafe` が 0 でも） | `path`・`include_str!`・`expected` の行を足す |
| 表にあるファイルを消す・改名する | `include_str!` のコンパイルエラー | 行を消す・直す |
| 背後実装を足す | 登録したがテストファイルが無いと失敗 | `BACKENDS` への登録、テストファイル 1 つ、`lib.rs` の mod 宣言の `#[allow(unsafe_code)]` |
| 数える道具（`tests/common/mod.rs`）の数え方を変える | 見本のテスト（`common_unsafe_count_samples.rs`）が失敗しうる | 見本を同じ PR で直す。表が変わるなら増減として扱う |

- backend と `cpu_ops/neon.rs` の外のファイルに `unsafe` を置く変更は、この手順では通せない（`lib.rs` の `deny` でビルドが失敗する）

### 手順

1. `unsafe` が要るかを確かめる。safe な書き方で足りるなら足さない。置き場は `backend_<名前>` の中と `cpu_ops/neon.rs` だけ
2. `// SAFETY:`・`/// # Safety` を「不変条件の書式」どおりに書く（ブロックは呼び出し箇所ごと）
3. `cargo test -p fandhe-silicon-contract --test backend_<名前>_unsafe_count`（`cpu_ops/neon.rs` は `--test cpu_ops_neon_unsafe_count`）を走らせ、失敗の出力からファイル・種別ごとの期待と実際の差を読む
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
- `#[allow(unsafe_code)]` を backend・`cpu_ops` の NEON の module・比較の腕の crate の下位の API を呼ぶ module（または関数）以外に付ける（比較の腕の crate の `unsafe fn` を呼ぶ `#[ignore]` 付きの device の試験ファイルの先頭に、PR ごとの承認で置く `#![allow(unsafe_code)]` を除く）。`deny(unsafe_code)`・`forbid(unsafe_code)`・workspace の lints を緩める
- 個数テストを `#[ignore]` や cfg で外す、検査を弱める
- `git commit --no-verify` を使う
- 文書・README・コメントに個数の値や変化の履歴を持つ

### 承認

本リポの運用の規則。

- オーナーの明示の承認が要る変更: 個数の増加、背後実装の追加、数える道具の変更、`#[allow(unsafe_code)]` の範囲の変更。承認が出るまで自動でマージしない
- 個数の増加・背後実装の追加の承認は、PR ごとの承認のほか、オーナーが範囲を限って与える包括の承認でもよい。包括の承認は、TASK-9 の公開側の親 Issue（#50）のコメントに、対象と承認日を書いて置く
    - 包括の承認の下でマージする PR は、手順 7 の security-auditor と reviewer のレビューに P0・P1 の指摘が無く、PR 本文に増減の表と包括の承認のコメントへのリンクがあること。どれかを欠く PR は、PR ごとの承認を待つ
    - 数える道具の変更と `#[allow(unsafe_code)]` の範囲の変更は、守りそのものを弱めうるため、包括の承認の対象にしない（PR ごとの承認だけ）
- レビュー（手順 7）で足りる変更: 個数の減少、個数 0 の行の追加
- 記録は PR 本文の増減の表と、PR 上の承認または包括の承認のコメントへのリンクに置く（PR ごとの承認は、その PR が閉じる子の Issue のコメントへのリンクでもよい）。数値を別のファイルに二重に持たない
- 検査の限界: 個数テストは表と実物の一致だけを見る。承認が本物かは判定できない。期待値のテストファイルに所有者レビューを必須にする設定はリポジトリ設定で、本書では変えない
- この規則を機械で守る仕組みは無い

## 確かめ方

worktree のルートで実行する。

- `cargo test -p fandhe-silicon-contract --test backend_metal_unsafe_count`（`vulkan`・`cuda` も同様）: 背後実装ごとの個数
- `cargo test -p fandhe-silicon-contract --test cpu_ops_neon_unsafe_count`: `cpu_ops/neon.rs` の個数
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

## 今の値の補い（base 55fd9c1）

記録時点の表と既存の節は書き換えない。以下は base `55fd9c1` で観測した今の値（実行環境は GPU なしの Linux）。

| 表の行 | 前の値 → 今の値 | 確かめ方 |
| ------ | --------------- | -------- |
| 所在の表の「個数テスト」の行 | 製品の `crates/contract/tests/` の個数テストだけを挙げている → 別 workspace の比較の腕の crate にも個数テストがある。`measure/crates/metal-gemm-arm/tests/unsafe_count.rs`（`src/metal.rs` の 1 行だけを表に持ち、block 11・ほか 0。5 件のテスト）と `baseline-wgpu/crates/gemm-arm/tests/unsafe_count.rs`（`src/bridge.rs` の 1 行だけを表に持ち、block 1・ほか 0。7 件のテスト）。どちらも数える道具をそのファイルの中に持ち、製品の crate のテストの道具には依存しない。`#[allow(unsafe_code)]` は腕の crate ごとに `src/lib.rs` の 1 箇所（`measure/crates/metal-gemm-arm/src/lib.rs` の 17 行目、`baseline-wgpu/crates/gemm-arm/src/lib.rs` の 10 行目） | `ls measure/crates/metal-gemm-arm/tests baseline-wgpu/crates/gemm-arm/tests`、`grep -n 'EXPECTED' -A2 measure/crates/metal-gemm-arm/tests/unsafe_count.rs baseline-wgpu/crates/gemm-arm/tests/unsafe_count.rs`、`cargo test --offline --manifest-path measure/Cargo.toml -p measure-metal-gemm-arm --test unsafe_count`（5 passed）、`cargo test --offline --manifest-path baseline-wgpu/Cargo.toml -p baseline-wgpu-gemm-arm --test unsafe_count`（7 passed）、`grep -rn 'allow(unsafe_code)' measure/crates/metal-gemm-arm/src baseline-wgpu/crates/gemm-arm/src` |

## 今の値の補い（base 5fa953d）

記録時点の表と既存の節は書き換えない。以下は base `5fa953d` で観測した今の値（実行環境は GPU なしの Linux）。

| 節・行 | 前の値 → 今の値 | 確かめ方 |
| ------ | --------------- | -------- |
| 前の補い（base 55fd9c1）の「個数テスト」の行（別 workspace の比較の腕の crate の個数テスト） | `measure/crates/metal-gemm-arm/tests/unsafe_count.rs` と `baseline-wgpu/crates/gemm-arm/tests/unsafe_count.rs` の 2 つ → 3 つ目として `baseline-wgpu/crates/mps/tests/unsafe_count.rs` がある。`src/mps.rs` の 1 行だけを表（`EXPECTED`）に持ち、block 8・extern block 1・ほか 0。テストは 7 件。数える道具はそのファイルの中に持ち、製品の crate のテストの道具には依存しない | `sed -n '/^const EXPECTED/,/^)\];/p' baseline-wgpu/crates/mps/tests/unsafe_count.rs`、`cargo test --manifest-path baseline-wgpu/Cargo.toml -p baseline-wgpu-mps --test unsafe_count`（7 件通過） |
| 同「`#[allow(unsafe_code)]` の所在」 | 腕の crate ごとに下位の API を呼ぶ module の宣言にだけ付く → `baseline-wgpu/crates/mps` では `src/lib.rs` の `mod mps;` の宣言（49 行目。`#[cfg(target_os = "macos")]` の次の行）にだけ付く | `grep -n 'allow(unsafe_code)' baseline-wgpu/crates/mps/src/lib.rs` |

## 今の値の補い（base ae152fb）

記録時点の表と既存の節は書き換えない。以下は base `ae152fb` で観測した今の値（実行環境は GPU なしの Linux）。

| 節・行 | 前の値 → 今の値 | 確かめ方 |
| ------ | --------------- | -------- |
| 前の補い（base 55fd9c1）の「個数テスト」の行の `measure/crates/metal-gemm-arm` の分（表は `src/metal.rs` の 1 行だけで block 11・ほか 0、`#[allow(unsafe_code)]` は `src/lib.rs` の 17 行目） | block 11 → block 15・ほか 0（表は今も `src/metal.rs` の 1 行だけ。`src/metallib.rs` は表に無く、全種別 0 を期待する）。`#[allow(unsafe_code)]` は 17 行目 → 21 行目（直後の 22 行目が `mod metal;`）。テストは今も 5 件 | `grep -n 'const EXPECTED' measure/crates/metal-gemm-arm/tests/unsafe_count.rs`、`grep -rn 'allow(unsafe_code)' measure/crates/metal-gemm-arm/src`、`cargo test --offline --manifest-path measure/Cargo.toml -p measure-metal-gemm-arm --test unsafe_count`（5 passed） |
| 前の補い（base 5fa953d）の表の `measure/crates/metal-gemm-arm` の分 | 上の行のとおり（前の補い（base 5fa953d）は `measure/crates/metal-gemm-arm` の値を直していない） | 同上 |

## 今の値の補い（base 2446902）

記録時点の表と既存の節は書き換えない。以下は base `2446902` で観測した今の値（実行環境は GPU なしの Linux）。

| 節・行 | 前の値 → 今の値 | 確かめ方 |
| ------ | --------------- | -------- |
| 前の補い（base ae152fb）の `measure/crates/metal-gemm-arm` の行（表は `src/metal.rs` の 1 行だけで block 15・ほか 0、`#[allow(unsafe_code)]` は `src/lib.rs` の 21 行目） | block 15・ほか 0 → block 15・fn 1・ほか 0（`unsafe fn` は `run_g5`。表は今も `src/metal.rs` の 1 行だけ）。`#[allow(unsafe_code)]` は 21 行目 → 25 行目（直後の 26 行目が `mod metal;`）。テストは今も 5 件 | `grep -n 'const EXPECTED' measure/crates/metal-gemm-arm/tests/unsafe_count.rs`、`grep -n 'allow(unsafe_code)' measure/crates/metal-gemm-arm/src/lib.rs`、`cargo test --offline --manifest-path measure/Cargo.toml -p measure-metal-gemm-arm --test unsafe_count`（5 passed） |
| 前の補い（base 55fd9c1）の「個数テスト」の行の `baseline-wgpu/crates/gemm-arm` の分（表は `src/bridge.rs` の 1 行だけで block 1・ほか 0、`#[allow(unsafe_code)]` は `src/lib.rs` の 10 行目） | block 1・ほか 0 → block 10・fn 2・ほか 0（`unsafe fn` は `run_g2`・`run_g3prime`。表は今も `src/bridge.rs` の 1 行だけ）。`#[allow(unsafe_code)]` は 10 行目 → 17 行目（直後の 18 行目が `mod bridge;`）。テストは今も 7 件 | `sed -n '/^const EXPECTED/,/^)\];/p' baseline-wgpu/crates/gemm-arm/tests/unsafe_count.rs`、`grep -n 'allow(unsafe_code)' baseline-wgpu/crates/gemm-arm/src/lib.rs`、`cargo test --offline --manifest-path baseline-wgpu/Cargo.toml -p baseline-wgpu-gemm-arm --test unsafe_count`（7 passed） |
| 前の補い（base 55fd9c1）の「`#[allow(unsafe_code)]` は腕の crate ごとに `src/lib.rs` の 1 箇所」 | `src/lib.rs` の 1 箇所 → `src/lib.rs` の 1 箇所に加えて、`unsafe fn` を呼ぶ `#[ignore]` 付きの device の試験ファイルの先頭（8 行目）にも `#![allow(unsafe_code)]` がある。`measure/crates/metal-gemm-arm/tests/gate_g5.rs`・`baseline-wgpu/crates/gemm-arm/tests/gate_external.rs`・`baseline-wgpu/crates/gemm-arm/tests/gate_prime.rs` | `grep -n 'allow(unsafe_code)' measure/crates/metal-gemm-arm/tests/gate_g5.rs baseline-wgpu/crates/gemm-arm/tests/gate_external.rs baseline-wgpu/crates/gemm-arm/tests/gate_prime.rs` |
