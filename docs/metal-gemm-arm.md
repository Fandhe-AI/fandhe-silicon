# Metal 直接の GEMM の腕の crate（TASK-1008）の所在と本リポの現状

TASK-1008 の作業のうち、測定専用の別 workspace `measure/` に置く腕の crate の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-1008。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 置き場 | `measure/crates/metal-gemm-arm`（package `measure-metal-gemm-arm`、lib `measure_metal_gemm_arm`） | `measure/Cargo.toml` の `members` |
| 依存 | `crates/core`（feature `test-support`）への path の 1 つだけ。外部依存は無い | `measure/crates/metal-gemm-arm/Cargo.toml` |
| lints | workspace の表を継ぐ（`unsafe_code = "forbid"`） | `[lints] workspace = true` |
| 流れる場所 | `make measure-verify` | `scripts/measure-verify.sh` |
| 門の枠 | `crates/core` の `parity` の module（TASK-1005 のポインタ。所在は [gemm-test-inputs](gemm-test-inputs.md)） | `crates/core/src/parity/` |

## カーネルのソースの取得と照合（2/5）

固定の rev・パス・sha256 は `src/source.rs` の定数（`SOURCE_REV`・`GEMM_SOURCE_PATH`・`GEMM_SOURCE_SHA256`）。値は本書に書かない。

| 関数 | 動き |
| ---- | ---- |
| `verify_source_against(bytes, expected)` | 長さが `MAX_SOURCE_BYTES` を超えれば `TooLarge`、sha256 が違えば `DigestMismatch`、UTF-8 でなければ `NotUtf8`、通れば `&str` |
| `verify_source(bytes)` | `GEMM_SOURCE_SHA256` で `verify_source_against` を呼ぶ |
| `load_source(path)` | 通常のファイルを上限つきで読み、`verify_source` を通ったものだけを `String` で返す。読めなければ `Unreadable` |

`SourceError` の文言は固定の英語で、パスやハッシュの値を含まない。

### bin `fetch-kernel-source`

使い方: `fetch-kernel-source <clone> <output>`。clone から `git show <rev>:<path>` で取り出し、`verify_source` を通ったときだけ `<output>` に新規作成で書く。取得したファイルは repo の中に置かない。

| 終了コード | 条件 |
| ---------- | ---- |
| 0 | 書いた |
| 1 | git の取得の失敗、上限超過、照合の失敗、書き込みの失敗（作ったファイルは消す） |
| 2 | 引数の数が違う、書き出し先が既にある、書き出し先を作れない |

- 書き出し先が既にあるかは git を起動する前に見る。上書きしない
- 標準エラーには固定の文言と引数の番号だけを出す。git の標準エラーは捨てる

### 試験

| ファイル | 内容 |
| -------- | ---- |
| `tests/source.rs` | 合成の入力での長さ・sha256・UTF-8 の拒否と受理、定数の形、`load_source` の失敗、`Display` の文言 |
| `tests/fetch_cmd.rs` | bin の引数の誤用、clone が無いときの終了コードと書き出し先が作られないこと、既にある書き出し先を変えないこと、標準エラーに引数の値が出ないこと |

確かめ方: `cargo test --manifest-path measure/Cargo.toml --locked -p measure-metal-gemm-arm`

## Metal 直接の GEMM の腕（3/5）

macOS 向けにだけ、Metal を直接呼んで門の 1 セルを 1 回だけ投入する腕を持つ。起動の表・詰め物・切り出しはどの OS でもコンパイルされる。

### 所在の補い

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 依存 | `crates/core` への path に加え、macOS 向けの外部依存 3 件（`objc2`・`objc2-metal`・`objc2-core-graphics`）。記録は `measure/deps-approved.toml` | `measure/crates/metal-gemm-arm/Cargo.toml`、`make deps-check` |
| lints | 自前の表（`unsafe_code = "deny"`、`unsafe_op_in_unsafe_fn`・`undocumented_unsafe_blocks`・`missing_safety_doc` は deny、`missing_docs` は warn） | `[lints.rust]`・`[lints.clippy]` |
| 起動の表 | `src/launch.rs` の `launch_for(cell)`。この腕の対象のセルだけが `Some` | `tests/launch.rs` |
| 詰め物・切り出し | `src/launch.rs` の `pad8_dim`・`pad_matrix`・`crop_matrix`。あふれと長さの食い違いは `Err` | `tests/launch.rs` |
| 型 | `src/lib.rs` の `ArmError`（固定の英語の文言）・`PipelineFacts` | `tests/launch.rs` |
| 橋の module | `src/metal.rs`。macOS 向けだけ。`run_g6` を `src/lib.rs` から出す | `src/lib.rs` |

### 腕の手順

`run_g6(cell, inputs, source)` は次の順で動き、投入は 1 回だけ。

1. 腕・dtype・入力の長さを照らす。食い違いは `Err`
2. A・B を実効の次元に詰める
3. 既定の device を得る。無ければ `Err`
4. コンパイルのオプションに数学モード `Safe`・関数の精度 `Precise` を設定する。設定の関数が無ければ `Err`。言語版は設定しない
5. ソースから library を作り、function constant を当てた入口の関数を作り、pipeline を作る
6. スレッド数が pipeline の上限を超えれば `Err`
7. 共有のバッファに A・B を書き、C を 0 で埋める
8. pipeline・バッファ・`setBytes`・threadgroup のメモリ長を設定して dispatch し、完了を待つ
9. 状態が完了でなければ `Err`。完了のときだけ C を読み、M×N に切り出して返す

### unsafe の置き場

- `unsafe` を書くのは `src/metal.rs` だけ。`#[allow(unsafe_code)]` は `src/lib.rs` の `mod metal;` の宣言にだけ付く
- すべての `unsafe` に `// SAFETY:` がある
- 個数は `tests/unsafe_count.rs` の表 `EXPECTED` が固定する。表に無いファイルは全種別 0 を期待する

### 試験

| ファイル | 内容 |
| -------- | ---- |
| `tests/launch.rs` | 起動の表の行の限界と式、function constant の index、dtype ごとの入口・割り当て、詰め物と切り出しの往復と失敗、`ArmError` の文言 |
| `tests/unsafe_count.rs` | `src/` の `unsafe` の個数と表の突き合わせ、数える道具の見本 |
| `src/metal.rs` の `tests` | `NSString` の往復、コンパイルのオプションの設定の読み戻し、function constant の設定（macOS でだけ流れる。device は作らない） |

確かめ方:

- `cargo test --manifest-path measure/Cargo.toml --locked -p measure-metal-gemm-arm`
- `cargo clippy --manifest-path measure/Cargo.toml -p measure-metal-gemm-arm --all-targets --target aarch64-apple-darwin --locked -- -D warnings`

## 門の run の device の試験（4/5）

門の 1 セルを腕で流す試験と、照合に合わないソースの拒否の試験を、macOS 向けに置く。どれも `#[ignore]` 付きで、device の要る専用の回でだけ `--ignored` で流す。既定の `cargo test` では流れない。

| ファイル | 試験 | 期待 |
| -------- | ---- | ---- |
| `tests/gate.rs` | 選ばれた 1 セルを枠の `run_selected_from_env` に腕を渡して流す | 出力が参照とビットで一致し、比べに外れが無い。検証レイヤーの下で流れる。pipeline の 2 つの値を標準出力に出し、起動の表の値と照らす |
| `tests/device.rs` | 照合に合わないソースの拒否 | `load_source` は `DigestMismatch`、`run_g6` は `SourceMismatch` を返す |

- `tests/gate.rs` は 1 つのテストのバイナリにつき 1 回だけの枠の入口を使うので、試験は 1 本だけ
- 照合済みのソースのパスは環境変数で渡す。値は表示にもエラーにも出さない。ソースは bin `fetch-kernel-source` で repo の外に書く
- 選ばれていない・不正・2 回目は失敗にし、成功や skip にしない

流すときの形:

```text
FANDHE_SILICON_G6_SOURCE=<repo の外のソースのパス> scripts/metal-session.sh run --combination <名前> --validation-layer --gate-cell <セルの識別名> -- cargo test --manifest-path measure/Cargo.toml --locked -p measure-metal-gemm-arm --test gate -- --ignored --nocapture
```

確かめ方:

- `cargo test --manifest-path measure/Cargo.toml --locked -p measure-metal-gemm-arm`（macOS では 2 つの試験のバイナリが ignored を数える。Linux では 0 本）
- `cargo clippy --manifest-path measure/Cargo.toml -p measure-metal-gemm-arm --all-targets --target aarch64-apple-darwin --locked -- -D warnings`

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 2/5 | カーネルのソースの取得と照合 | #6310 |
| 3/5 | Metal 直接の GEMM の腕 | #6311 |
| 4/5 | 門の run の device の試験 | #6312 |
