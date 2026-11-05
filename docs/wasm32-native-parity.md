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

### GEMM の 9 行（3/5）

`wasm/node/cases.json` の既定の入力に足した生成の行。並びは固定の行の後に、ケースの順・組の順。

| id | ビルド | export | ケース | 参照の出どころ |
| -- | ------ | ------ | ------ | -------------- |
| `gemm-64-scalar-gemm-scalar` | `scalar` | `gemm_scalar` | `gemm-64` | `native-scalar` |
| `gemm-64-scalar-gemm-simd` | `scalar` | `gemm_simd` | `gemm-64` | `native-scalar` |
| `gemm-64-simd128-gemm-scalar` | `simd128` | `gemm_scalar` | `gemm-64` | `native-scalar` |
| `gemm-256-scalar-gemm-scalar` | `scalar` | `gemm_scalar` | `gemm-256` | `native-scalar` |
| `gemm-256-scalar-gemm-simd` | `scalar` | `gemm_simd` | `gemm-256` | `native-scalar` |
| `gemm-256-simd128-gemm-scalar` | `simd128` | `gemm_scalar` | `gemm-256` | `native-scalar` |
| `gemm-37x41x53-scalar-gemm-scalar` | `scalar` | `gemm_scalar` | `gemm-37x41x53` | `native-scalar` |
| `gemm-37x41x53-scalar-gemm-simd` | `scalar` | `gemm_simd` | `gemm-37x41x53` | `native-scalar` |
| `gemm-37x41x53-simd128-gemm-scalar` | `simd128` | `gemm_scalar` | `gemm-37x41x53` | `native-scalar` |

#### 確かめ方

```console
$ grep -c '"case"' wasm/node/cases.json
9
$ make test-wasm-node-check 2>&1 | tail -n 1
passed: 229, failed: 0
```

CI の `wasm-macos` ジョブの `make wasm-node-check` の行は次で読む（`<id>` は job の id）。

```console
$ gh run view --job <id> --log | grep -E ' (row|count|sha256|summary): '
```

### 出力の保存と数え直し（4/5）

`scripts/wasm-node-check.sh` の口 2 つ（`--output-dir`・`--recount`）と、実行の中の数え直し。対象は参照の出どころが `native-scalar` の生成の行だけで、`native-neon` の生成の行は保存の形にも `recount:` の行にも入らない。

#### 保存の形のファイル

| ファイル | 中身 |
| -------- | ---- |
| `manifest.json` | host 側の道具の出力のバイト列そのまま |
| manifest の各ケースの `a_file`・`b_file`・`native_scalar_file` | host 側の道具の出力のバイト列そのまま（行が使わないケースも入る） |
| `<行の id>.bin` | wasm の出力。`native-scalar` の生成の行で出力があるものだけ（語の数が参照と違うものも入る） |
| `outputs.json` | `{"rows": [...]}`。行は `id`・`export`・`build`・`case`・`output_file`・`output_sha256`。呼び出しの失敗の行は後ろの 2 欄が `null` |

#### 口

| 口 | 要るもの | 終了コード |
| -- | -------- | ---------- |
| `--output-dir <dir>` | 実行と同じ（node・wasm-bindgen・cargo・python3）。`<dir>` は絶対パス・`..` 無し・存在する・ディレクトリ・シンボリックリンクでない（`.wasm` のビルドより前に検査）。`native-scalar` の生成の行が無い入力では 2。`O_EXCL` とリンクを辿らない書き方で新しく書き、失敗したらこの実行で作ったものを消す | 実行と同じ（0 全行 match・1 mismatch か not-run・2 誤用か前提の不足か入力の不正） |
| `--recount <dir>` | python3 だけ。ほかの口とは併用できない | 0 全行の件数が 0 で比べなかった行も無い・1 件数が 0 でない行か比べなかった行がある・2 誤用・読めない・形の不正・sha256 の違い・想定外のファイル |

`--output-dir` を選ばないとき、標準出力と終了コードは変わらない（下の `recount:` の行を除く）。`--record-dir` と `--record-combination` の書き出しは別の口のまま変わらない。

#### `--recount` が照らすもの

`outputs.json` の形、`manifest.json` の各ファイルの名前・sha256・長さ、`<id>.bin` の sha256 を照らし、ディレクトリの中身が期待の集合と完全に一致すること（余り・欠け・シンボリックリンク・サブディレクトリは 2）を確かめる。そのうえで行ごとに wasm の出力と `.native-scalar.bin` を 4 バイトの語ごとに比べる。

| 出力の行 | 意味 |
| -------- | ---- |
| `recount: <id> mismatches=<件数> n=<語の数>` | 比べた行 |
| `recount-uncompared: <id>` | 比べなかった行（出力が無い行、語の数が参照と違う行） |
| `sha256: <id> <64 桁>` と `sha256: <ケース>.a`・`.b`・`.native-scalar` | 実行のときの `sha256:` の行と同じ形と並び |
| `recount-summary: rows=<n> zero=<n> nonzero=<n> uncompared=<n>` | 最後の行 |

#### 実行の中の数え直し

`native-scalar` の生成の行が 1 行以上ある実行は、判定のあと作業の一時ディレクトリに保存の形を作り、`--recount` と同じ関数でそのディレクトリだけから数え直す。判定の件数・比べなかった行の集合と 1 つでも違えば、`row:` や `summary:` を出さず `error: recount disagrees` で終了コード 2 にする（`--output-dir`・`--record-dir` にも何も書かない）。違わなければ `recount:`・`recount-uncompared:` の行と `recount-summary:` の行を `summary:` の前に出す。`sha256:` の行は実行のときの 1 組だけで、数え直しでは重ねて出さない。

#### 回帰テスト

`scripts/test-wasm-node-check.sh` の項目 51 以降。

| 項目 | 確かめること |
| ---- | ------------ |
| 51〜55 | `--output-dir` の名前の集合・中身・sha256、出力の不変、上書きしない・リンクを辿らない・`--record-dir` の失敗で消える、不正な書き出し先と値の欠け、`native-scalar` の生成の行が無い入力、`native-neon` の生成の行が入らないこと |
| 56〜60 | `--recount` の 0・1・2（sha256 の違い・欠け・余り・サブディレクトリ・リンク・壊れた JSON・鍵の過不足・深い入れ子など）、ほかの口との組、node・wasm-bindgen・cargo を要らないこと |
| 61〜63 | 実行の中の `recount:` の行と並び、不一致の実行、呼び出しの失敗・語の数の違いの行 |
| 64 | 数え直しの比べの式だけを変えた変異体で、判定と食い違うと 2 になり何も書かないこと |
| 65 | `--output-dir` を選ばない実行が一時ディレクトリに何も残さないこと |

#### 確かめ方

```console
$ make test-wasm-node-check 2>&1 | tail -n 1
passed: 328, failed: 0
$ grep -n "recount_form\|def write_new" scripts/wasm-node-check.sh
```

合成の入力で `--recount` を流す手順は、host 側の道具の出力のディレクトリに `<行の id>.bin`（参照と同じバイト列）と `outputs.json` を足した保存の形に対して行う（実行した出力の先頭の 3 行）。

```console
$ scripts/wasm-node-check.sh --recount <dir>
recount: gemm-64-scalar-gemm-scalar mismatches=0 n=4096
recount: gemm-256-scalar-gemm-scalar mismatches=0 n=65536
recount: gemm-37x41x53-scalar-gemm-scalar mismatches=0 n=1517
```

CI の `wasm-macos` ジョブの `make wasm-node-check` の `recount:` の行は次で取り出す（`<id>` は job の id）。

```console
$ gh run view --job <id> --log | grep -E ' (recount|recount-summary): '
```

### 完了の条件の確かめ（5/5）

本リポで観測できる事実を、コマンドと出力だけで記録する（TASK-994）。

時点のコミット: `55c2789`（この変更の土台）。(2)〜(6) は、このコミットの main への push で流れた CI の run `42000912040` から取る（event `push`・conclusion `success`・headSha は `55c2789724ec4099d74038ff31ce253a56f916bd`）。

| # | 確かめ | コマンド | 結果 |
| - | ------ | -------- | ---- |
| (1) | 3 ケースの入力とネイティブの参照の作り方 | `cargo test --manifest-path wasm/Cargo.toml --workspace --locked -- --list` と同じコマンドの `test result:` の行 | `tests/gen.rs` の 8 本と `test result:` の 5 行（下のコードブロック） |
| (2) | 9 行の比べ | `gh run view 42000912040 --job 127155573357 --log` の `make wasm-node-check` の段 | ジョブ `wasm32 build + size + node (macos-latest)` は `success`。`node:`・`wasm-bindgen:`・`row:` 14 行・`count:` 9 行・`summary:` の行（下のコードブロック） |
| (3) | 保存した出力からの数え直し | 同じログの `make wasm-node-check` の段 | `recount:` 9 行と `recount-summary:` の行（下のコードブロック） |
| (4) | 出力の sha256 | 同じログの `make wasm-node-check` の段 | `sha256:` 18 行（出力 9・ケースのファイル 9。下のコードブロック） |
| (5) | `wasm-macos` ジョブの環境の行 | 同じジョブの `Set up job`・`Install pinned Node and wasm-bindgen CLI`・`Show tool versions` の段 | 下のコードブロック |
| (6) | 回帰テスト | `gh run view 42000912040 --job 127155572755 --log` の `make test-wasm-node-check` の段 | ジョブ `make deny (cargo-deny)` は `success`。最終行は `passed: 337, failed: 0` |
| (7) | unsafe・外部の依存の追加 0 | `git diff --stat` と `git diff -U0` と `grep -rn unsafe`（下のコードブロック） | stat は空、`wasm/Cargo.lock` の差分は path の package 1 項、`grep` は 0 件 |

(1) の出力。

```console
$ cargo test --manifest-path wasm/Cargo.toml --workspace --locked -- --list 2>&1 | grep -E ': test$'
binary_reports_exit_codes: test
gen_refuses_existing_files: test
gen_rejects_bad_arguments: test
inputs_are_finite: test
manifest_hashes_match_files: test
native_reference_matches_default_cpu_ops: test
odd_case_matches_gate_row: test
square_cases_follow_fresh_generator: test
$ cargo test --manifest-path wasm/Cargo.toml --workspace --locked 2>&1 | grep '^test result:'
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.27s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

(2) の出力。ログの行は `ジョブ名<TAB>段の名前<TAB>時刻 本文` の形で、段の名前で絞って時刻までを落とす。

```console
$ gh run view 42000912040 --job 127155573357 --log | awk -F'\t' '$2=="make wasm-node-check"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | grep -E '^(node|wasm-bindgen|row|count|summary): '
node: v24.13.0
wasm-bindgen: 0.2.128
row: roundtrip-empty match
row: roundtrip-finite match
row: roundtrip-non-finite match
row: isa-scalar match
row: isa-simd128 match
row: gemm-64-scalar-gemm-scalar match
row: gemm-64-scalar-gemm-simd match
row: gemm-64-simd128-gemm-scalar match
row: gemm-256-scalar-gemm-scalar match
row: gemm-256-scalar-gemm-simd match
row: gemm-256-simd128-gemm-scalar match
row: gemm-37x41x53-scalar-gemm-scalar match
row: gemm-37x41x53-scalar-gemm-simd match
row: gemm-37x41x53-simd128-gemm-scalar match
count: gemm-64-scalar-gemm-scalar mismatches=0 n=4096
count: gemm-64-scalar-gemm-simd mismatches=0 n=4096
count: gemm-64-simd128-gemm-scalar mismatches=0 n=4096
count: gemm-256-scalar-gemm-scalar mismatches=0 n=65536
count: gemm-256-scalar-gemm-simd mismatches=0 n=65536
count: gemm-256-simd128-gemm-scalar mismatches=0 n=65536
count: gemm-37x41x53-scalar-gemm-scalar mismatches=0 n=1517
count: gemm-37x41x53-scalar-gemm-simd mismatches=0 n=1517
count: gemm-37x41x53-simd128-gemm-scalar mismatches=0 n=1517
summary: rows=14 match=14 mismatch=0 not-run=0
```

(3) の出力（(2) と同じ前段に `grep -E '^(recount|recount-summary): '` をつなぐ）。

```console
recount: gemm-64-scalar-gemm-scalar mismatches=0 n=4096
recount: gemm-64-scalar-gemm-simd mismatches=0 n=4096
recount: gemm-64-simd128-gemm-scalar mismatches=0 n=4096
recount: gemm-256-scalar-gemm-scalar mismatches=0 n=65536
recount: gemm-256-scalar-gemm-simd mismatches=0 n=65536
recount: gemm-256-simd128-gemm-scalar mismatches=0 n=65536
recount: gemm-37x41x53-scalar-gemm-scalar mismatches=0 n=1517
recount: gemm-37x41x53-scalar-gemm-simd mismatches=0 n=1517
recount: gemm-37x41x53-simd128-gemm-scalar mismatches=0 n=1517
recount-summary: rows=9 zero=9 nonzero=0 uncompared=0
```

(4) の出力（同じ前段に `grep -E '^sha256: '` をつなぐ）。

```console
sha256: gemm-64-scalar-gemm-scalar 25db4c0dde78a6a12cf5449e4c79542e7be1f742993c243a42a31519e8fa6f11
sha256: gemm-64-scalar-gemm-simd 25db4c0dde78a6a12cf5449e4c79542e7be1f742993c243a42a31519e8fa6f11
sha256: gemm-64-simd128-gemm-scalar 25db4c0dde78a6a12cf5449e4c79542e7be1f742993c243a42a31519e8fa6f11
sha256: gemm-256-scalar-gemm-scalar d7bc8e3c3e96f39fd560a67bc9446532ce233ee46b805c44d59504cdf39956c1
sha256: gemm-256-scalar-gemm-simd d7bc8e3c3e96f39fd560a67bc9446532ce233ee46b805c44d59504cdf39956c1
sha256: gemm-256-simd128-gemm-scalar d7bc8e3c3e96f39fd560a67bc9446532ce233ee46b805c44d59504cdf39956c1
sha256: gemm-37x41x53-scalar-gemm-scalar a0bb2a40eeebf8093a0236ffac4f9c053d7c8fca2267c1e08a3cdd40a5551d2b
sha256: gemm-37x41x53-scalar-gemm-simd a0bb2a40eeebf8093a0236ffac4f9c053d7c8fca2267c1e08a3cdd40a5551d2b
sha256: gemm-37x41x53-simd128-gemm-scalar a0bb2a40eeebf8093a0236ffac4f9c053d7c8fca2267c1e08a3cdd40a5551d2b
sha256: gemm-64.a 2259215901630df152c4b1e0df09edf21118c82c285b4a9968901fca8ca2ca52
sha256: gemm-64.b 9d19ef5ea664105fe1cfbdc69868867fcecde4652eb739d1912527d4652d3eeb
sha256: gemm-64.native-scalar 25db4c0dde78a6a12cf5449e4c79542e7be1f742993c243a42a31519e8fa6f11
sha256: gemm-256.a 0d509543d75aff9fa7372ca461e5c91dd8e9d8e41c81fe3b302e6610b1228b89
sha256: gemm-256.b e5aae545d81ea7cf7b62df1d03c79055a67967826d0b74ea02c0c68063fbd912
sha256: gemm-256.native-scalar d7bc8e3c3e96f39fd560a67bc9446532ce233ee46b805c44d59504cdf39956c1
sha256: gemm-37x41x53.a 8b6bdc24887e48cd2bc3598950e2949c599f27b9c14d2a2c11384d2bb9c454d1
sha256: gemm-37x41x53.b 60efc5337595142f4ec8d78f08e8e2663f4a67630672e1d21d7cc2f21f90e19e
sha256: gemm-37x41x53.native-scalar a0bb2a40eeebf8093a0236ffac4f9c053d7c8fca2267c1e08a3cdd40a5551d2b
```

(5) の出力。`Set up job` の段は `Operating System` と `Runner Image` の group の行、`Install pinned Node and wasm-bindgen CLI` の段は Node の tar の行、`Show tool versions` の段は末尾の 4 行。

```console
##[group]Operating System
macOS
26.6.2
25G83
##[endgroup]
##[group]Runner Image
Image: macos-26-arm64
Version: 20261026.0455.2
Included Software: https://github.com/actions/runner-images/blob/macos-26-arm64/20261026.0455/images/macos/macos-26-arm64-Readme.md
Image Release: https://github.com/actions/runner-images/releases/tag/macos-26-arm64%2F20261026.0455
##[endgroup]
node-v24.13.0-darwin-arm64.tar.gz: OK
node: v24.13.0
wasm-bindgen: wasm-bindgen 0.2.128
jq-1.8.2
Python 3.15.1
```

(6) の出力。

```console
$ gh run view 42000912040 --job 127155572755 --log | awk -F'\t' '$2=="make test-wasm-node-check"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | tail -n 1
passed: 337, failed: 0
```

(7) の出力。1/5 の base は `5cf1d33`。

```console
$ git diff --stat 5cf1d33..55c2789 -- wasm/deps-approved.toml deps-approved.toml Cargo.toml Cargo.lock crates/contract .github
$ git diff -U0 5cf1d33..55c2789 -- wasm/Cargo.lock
diff --git a/wasm/Cargo.lock b/wasm/Cargo.lock
index e8c2c4b..da2f894 100644
--- a/wasm/Cargo.lock
+++ b/wasm/Cargo.lock
@@ -127,0 +128,8 @@ dependencies = [
+
+[[package]]
+name = "wasm-parity-host"
+version = "0.0.0"
+dependencies = [
+ "fandhe-silicon-contract",
+ "fandhe-silicon-core",
+]
$ grep -rn unsafe wasm/crates/parity-host
```

#### 今の値の補い（base 55c2789）

4/5 の節の「確かめ方」に書いた `make test-wasm-node-check` の最終行は `passed: 328, failed: 0`。この時点の値は次のとおり（(6) の CI のログの最終行と同じ）。

```console
$ make test-wasm-node-check 2>&1 | tail -n 1
passed: 337, failed: 0
```

`scripts/wasm-node-check.sh` の `--output-dir` は、4/5 の「口」の表の条件のほかに、`<dir>` が空であることと `--record-dir` と別のディレクトリであることを確かめる。

```console
$ grep -n 'must be empty\|samefile' scripts/wasm-node-check.sh
163:            die("output dir must be empty")
172:        return os.path.samefile(a, b)
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/5 | GEMM の入力とネイティブの scalar の参照を書き出す host 側の道具 | #6271 |
| 2/5 | Node での照合を生成の入力の行と 2 つのビルドに広げる | #6272 |
| 3/5 | GEMM の 3 ケースの 9 行を照合の入力に足す | #6273 |
| 4/5 | 照合の出力の保存と保存先からの数え直し | #6274 |
| 5/5 | 完了の条件の確かめ | #6275 |

## 今の値の補い（base 5ea11ff）

記録時点の表と既存の節は書き換えない。以下は base `5ea11ff` で観測した今の値（実行環境は GPU なしの Linux。node・wasm-bindgen の CLI は入れていないので `make wasm-node-check` の実物の実行は流していない）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| `make test-wasm-node-check` の最終行（「今の値の補い（base 55c2789）」の `passed: 337, failed: 0`） | `passed: 337, failed: 0` → `passed: 342, failed: 0`。4/5 の節の `229` と 5/5 の節の `328` は記録時点の値のまま | `make test-wasm-node-check` |
| 5/5 の節の `make wasm-node-check` の CI の run の後の main の run | run `42004939587`（`b7efe77`）は attempt 2 が `success`。ログの行は `summary: rows=14 match=14 mismatch=0 not-run=0`・`recount-summary: rows=9 zero=9 nonzero=0 uncompared=0` | `gh run view 42004939587 -R Fandhe-AI/fandhe-silicon --json attempt,conclusion` |

## 部分 Top-k の比べ（TASK-996）

### 記録の所在

記録の正は private spec 側（`docs/spec/05-tasks.md` の TASK-996。ポインタのみ）。本文は転記しない
（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

### Node での照合の部分 Top-k の種類（1/4）

`scripts/wasm-node-check.sh` の python の中に、ケースの種類ごとの性質を持つ表 `KINDS` を置く。`load_manifest`・`load_results`・判定・`case_sha_lines`・`recount_form_inner`・`form_items` はこの表を引く。`wasm/node/run-glue.cjs` の許可表に `topk_scalar`・`topk_simd` を足し、`runTopk` が呼ぶ。

#### 種類の表

| 種類 | manifest の鍵 | ファイル | 比べの単位 | `sha256:` の接尾辞 | `export` |
| ---- | ------------- | -------- | ---------- | ------------------ | -------- |
| `gemm` | `name`・`kind`・`m`・`k`・`n`・`a_file`・`b_file`・`native_scalar_file`・`a_sha256`・`b_sha256`・`native_scalar_sha256` | `a_file`（m×k×4 バイト）・`b_file`（k×n×4）・`native_scalar_file`（m×n×4） | 4 バイト | `.a`・`.b`・`.native-scalar` | `gemm_scalar`・`gemm_simd` |
| `topk` | `name`・`kind`・`n`・`k`・`scores_file`・`native_scalar_file`・`scores_sha256`・`native_scalar_sha256` | `scores_file`（n×4 バイト）・`native_scalar_file`（k×8） | 8 バイト | `.scores`・`.native-scalar` | `topk_scalar`・`topk_simd` |

鍵は種類ごとに完全一致で、`n`・`k` は正の整数（真偽値は不可）。`topk` の 1 件はスロット番号の u32 とスコアの f32 をこの順にリトルエンディアンで並べた 8 バイト。`kind` が欠ければ鍵の過不足、文字列でない値と表に無い値は `manifest case kind is unknown`（どちらも終了コード 2）。ファイル名・sha256・通常のファイルの照らしは種類によらず同じ。

#### `run-glue.cjs` の呼び方

| 引数・戻り | 内容 |
| ---------- | ---- |
| `scores` | `scores_file` を新しい `ArrayBuffer` に写した `Float32Array` |
| `k` | manifest の `k` |
| `out` | 長さ k×8 の `Uint8Array`。同じ `ArrayBuffer` の `Uint32Array` で全語を `0x7fc0dead` に埋めて渡す |
| 戻り | `>>> 0` で語にする。0 のときだけ `out` のバイト列を `<outputs>/<行の id>.bin` に書き `{id, output_file}`、0 以外は `{id, error: "nonzero return"}`、例外は `call failed` |

呼ぶ関数は種類 → 関数の `Map` で引く（`gemm` は `runGemm`、`topk` は `runTopk`）。

#### 判定と出力の行

| 場面 | 状態・行 |
| ---- | -------- |
| 1 件のスロット番号かスコアのビット列のどちらかが違う | その 1 件が不一致。`mismatch (mismatches <件数>/<k>)` |
| 件数が違う | `mismatch (length expected <k> got <m>)`。`count:`・`sha256:` は出さない |
| 全件が同じ | `match`。`count: <id> mismatches=0 n=<k>` |
| 出力の長さが単位の倍数でない | 終了コード 2（`not a multiple of 8`）。`--recount` も同じ |
| `native-neon` の `topk` の行 | `not-run (reference-unavailable)`。`count:`・`nonfinite:` を出さない |

`nonfinite: <ケース> input=<件数> output=<件数>` は、`native-scalar` の生成の行が指す `topk` のケースごとに manifest の順で出す。`input` は `scores_file` の 4 バイトの語のうち `w & 0x7f800000 == 0x7f800000` の語の数、`output` は `native_scalar_file` の各件のスコア（件の先頭から 4 バイト目の語）のうち同じ条件の語の数。出す位置は、判定が `sha256: <ケース>…` の行の後で `recount:` の行の前、`--recount` が `sha256:` の行の後で `recount-summary:` の行の前。判定と終了コードには関わらない。実行の中の数え直しは、この行も判定の側と突き合わせる（違えば `recount disagrees` で終了コード 2）。

#### export とケースの種類の照らし

行の `export` の種類（`gemm_*` は `gemm`、`topk_*` は `topk`）と、行の `case` の manifest の種類が違えば終了コード 2（Node の本体は起動しない）。`--recount` の `outputs.json` の行にも同じ照らしを当てる。

#### 回帰テスト

`scripts/test-wasm-node-check.sh` の足した項目と直した項目。

| 項目 | 確かめること |
| ---- | ------------ |
| 22 | `topk` の行を足した合成の入力が、回帰テストを直さずに全行 `match` |
| 23c | `run-glue.cjs` を本物の node で偽の glue に当て、`scores` のビット列・`k`・`out` の長さと埋め・戻り 0 のときだけ書くこと・書かなかった語が番兵のまま残ること |
| 44 | `kind` の欠け・未知の値・文字列でない値 |
| 64 | 数え直しの比べの式の変異体（単位を引く形の字句に追随） |
| 66・67 | 全部一致の行と並び、スロット番号だけ・スコアだけ・両方・`-0.0` と `+0.0`・最後の件だけの不一致 |
| 68・69 | 件数の違い・12 バイトの出力・呼び出しの失敗、`native-neon` の `topk` の行 |
| 70 | 参照のスコアが非有限のとき `output` の件数（スコアの語で数える） |
| 71・72 | export とケースの種類の照らし、`topk` の manifest の不正 |
| 73・74 | `gemm` と `topk` が混ざった保存の形、manifest に `topk` があり行が `gemm` だけの実行 |
| 75・76 | `topk` の保存の形の `--recount`（0・1・2）、`nonfinite:` の数え直しの食い違いの変異体 |

#### 確かめ方

```console
$ make test-wasm-node-check 2>&1 | tail -n 1
passed: 416, failed: 0
$ grep -n "^KINDS\|def nonfinite_lines\|def kind_of_export" scripts/wasm-node-check.sh
$ grep -n "RUNNERS\|runTopk" wasm/node/run-glue.cjs
```

### 部分 Top-k の入力とネイティブの参照（2/4）

`wasm-parity-host gen <dir>` が、GEMM の 3 ケースに続けて部分 Top-k の 3 ケースの入力と `DefaultCpuOps.topk_partial_scalar` の出力を書く。`wasm/node/cases.json` は変わらず、行は 14 行のまま。

#### ケース

定数 `TOPK_CASES`（`wasm/crates/parity-host/src/lib.rs`）が持つ。ケースごとに新しい `SplitMix64::new(GEMM_INPUT_SEED)` から作る。

| 名前 | 件数 | k | 入力の作り方 |
| ---- | ---- | -- | ------------ |
| `topk-batch` | 20,000 | 64 | `next_signed_unit_f32` を 20,000 回 |
| `topk-ties` | 200 | 10 | 添字 `i` が `i % 10 == 3` なら `0.0`（生成器を進めない）、それ以外は `((next_u64() >> 61) as f32) + 1.0` |
| `topk-nonfinite` | 200 | 10 | `next_signed_unit_f32` を 200 回の後、添字 5・17・30・77 を `f32::NAN`・`f32::from_bits(0xffc0_0000)`・`f32::INFINITY`・`f32::NEG_INFINITY` に置く |

ネイティブの参照は、同じスコア列を `DefaultCpuOps.topk_partial_scalar(&scores, k, &mut out)` に渡した `out`（長さ k、初期値 `(0, 0.0)`）。

#### 書くファイル

| ファイル | 中身 |
| -------- | ---- |
| `<ケース>.scores.bin` | スコアの f32 を添字の順にリトルエンディアンで並べたもの（NaN のビット列を保つ） |
| `<ケース>.native-scalar.bin` | `out` の各件を、`slot.to_le_bytes()` の 4 バイト、`score.to_bits().to_le_bytes()` の 4 バイトの順に並べたもの（1 件 8 バイト） |
| `manifest.json` の `topk` の要素 | `name`・`kind`（`"topk"`）・`n`（スコアの件数）・`k`・`scores_file`・`native_scalar_file`・`scores_sha256`・`native_scalar_sha256` を 1 欄 1 行で持つ |

今の値として、`manifest.json` の `cases` は GEMM の 3 ケースの後に部分 Top-k の 3 ケースを並べた 6 要素で、書く名前は 16 個（`.bin` が 15 個と `manifest.json`）。`topk_partial_scalar` が `Err` を返したときは固定の文言 `error: native topk_partial_scalar failed` を標準エラーに出し、終了コードは 2。

#### 試験

`wasm/crates/parity-host/tests/gen.rs`。期待は生成器・`DefaultCpuOps`・`reference_topk_f32`・`compare_topk` の呼び出しで作る。

| 試験 | 確かめること |
| ---- | ------------ |
| `topk_batch_follows_fresh_generator` | 列が新しい生成器の `next_signed_unit_f32` の列とビット列で同じ。件数 20,000。全部有限 |
| `topk_ties_input_shape` | 件数 200。全部有限。`0x0000_0000` が 20 個、`0x8000_0000` が 0 個。`reference_topk_f32(&v, 11)` の 10 件目と 11 件目のスコアのビット列が同じ。生成器から作り直した列とビット列で同じ |
| `topk_nonfinite_input_shape` | 件数 200。NaN 2 つ（`f32::NAN.to_bits()` と `0xffc0_0000`）・`+Inf`・`-Inf` が添字 5・17・30・77。ほかは有限で、新しい生成器の列とビット列で同じ |
| `native_topk_matches_default_cpu_ops` | 書いた `.native-scalar.bin` が `DefaultCpuOps.topk_partial_scalar` の出力を 8 バイトずつ並べたものと同じ（3 ケース） |
| `native_topk_matches_cpu_reference` | 読み戻した列が `compare_topk(&reference_topk_f32(&scores, k), …)` で `is_match`、`compared` が k（3 ケース） |
| `native_topk_has_no_non_finite_slots` | 3 ケースの `.native-scalar.bin` のスコアが全部有限。`topk-nonfinite` のスロット番号に 5・17・30・77 が無い |
| `topk_record_layout` | 手書きの 2 件で、1 件の 8 バイトがスロット番号・スコアのビット列の順のリトルエンディアン |
| `record_compare_agrees_with_compare_topk` | スロット番号の最下位ビット、スコアの最下位ビットを別々に反転すると、8 バイトごとの違いが 1 件で `compare_topk` の `mismatched` も 1（3 ケース） |
| `topk_manifest_entries_match_files` | `topk` の 3 要素の鍵がちょうど 8 個。`kind`・`n`・`k`・sha256 が合う。ファイルの長さが `n`×4・`k`×8。ファイル名が `[a-z0-9.-]` だけ。`.scores.bin` が `f32_le_bytes` と同じ |
| `gen_refuses_existing_topk_file` | `topk-ties.scores.bin` が先にあると `FileExists`。ディレクトリの中身はそのファイルだけで、中身も変わらない |
| `native_topk_error_text` | `GenError::NativeTopkFailed` の文言が固定 |

既存の `manifest_hashes_match_files` の名前の数の期待は、GEMM と部分 Top-k のケースの数の和。

#### 確かめ方

時点のコミット: `cbc8395`（この変更の土台）。

```console
$ cargo test --manifest-path wasm/Cargo.toml --workspace --locked 2>&1 | grep '^test result:'
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.36s
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
topk-batch.native-scalar.bin
topk-batch.scores.bin
topk-nonfinite.native-scalar.bin
topk-nonfinite.scores.bin
topk-ties.native-scalar.bin
topk-ties.scores.bin
0d509543d75aff9fa7372ca461e5c91dd8e9d8e41c81fe3b302e6610b1228b89  gemm-256.a.bin
e5aae545d81ea7cf7b62df1d03c79055a67967826d0b74ea02c0c68063fbd912  gemm-256.b.bin
d7bc8e3c3e96f39fd560a67bc9446532ce233ee46b805c44d59504cdf39956c1  gemm-256.native-scalar.bin
8b6bdc24887e48cd2bc3598950e2949c599f27b9c14d2a2c11384d2bb9c454d1  gemm-37x41x53.a.bin
60efc5337595142f4ec8d78f08e8e2663f4a67630672e1d21d7cc2f21f90e19e  gemm-37x41x53.b.bin
a0bb2a40eeebf8093a0236ffac4f9c053d7c8fca2267c1e08a3cdd40a5551d2b  gemm-37x41x53.native-scalar.bin
2259215901630df152c4b1e0df09edf21118c82c285b4a9968901fca8ca2ca52  gemm-64.a.bin
9d19ef5ea664105fe1cfbdc69868867fcecde4652eb739d1912527d4652d3eeb  gemm-64.b.bin
25db4c0dde78a6a12cf5449e4c79542e7be1f742993c243a42a31519e8fa6f11  gemm-64.native-scalar.bin
92c07bf7987315eac089e4d7c0d094715f4a7d890e08fca1d0027ea5619e0e14  manifest.json
562e641919b9d368e848150bd5adff9c5f78022dfae32d7bbfa88b3bc63ae91f  topk-batch.native-scalar.bin
e583c60181f74537ccb440169920ca11583822f4e65653b8dd34ea420ebd21be  topk-batch.scores.bin
98fa0af29bc71ec2b760aab079d2091327c9294f52258debc07524215c0062d2  topk-nonfinite.native-scalar.bin
a8c42cd6034f57cb241fa7a4b9023dff42547a234f64d2a9aa152ce2084bcb45  topk-nonfinite.scores.bin
0c0b103cd5f6513b413e98ddba0f27fee9bd4658d3289523035841c98a59aa5c  topk-ties.native-scalar.bin
f951986cfa3ee3f33f94b487d0a65b36cd546269a83b47ac250518d55cbde166  topk-ties.scores.bin
```

定数と関数の所在は `grep -n "TOPK_CASES\|pub fn" wasm/crates/parity-host/src/lib.rs` で確かめる。

### 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | Node での照合に部分 Top-k の種類を足す | #6281 |
| 2/4 | 部分 Top-k の 3 ケースの入力とネイティブの参照を host 側の道具に足す | #6282 |
