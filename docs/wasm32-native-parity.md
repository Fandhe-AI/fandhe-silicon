# wasm32 とネイティブの出力の比べの所在と本リポの現状

別 workspace `wasm/` が持つ、wasm32 の出力とネイティブの出力を比べるための道具の所在と、本リポの現状の事実をまとめる。

## 記録の所在

記録の正は private spec 側（`docs/spec/05-tasks.md` の TASK-994。ポインタのみ）。本文は転記しない
（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## GEMM の scalar のビルドの比べ（TASK-994）

### 入力とネイティブの参照を書き出す道具（1/5）

#### 所在

`wasm/crates/parity-host/`（package `wasm-parity-host`。lib `wasm_parity_host` と bin `wasm-parity-host`）。
別 workspace `wasm/` の member で、依存は path の 2 つだけ。

| 依存 | feature |
| ---- | ------- |
| `fandhe-silicon-contract` | `contract-core`・`cpu-isa`・`cpu-ops`（glue と同じ組） |
| `fandhe-silicon-core` | `test-support` |

外部の crate には依存しない。`[dev-dependencies]` も持たない。

#### 実行の口と終了コード

```console
wasm-parity-host gen <dir>
```

引数はちょうど `gen <dir>` の 2 つ。`<dir>` は既存のディレクトリで、環境変数は読まない。
ファイルは新しく作り（`create_new`）、既存のものは上書きしない。成功のとき標準出力に何も出さない。

| 場面 | 終了コード | 標準エラー |
| ---- | ---------- | ---------- |
| 成功 | 0 | 空 |
| 引数が `gen <dir>` の形でない | 2 | `usage: wasm-parity-host gen <dir>` |
| `<dir>` が無い | 2 | `error: output directory does not exist` |
| 書く名前のファイルが既にある | 2 | `error: output file already exists` |
| 書けない | 2 | `error: cannot write output file` |
| 入力を作れない | 2 | `error: cannot generate inputs` |
| `DefaultCpuOps::gemm_scalar` が `Err` | 2 | `error: native gemm_scalar failed` |

文言は固定で、渡したパスの値を含まない。入力と参照を全部作り、書く名前（10 個）の不在を確かめてから書き始める。
入力・参照の失敗と名前の衝突のときは何も書かない。`manifest.json` は最後に書く。

#### ケース

定数 `GEMM_CASES`（`wasm/crates/parity-host/src/lib.rs`）が持つ。

| 名前 | m | k | n | 入力の作り方 |
| ---- | -- | -- | -- | ------------ |
| `gemm-64` | 64 | 64 | 64 | `SplitMix64::new(GEMM_INPUT_SEED)` から `next_signed_unit_f32` で `a`（m×k）、続けて同じ生成器で `b`（k×n） |
| `gemm-256` | 256 | 256 | 256 | 同上（ケースごとに生成器を作り直す） |
| `gemm-37x41x53` | 37 | 53 | 41 | `generate_f32(GemmGateRow::Odd37x41x53)` の `a()`・`b()` |

ネイティブの参照は、同じ入力を `DefaultCpuOps.gemm_scalar(a, b, m, k, n, &mut out)` に渡した `out`（長さ m×n、初期値 `0.0`）。

#### 書くファイル

| ファイル | 中身 |
| -------- | ---- |
| `<ケース>.a.bin` | `a` の f32 を添字の順にリトルエンディアンで並べたもの |
| `<ケース>.b.bin` | `b` の同様の列 |
| `<ケース>.native-scalar.bin` | 参照 `out` の同様の列 |
| `manifest.json` | `cases` の配列。ケースごとに `name`・`kind`（`"gemm"`）・`m`・`k`・`n`・`a_file`・`b_file`・`native_scalar_file`・`a_sha256`・`b_sha256`・`native_scalar_sha256` を 1 欄 1 行で持つ |

ファイル名は `[a-z0-9.-]` だけ。sha256 は各ファイルのバイト列の `fandhe_silicon_core::hash::sha256_bytes` で、小文字 64 桁の 16 進。

#### 試験

`wasm/crates/parity-host/tests/gen.rs`。期待の値は生成器と `DefaultCpuOps` の呼び出しで作る。

| 試験 | 確かめること |
| ---- | ------------ |
| `square_cases_follow_fresh_generator` | 64・256 の `a` の後に `b` を並べた列が、新しい生成器から続けて取った列とビット列で同じ |
| `odd_case_matches_gate_row` | 37×41×53 の `a`・`b` が `generate_f32(GemmGateRow::Odd37x41x53)` とビット列で同じ。`m`・`k`・`n` が 37・53・41 |
| `inputs_are_finite` | 6 つの入力の全要素が有限 |
| `native_reference_matches_default_cpu_ops` | 書いた `.native-scalar.bin` を読み戻した列が、同じ入力の `DefaultCpuOps.gemm_scalar` の出力とビット列で同じ（3 ケース） |
| `manifest_hashes_match_files` | manifest の各 sha256 が書いたファイルのバイト列の sha256 と同じ。ファイルの長さが m×k・k×n・m×n の 4 倍。ファイル名が `[a-z0-9.-]` だけ。種類が `gemm` |
| `gen_refuses_existing_files` | 同じディレクトリに 2 回当てると 2 回目が失敗し、1 回目のファイルの中身が変わらない |
| `gen_rejects_bad_arguments` | 引数の数の違い・`gen` 以外の語・無いディレクトリで失敗する |
| `binary_reports_exit_codes` | 実行ファイルの終了コードが上の表のとおりで、成功のとき標準出力が空、失敗のとき標準エラーに渡したパスの値が出ない |

#### 確かめ方

時点のコミット: `5cf1d33`（この変更の土台）。

```console
$ cargo test --manifest-path wasm/Cargo.toml --workspace --locked 2>&1 | grep '^test result:'
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.30s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

一時のディレクトリに当てた出力（`d` は `mktemp -d` のディレクトリ）。

```console
$ cargo run -q --manifest-path wasm/Cargo.toml -p wasm-parity-host --locked -- gen "$d"
$ (cd "$d" && ls && sha256sum *)
gemm-256.a.bin
gemm-256.b.bin
gemm-256.native-scalar.bin
gemm-37x41x53.a.bin
gemm-37x41x53.b.bin
gemm-37x41x53.native-scalar.bin
gemm-64.a.bin
gemm-64.b.bin
gemm-64.native-scalar.bin
manifest.json
0d509543d75aff9fa7372ca461e5c91dd8e9d8e41c81fe3b302e6610b1228b89  gemm-256.a.bin
e5aae545d81ea7cf7b62df1d03c79055a67967826d0b74ea02c0c68063fbd912  gemm-256.b.bin
d7bc8e3c3e96f39fd560a67bc9446532ce233ee46b805c44d59504cdf39956c1  gemm-256.native-scalar.bin
8b6bdc24887e48cd2bc3598950e2949c599f27b9c14d2a2c11384d2bb9c454d1  gemm-37x41x53.a.bin
60efc5337595142f4ec8d78f08e8e2663f4a67630672e1d21d7cc2f21f90e19e  gemm-37x41x53.b.bin
a0bb2a40eeebf8093a0236ffac4f9c053d7c8fca2267c1e08a3cdd40a5551d2b  gemm-37x41x53.native-scalar.bin
2259215901630df152c4b1e0df09edf21118c82c285b4a9968901fca8ca2ca52  gemm-64.a.bin
9d19ef5ea664105fe1cfbdc69868867fcecde4652eb739d1912527d4652d3eeb  gemm-64.b.bin
25db4c0dde78a6a12cf5449e4c79542e7be1f742993c243a42a31519e8fa6f11  gemm-64.native-scalar.bin
494ee7ead8543b9454f035f7af2fa058823208a8e2b4be8e38bf6193bb1c4076  manifest.json
```

定数と関数の所在は `grep -n "GEMM_CASES\|pub fn" wasm/crates/parity-host/src/lib.rs` で確かめる。

### 生成の入力の行と 2 つのビルド（2/5）

`scripts/wasm-node-check.sh` と `wasm/node/run-glue.cjs` が、1/5 の道具の出力を読む行と、scalar・simd128 の 2 つのビルドを扱う。

#### 行の 2 つの形（`wasm/node/cases.json`）

| 形 | 欄 | `export` | `reference_source` |
| -- | -- | -------- | ------------------ |
| 固定の行 | `id`・`export`・`input`・`reference_source` と任意の `reference`・`build`（無ければ `scalar`） | `roundtrip_f32`・`detect_isa_bits`（`input` は空の列だけ） | `fixed`・`native-neon`（`native-scalar` は終了コード 2） |
| 生成の行 | ちょうど `id`・`export`・`build`・`case`・`reference_source` | `gemm_scalar`・`gemm_simd` | `native-scalar`・`native-neon`（`fixed` は終了コード 2） |

生成の行は値を JSON に書かない。`case` は 1/5 の道具の manifest にあるケースの名前で、manifest を読んだ後に照らす（未知は終了コード 2）。
欄の過不足・未知の `build`・未知の `export`・形の混ざりも終了コード 2 で、Node の本体は起動しない。
既定の入力に足した行は `isa-scalar`（`build` が `scalar`、参照 `00000000`）と `isa-simd128`（`build` が `simd128`、参照 `00000001`）の 2 行。

#### 2 つのビルド

| ビルド | cargo の呼び方 | `--wasm` 系の口 |
| ------ | -------------- | --------------- |
| scalar | `cargo build --manifest-path wasm/Cargo.toml -p wasm-glue --locked --target wasm32-unknown-unknown --profile wasm-size`（`CARGO_ENCODED_RUSTFLAGS` は空） | `--wasm <path>` |
| simd128 | 同上に `--target-dir <target>/node-simd128`、`CARGO_ENCODED_RUSTFLAGS` に `-C` と `target-feature=+simd128`（`\x1f` 区切り） | `--wasm-simd128 <path>` |

どちらも `RUSTFLAGS`・`CARGO_BUILD_RUSTFLAGS`・`CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS` は `env -u` で外す。
行が使うビルドだけを作る。口を 1 つでも渡したら cargo でビルドせず、行が使うビルドの `.wasm` が無ければ終了コード 2。
`wasm-bindgen --target nodejs` はビルドごとに別のディレクトリへ出す。

#### host 側の道具の呼び方

生成の行が 1 行以上あるときだけ、`cargo run --manifest-path wasm/Cargo.toml -p wasm-parity-host --locked --quiet -- gen <work>/inputs` を呼ぶ
（`--wasm` を渡していても cargo が要る。標準出力・標準エラーは標準エラーへ。0 以外は終了コード 2）。
manifest は python3 で読み、各ファイルの sha256・長さ・通常のファイルであること・ファイル名（`[a-z0-9.-]` だけで `..` と先頭の `.` を含まない）を照らす。違えば終了コード 2。

#### `run-glue.cjs` の呼び方

```console
node wasm/node/run-glue.cjs <scalar の glue|-> <cases> <results> <simd128 の glue|-> <inputs|-> <outputs>
```

引数はちょうど 6 つ（数が違えば usage で 2）。行の `build` が `simd128` なら simd128 の glue、それ以外は scalar の glue を使う。
許可表は `roundtrip_f32`・`detect_isa_bits`（戻りを `>>> 0` にして 1 語の 16 進で書く）・`gemm_scalar`・`gemm_simd`。
gemm は `.bin` を新しい `ArrayBuffer` に写して `Float32Array` にし、長さ m×n の `out` を `0x7fc0dead` で埋めて渡す。
戻りが 0 のときだけ `out` のバイト列を `<outputs>/<行の id>.bin` に書き、結果に `output_file` を書く。0 以外は `nonzero return`、例外は `call failed`。

#### 判定と出力の行

| 場面 | 状態 |
| ---- | ---- |
| 出どころが `PRODUCIBLE`（`fixed`・`native-scalar`）に無い（`native-neon`） | `not-run (reference-unavailable)`。比べず、`count:`・`sha256:` を出さない |
| 呼び出しの失敗 | `mismatch (call error)`。`count:`・`sha256:` は出さない |
| 生成の行で語の数が違う | `mismatch (length expected <n> got <m>)`（語の数）。`count:`・`sha256:` は出さない |
| 生成の行で 4 バイトの語ごとの比べに違いがある | `mismatch (mismatches <件数>/<語の数>)` |
| 生成の行で全語が同じ | `match` |

標準出力の並びは、`row:`（入力の行の順）、語ごとに比べた生成の行の `count: <id> mismatches=<件数> n=<語の数>` と `sha256: <id> <64 桁>`、
`native-scalar` の生成の行が指すケースの `sha256: <ケース>.a`・`.b`・`.native-scalar`（manifest の順）、最後に `summary:`。
終了コードの規則は変わらない。

#### 参照の出どころ

トークンは `fixed`・`native-neon`・`native-scalar`。所在は `crates/core/src/rerun/reference.rs` の `ReferenceSource`（TASK-29 のポインタ）。
この環境で作れる出どころはスクリプトの `PRODUCIBLE` が持つ。

#### 回帰テスト

`scripts/test-wasm-node-check.sh` の足した項目（22 と 23 は生成の行も扱う形に直した）。

| 項目 | 確かめること |
| ---- | ------------ |
| 22 | 同梱の入力と、生成の行を 9 行足した合成の入力が全行 `match` |
| 23・23b | `run-glue.cjs` を本物の node で偽の glue に当て、`out` の埋め・戻り 0 のときだけ書くこと・ビット列を保つこと・ビルドごとの glue の選択 |
| 37 | 生成の行が全部一致のときの `count:`・`sha256:` の行と並び |
| 38〜40 | 1 語違い（NaN のペイロード・`-0.0`）・語の数の違い・呼び出しの失敗 |
| 41・42 | `isa-simd128` の値違い、`native-neon` の生成の行 |
| 43〜45 | 行の形の不正・manifest の不正・結果の形の不正（どれも終了コード 2） |
| 46〜48 | `--wasm`・`--wasm-simd128` の口、ビルドの形、host 側の道具の呼び方と失敗 |
| 49 | `--record-dir` で生成の行も部品になる |

#### 確かめ方

```console
$ make test-wasm-node-check 2>&1 | tail -n 1
$ grep -n "PRODUCIBLE\|GEN_EXPORTS" scripts/wasm-node-check.sh
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/5 | GEMM の入力とネイティブの scalar の参照を書き出す host 側の道具 | #6271 |
| 2/5 | Node での照合を生成の入力の行と 2 つのビルドに広げる | #6272 |
