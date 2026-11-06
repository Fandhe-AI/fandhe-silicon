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

### 部分 Top-k の 10 行（3/4）

`wasm/node/cases.json` の既定の入力に足した生成の行。並びは GEMM の行の後に、ケースの順・組の順。

| id | ビルド | export | ケース | 参照の出どころ |
| -- | ------ | ------ | ------ | -------------- |
| `topk-batch-scalar-topk-scalar` | `scalar` | `topk_scalar` | `topk-batch` | `native-scalar` |
| `topk-batch-scalar-topk-simd` | `scalar` | `topk_simd` | `topk-batch` | `native-scalar` |
| `topk-batch-simd128-topk-scalar` | `simd128` | `topk_scalar` | `topk-batch` | `native-scalar` |
| `topk-batch-simd128-topk-simd` | `simd128` | `topk_simd` | `topk-batch` | `native-scalar` |
| `topk-ties-scalar-topk-scalar` | `scalar` | `topk_scalar` | `topk-ties` | `native-scalar` |
| `topk-ties-scalar-topk-simd` | `scalar` | `topk_simd` | `topk-ties` | `native-scalar` |
| `topk-ties-simd128-topk-scalar` | `simd128` | `topk_scalar` | `topk-ties` | `native-scalar` |
| `topk-ties-simd128-topk-simd` | `simd128` | `topk_simd` | `topk-ties` | `native-scalar` |
| `topk-nonfinite-scalar-topk-scalar` | `scalar` | `topk_scalar` | `topk-nonfinite` | `native-scalar` |
| `topk-nonfinite-simd128-topk-simd` | `simd128` | `topk_simd` | `topk-nonfinite` | `native-scalar` |

`wasm/node/cases-reference-unavailable.json` の 3 行。`make test-wasm-node-check` が `--cases` で読む。

| id | 参照の出どころ | 回帰テストで出る状態 |
| -- | -------------- | -------------------- |
| `topk-batch-native-neon` | `native-neon` | `not-run (reference-unavailable)` |
| `topk-ties-native-neon` | `native-neon` | `not-run (reference-unavailable)` |
| `topk-nonfinite-native-neon` | `native-neon` | `not-run (reference-unavailable)` |

#### 確かめ方

```console
$ grep -c '"topk_' wasm/node/cases.json
10
$ grep -c '"reference_source": "native-neon"' wasm/node/cases-reference-unavailable.json
3
$ make test-wasm-node-check 2>&1 | tail -n 1
passed: 420, failed: 0
```

### 完了の条件の確かめ（4/4）

本リポで観測できる事実を、コマンドと出力だけで記録する（TASK-996）。

時点のコミット: `a7a6960`（この変更の土台）。(2)〜(8) は、このコミットの main への push で流れた CI の run `42024246697` から取る（event `push`・conclusion `success`・headSha は `a7a696001a662cbd2c920e529cc3f29e85a28dab`）。

| # | 確かめ | コマンド | 結果 |
| - | ------ | -------- | ---- |
| (1) | 3 ケースの入力とネイティブの参照の作り方 | `cargo test --manifest-path wasm/Cargo.toml --workspace --locked -- --list` と同じコマンドの `test result:` の行 | 試験の名前 11 行と `test result:` の 5 行（下のコードブロック） |
| (2) | 10 行の比べ | `gh run view 42024246697 --job 127234347057 --log` の `make wasm-node-check` の段 | ジョブ `wasm32 build + size + node (macos-latest)` は `success`。`node:`・`wasm-bindgen:`・Top-k の `row:` 10 行・`count:` 10 行・`summary:` の行（下のコードブロック） |
| (3) | 非有限の件数 | 同じログの `make wasm-node-check` の段 | `nonfinite:` 3 行（下のコードブロック） |
| (4) | 保存した出力からの数え直し | 同じログの `make wasm-node-check` の段 | `recount:` 10 行と `recount-summary:` の行（下のコードブロック） |
| (5) | 出力の sha256 | 同じログの `make wasm-node-check` の段 | `sha256:` 16 行（出力 10・ケースのファイル 6。下のコードブロック） |
| (6) | `wasm-macos` ジョブの環境の行 | 同じジョブの `Set up job`・`Install pinned Node and wasm-bindgen CLI`・`Show tool versions` の段 | 下のコードブロック |
| (7) | 回帰テスト | `gh run view 42024246697 --job 127234346903 --log` の `make test-wasm-node-check` の段 | ジョブ `make deny (cargo-deny)` は `success`。`ok:` 4 行と最終行は `passed: 420, failed: 0` |
| (8) | `tests/cpu_ops_topk_reference.rs` の 2 本の試験 | `gh run view 42024246697 --job 127234347235 --log` の `make verify` の段 | ジョブ `make verify + build-profiles (macos-latest)` は `success`。2 本は `ok`、`test result:` は 1 行（下のコードブロック） |
| (9) | unsafe・外部の依存の追加 0 | `git diff --stat` と `grep -rn unsafe`（下のコードブロック） | stat は空、`grep` は 0 件 |

(1) の出力。

```console
$ cargo test --manifest-path wasm/Cargo.toml --workspace --locked -- --list 2>&1 | grep -E '^(topk_batch_follows_fresh_generator|topk_ties_input_shape|topk_nonfinite_input_shape|native_topk_matches_default_cpu_ops|native_topk_matches_cpu_reference|native_topk_has_no_non_finite_slots|topk_record_layout|record_compare_agrees_with_compare_topk|topk_manifest_entries_match_files|gen_refuses_existing_topk_file|native_topk_error_text): test$'
gen_refuses_existing_topk_file: test
native_topk_error_text: test
native_topk_has_no_non_finite_slots: test
native_topk_matches_cpu_reference: test
native_topk_matches_default_cpu_ops: test
record_compare_agrees_with_compare_topk: test
topk_batch_follows_fresh_generator: test
topk_manifest_entries_match_files: test
topk_nonfinite_input_shape: test
topk_record_layout: test
topk_ties_input_shape: test
$ cargo test --manifest-path wasm/Cargo.toml --workspace --locked 2>&1 | grep '^test result:'
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.33s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

(2) の出力。ログの行は `ジョブ名<TAB>段の名前<TAB>時刻 本文` の形で、段の名前で絞って時刻までを落とす。

```console
$ gh run view 42024246697 --job 127234347057 --log | awk -F'\t' '$2=="make wasm-node-check"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | grep -E '^(node|wasm-bindgen|summary): |^(row|count): topk-'
node: v24.13.0
wasm-bindgen: 0.2.128
row: topk-batch-scalar-topk-scalar match
row: topk-batch-scalar-topk-simd match
row: topk-batch-simd128-topk-scalar match
row: topk-batch-simd128-topk-simd match
row: topk-ties-scalar-topk-scalar match
row: topk-ties-scalar-topk-simd match
row: topk-ties-simd128-topk-scalar match
row: topk-ties-simd128-topk-simd match
row: topk-nonfinite-scalar-topk-scalar match
row: topk-nonfinite-simd128-topk-simd match
count: topk-batch-scalar-topk-scalar mismatches=0 n=64
count: topk-batch-scalar-topk-simd mismatches=0 n=64
count: topk-batch-simd128-topk-scalar mismatches=0 n=64
count: topk-batch-simd128-topk-simd mismatches=0 n=64
count: topk-ties-scalar-topk-scalar mismatches=0 n=10
count: topk-ties-scalar-topk-simd mismatches=0 n=10
count: topk-ties-simd128-topk-scalar mismatches=0 n=10
count: topk-ties-simd128-topk-simd mismatches=0 n=10
count: topk-nonfinite-scalar-topk-scalar mismatches=0 n=10
count: topk-nonfinite-simd128-topk-simd mismatches=0 n=10
summary: rows=24 match=24 mismatch=0 not-run=0
```

(3) の出力（(2) と同じ前段に `grep -E '^nonfinite: '` をつなぐ）。

```console
nonfinite: topk-batch input=0 output=0
nonfinite: topk-ties input=0 output=0
nonfinite: topk-nonfinite input=4 output=0
```

(4) の出力（同じ前段に `grep -E '^recount: topk-|^recount-summary: '` をつなぐ）。

```console
recount: topk-batch-scalar-topk-scalar mismatches=0 n=64
recount: topk-batch-scalar-topk-simd mismatches=0 n=64
recount: topk-batch-simd128-topk-scalar mismatches=0 n=64
recount: topk-batch-simd128-topk-simd mismatches=0 n=64
recount: topk-ties-scalar-topk-scalar mismatches=0 n=10
recount: topk-ties-scalar-topk-simd mismatches=0 n=10
recount: topk-ties-simd128-topk-scalar mismatches=0 n=10
recount: topk-ties-simd128-topk-simd mismatches=0 n=10
recount: topk-nonfinite-scalar-topk-scalar mismatches=0 n=10
recount: topk-nonfinite-simd128-topk-simd mismatches=0 n=10
recount-summary: rows=19 zero=19 nonzero=0 uncompared=0
```

(5) の出力（同じ前段に `grep -E '^sha256: topk-'` をつなぐ）。

```console
sha256: topk-batch-scalar-topk-scalar 562e641919b9d368e848150bd5adff9c5f78022dfae32d7bbfa88b3bc63ae91f
sha256: topk-batch-scalar-topk-simd 562e641919b9d368e848150bd5adff9c5f78022dfae32d7bbfa88b3bc63ae91f
sha256: topk-batch-simd128-topk-scalar 562e641919b9d368e848150bd5adff9c5f78022dfae32d7bbfa88b3bc63ae91f
sha256: topk-batch-simd128-topk-simd 562e641919b9d368e848150bd5adff9c5f78022dfae32d7bbfa88b3bc63ae91f
sha256: topk-ties-scalar-topk-scalar 0c0b103cd5f6513b413e98ddba0f27fee9bd4658d3289523035841c98a59aa5c
sha256: topk-ties-scalar-topk-simd 0c0b103cd5f6513b413e98ddba0f27fee9bd4658d3289523035841c98a59aa5c
sha256: topk-ties-simd128-topk-scalar 0c0b103cd5f6513b413e98ddba0f27fee9bd4658d3289523035841c98a59aa5c
sha256: topk-ties-simd128-topk-simd 0c0b103cd5f6513b413e98ddba0f27fee9bd4658d3289523035841c98a59aa5c
sha256: topk-nonfinite-scalar-topk-scalar 98fa0af29bc71ec2b760aab079d2091327c9294f52258debc07524215c0062d2
sha256: topk-nonfinite-simd128-topk-simd 98fa0af29bc71ec2b760aab079d2091327c9294f52258debc07524215c0062d2
sha256: topk-batch.scores e583c60181f74537ccb440169920ca11583822f4e65653b8dd34ea420ebd21be
sha256: topk-batch.native-scalar 562e641919b9d368e848150bd5adff9c5f78022dfae32d7bbfa88b3bc63ae91f
sha256: topk-ties.scores f951986cfa3ee3f33f94b487d0a65b36cd546269a83b47ac250518d55cbde166
sha256: topk-ties.native-scalar 0c0b103cd5f6513b413e98ddba0f27fee9bd4658d3289523035841c98a59aa5c
sha256: topk-nonfinite.scores a8c42cd6034f57cb241fa7a4b9023dff42547a234f64d2a9aa152ce2084bcb45
sha256: topk-nonfinite.native-scalar 98fa0af29bc71ec2b760aab079d2091327c9294f52258debc07524215c0062d2
```

(6) の出力。`Set up job` の段は `Operating System` と `Runner Image` の group の行、`Install pinned Node and wasm-bindgen CLI` の段は Node の tar の行、`Show tool versions` の段は末尾の 4 行。

```console
##[group]Operating System
macOS
26.7.2
25G332
##[endgroup]
##[group]Runner Image
Image: macos-26-arm64
Version: 20261102.0473.1
Included Software: https://github.com/actions/runner-images/blob/macos-26-arm64/20261102.0473/images/macos/macos-26-arm64-Readme.md
Image Release: https://github.com/actions/runner-images/releases/tag/macos-26-arm64%2F20261102.0473
##[endgroup]
node-v24.13.0-darwin-arm64.tar.gz: OK
node: v24.13.0
wasm-bindgen: wasm-bindgen 0.2.128
jq-1.8.2
Python 3.15.2
```

(7) の出力。

```console
$ gh run view 42024246697 --job 127234346903 --log | awk -F'\t' '$2=="make test-wasm-node-check"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | grep -E '^ok: (22 bundled topk rows are exactly|22 every bundled row has a reference source|77 reference-unavailable rows|77 the file has three rows)'
ok: 22 bundled topk rows are exactly the ten (case, export, build) pairs
ok: 22 every bundled row has a reference source of fixed or native-scalar
ok: 77 reference-unavailable rows
ok: 77 the file has three rows, all native-neon
$ gh run view 42024246697 --job 127234346903 --log | awk -F'\t' '$2=="make test-wasm-node-check"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | tail -n 1
passed: 420, failed: 0
```

(8) の出力（`make verify` の段のうち `Running tests/cpu_ops_topk_reference.rs` から最初の `test result:` まで）。

```console
$ gh run view 42024246697 --job 127234347235 --log | awk -F'\t' '$2=="make verify"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | awk '/Running tests\/cpu_ops_topk_reference\.rs/{f=1} f&&/^test (topk_simd_matches_scalar_bytes|topk_simd_scalar_byte_sequences_match) \.\.\. |^test result:/{print} f&&/^test result:/{exit}'
test topk_simd_scalar_byte_sequences_match ... ok
test topk_simd_matches_scalar_bytes ... ok
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
```

(9) の出力。1/4 の base は `9d458e1`。

```console
$ git diff --stat 9d458e1..a7a6960 -- wasm/deps-approved.toml deps-approved.toml Cargo.toml Cargo.lock wasm/Cargo.toml wasm/Cargo.lock crates .github
$ grep -rn unsafe wasm/crates/parity-host
```

### 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | Node での照合に部分 Top-k の種類を足す | #6281 |
| 2/4 | 部分 Top-k の 3 ケースの入力とネイティブの参照を host 側の道具に足す | #6282 |
| 3/4 | 部分 Top-k の 10 行を照合の入力に足す | #6283 |
| 4/4 | 完了の条件の確かめ | #6284 |

## 要素ごと演算の比べ（TASK-997）

### 記録の所在

記録の正は private spec 側（`docs/spec/05-tasks.md` の TASK-997。ポインタのみ）。本文は転記しない
（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

### Node での照合の要素ごと演算の種類（1/4）

`scripts/wasm-node-check.sh` の種類の表 `KINDS` に `elementwise` の項を足す。表の全項に、種類に固有の欄の照らし `fields` と、`input-special:` の数える先 `special` の 2 欄を持たせる（`gemm`・`topk` は `()` と `None`）。`wasm/node/run-glue.cjs` の許可表に `elementwise_scalar`・`elementwise_simd` を足し、`runElementwise` が呼ぶ。

#### 種類の表

| 種類 | manifest の鍵 | ファイル | 比べの単位 | `sha256:` の接尾辞 | `export` |
| ---- | ------------- | -------- | ---------- | ------------------ | -------- |
| `elementwise` | `name`・`kind`・`n`・`op_code`・`c_bits`・`input_file`・`native_scalar_file`・`input_sha256`・`native_scalar_sha256` | `input_file`（n×4 バイト）・`native_scalar_file`（n×4） | 4 バイト | `.input`・`.native-scalar` | `elementwise_scalar`・`elementwise_simd` |

鍵は完全一致で、`n` は正の整数（真偽値は不可）。`op_code` は整数の 0 か 1（真偽値・文字列・2 以上・負は不可）、`c_bits` は 8 桁の小文字の 16 進の文字列（f32 のビット列）。違えば `manifest case field is invalid`（終了コード 2・Node の本体は起動しない）。ファイル名・sha256・通常のファイルの照らしは種類によらず同じ。

#### `run-glue.cjs` の呼び方

| 引数・戻り | 内容 |
| ---------- | ---- |
| `op_code` | manifest の `op_code` |
| `c` | `c_bits` を f32 にした値 |
| `a` | `input_file` を新しい `ArrayBuffer` に写した `Float32Array` |
| `out` | 長さ n の `Float32Array`。同じ `ArrayBuffer` の `Uint32Array` で全語を `0x7fc0dead` に埋めて渡す |
| 戻り | `>>> 0` で語にする。0 のときだけ `out` のバイト列を `<outputs>/<行の id>.bin` に書き `{id, output_file}`、0 以外は `{id, error: "nonzero return"}`、例外は `call failed` |

呼ぶ関数は種類 → 関数の `Map` で引く（`gemm` は `runGemm`、`topk` は `runTopk`、`elementwise` は `runElementwise`）。

#### 判定と出力の行

| 場面 | 状態・行 |
| ---- | -------- |
| 1 語でも 32 ビットが違う（`+0.0` と `-0.0`、NaN のビット列の違いを含む） | その語が不一致。`mismatch (mismatches <件数>/<n>)` |
| 語の数が違う | `mismatch (length expected <n> got <m>)`。`count:`・`sha256:` は出さない |
| 全語が同じ | `match`。`count: <id> mismatches=0 n=<n>` |
| 出力の長さが 4 の倍数でない | 終了コード 2（`not a multiple of 4`）。`--recount` も同じ |
| `native-neon` の `elementwise` の行 | `not-run (reference-unavailable)`。`count:`・`input-special:` を出さない |

`input-special: <ケース> nan=<件数> inf=<件数> zero=<件数>` は、`native-scalar` の生成の行が指すケースのうち表の `special` を持つもの（`elementwise`）ごとに manifest の順で出す。`input_file` の 4 バイトの語 `w` を `e = w & 0x7fffffff` とし、`e == 0` は `zero`（`+0` と `-0`）、`e == 0x7f800000` は `inf`（`+Inf` と `-Inf`）、`e > 0x7f800000` は `nan`。数えるのはファイルのバイト列で、manifest の申告は使わない。出す位置は、判定が `sha256:`・`nonfinite:` の行の後で `recount:` の行の前、`--recount` が `sha256:`・`nonfinite:` の行の後で `recount-summary:` の行の前。判定と終了コードには関わらない。実行の中の数え直しは、この行も判定の側と突き合わせる（違えば `recount disagrees` で終了コード 2）。

#### export とケースの種類の照らし

行の `export` の種類（`gemm_*` は `gemm`、`topk_*` は `topk`、`elementwise_*` は `elementwise`）と、行の `case` の manifest の種類が違えば終了コード 2（Node の本体は起動しない）。`--recount` の `outputs.json` の行にも同じ照らしを当てる。

#### 回帰テスト

`scripts/test-wasm-node-check.sh` の足した項目と直した項目。

| 項目 | 確かめること |
| ---- | ------------ |
| 22 | `elementwise` の行を足した合成の入力が、回帰テストを直さずに全行 `match` |
| 23d | `run-glue.cjs` を本物の node で偽の glue に当て、`op_code`・`c`・`a` のビット列と長さ・`out` の長さと埋め・戻り 0 のときだけ書くこと・書かなかった語が番兵のまま残ること |
| 78 | 全部一致の行と並び（`count:`・`sha256:`・`input-special:`・`summary:`） |
| 79・80 | 最初の語・最後の語・`-0.0` と `+0.0`・NaN のビット列だけの不一致、語数の違い・21 バイトの出力・呼び出しの失敗 |
| 81・82 | `native-neon` の `elementwise` の行、入力に NaN・Inf・±0 がある合成のケースの件数 |
| 83・84 | export とケースの種類の照らし、`elementwise` の manifest の不正（鍵・`n`・`op_code`・`c_bits`・長さ・sha256・ファイル名） |
| 85・86 | 3 種類が混ざった保存の形、manifest に `elementwise` があり行が `gemm` だけの実行 |
| 87・88 | `elementwise` の保存の形の `--recount`（0・1・2）、`input-special:` の数え直しの食い違いの変異体 |

#### 確かめ方

```console
$ make test-wasm-node-check 2>&1 | tail -n 1
passed: 504, failed: 0
$ grep -n "^KINDS\|def input_special_lines\|def kind_of_export" scripts/wasm-node-check.sh
$ grep -n "RUNNERS\|runElementwise" wasm/node/run-glue.cjs
```

### 要素ごと演算の入力とネイティブの参照（2/4）

`wasm-parity-host gen <dir>` が、GEMM の 3 ケース・部分 Top-k の 3 ケースに続けて要素ごと演算の 2 ケースの入力と `DefaultCpuOps.elementwise_scalar` の出力を書く。`wasm/node/cases.json` は変わらず、行は 24 行のまま。

#### ケース

定数 `ELEMENTWISE_CASES`（`wasm/crates/parity-host/src/lib.rs`）が持つ。件数は定数 `ELEMENTWISE_LEN`。ケースごとに新しい `SplitMix64::new(GEMM_INPUT_SEED)` から作る。

| 名前 | 件数 | `op_code` | `c_bits` | 入力の作り方 |
| ---- | ---- | --------- | -------- | ------------ |
| `elementwise-relu` | 1,000,003 | 0 | `00000000` | `next_signed_unit_f32` を順に取り、`0.0` と等しい値は入れずに引き直して 1,000,003 個 |
| `elementwise-add-scalar` | 1,000,003 | 1 | `3f000000` | 同じ（`elementwise-relu` と同じ列） |

ネイティブの参照は、同じ入力を `DefaultCpuOps.elementwise_scalar(op, &input, &mut out)`（`out` は長さ 1,000,003 で初期値 `0.0`）に渡した `out`。`op` は `elementwise-relu` が `UnaryElementwiseOp::Relu`、`elementwise-add-scalar` が `UnaryElementwiseOp::AddScalar(0.5)`。

#### 書くファイル

| ファイル | 中身 |
| -------- | ---- |
| `<ケース>.input.bin` | 入力の f32 を添字の順にリトルエンディアンで並べたもの。2 ケースで同じ中身 |
| `<ケース>.native-scalar.bin` | `out` の f32 を添字の順にリトルエンディアンで並べたもの |
| `manifest.json` の `elementwise` の要素 | `name`・`kind`（`"elementwise"`）・`n`・`op_code`（整数）・`c_bits`（小文字 8 桁の 16 進の文字列）・`input_file`・`native_scalar_file`・`input_sha256`・`native_scalar_sha256` を 1 欄 1 行で持つ |

今の値として、`manifest.json` の `cases` は GEMM の 3 ケース・部分 Top-k の 3 ケースの後に要素ごと演算の 2 ケースを並べた 8 要素で、書く名前は 20 個（`.bin` が 19 個と `manifest.json`）。`elementwise_scalar` が `Err` を返したときは固定の文言 `error: native elementwise_scalar failed` を標準エラーに出し、終了コードは 2。

#### 試験

`wasm/crates/parity-host/tests/gen.rs`。期待は生成器・`DefaultCpuOps`・`ReferenceCpuOps`・式の呼び出しで作る。

| 試験 | 確かめること |
| ---- | ------------ |
| `native_elementwise_error_text` | `GenError::NativeElementwiseFailed` の文言が固定 |
| `elementwise_input_follows_fresh_generator` | 入力が、新しい生成器の `next_signed_unit_f32` の列から `0.0` を除いた先頭 1,000,003 個とビット列で同じ（2 ケース） |
| `elementwise_input_has_no_special_values` | 件数 1,000,003。NaN 0・Inf 0・`to_bits() & 0x7fff_ffff == 0` の語 0（2 ケース） |
| `elementwise_cases_share_input_bytes` | 2 ケースの `.input.bin` のバイト列が同じ |
| `native_elementwise_matches_default_cpu_ops` | 書いた `.native-scalar.bin` が `DefaultCpuOps.elementwise_scalar` の出力を `f32_le_bytes` で並べたものと同じ（2 ケース） |
| `native_elementwise_matches_expression` | 読み戻した列が、`elementwise-relu` は `x.max(0.0)`、`elementwise-add-scalar` は `x + 0.5` の列とビット列で同じ |
| `native_elementwise_matches_reference_cpu_ops` | 読み戻した列が `ReferenceCpuOps.elementwise_scalar` の出力とビット列で同じ（2 ケース） |
| `elementwise_manifest_entries_match_files` | `elementwise` の 2 要素の鍵がちょうど 9 個。`kind`・`n`・`op_code`（0・1）・`c_bits`（`00000000`・`3f000000`）・sha256 が合う。ファイルの長さが `n`×4。ファイル名が `[a-z0-9.-]` だけ。`.input.bin` が `f32_le_bytes` と同じ |
| `gen_refuses_existing_elementwise_file` | `elementwise-relu.input.bin` が先にあると `FileExists`。ディレクトリの中身はそのファイルだけで、中身も変わらない |

既存の `manifest_hashes_match_files` と `topk_manifest_entries_match_files` の名前の数の期待は、GEMM・部分 Top-k・要素ごと演算のケースの数の和。

#### 確かめ方

時点のコミット: `739a3ae`（この変更の土台）。

```console
$ cargo test --manifest-path wasm/Cargo.toml --workspace --locked 2>&1 | grep '^test result:'
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.85s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

一時のディレクトリに当てた出力（`d` は `mktemp -d` のディレクトリ）。

```console
$ cargo run -q --manifest-path wasm/Cargo.toml -p wasm-parity-host --locked -- gen "$d"
$ (cd "$d" && ls && sha256sum *)
elementwise-add-scalar.input.bin
elementwise-add-scalar.native-scalar.bin
elementwise-relu.input.bin
elementwise-relu.native-scalar.bin
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
e9b99b8c78de25bd6fcf651b4674abbb8e6525d4480ce60c5ef8f52f0061ec52  elementwise-add-scalar.input.bin
25996e901beab1e6891ba2b25366fee752e1c350cad2cc239f4a6878c0c3d509  elementwise-add-scalar.native-scalar.bin
e9b99b8c78de25bd6fcf651b4674abbb8e6525d4480ce60c5ef8f52f0061ec52  elementwise-relu.input.bin
52444bc37b060fd87456b23132c1f5b2a4e32e2d008fc1d1d453cac5b9913588  elementwise-relu.native-scalar.bin
0d509543d75aff9fa7372ca461e5c91dd8e9d8e41c81fe3b302e6610b1228b89  gemm-256.a.bin
e5aae545d81ea7cf7b62df1d03c79055a67967826d0b74ea02c0c68063fbd912  gemm-256.b.bin
d7bc8e3c3e96f39fd560a67bc9446532ce233ee46b805c44d59504cdf39956c1  gemm-256.native-scalar.bin
8b6bdc24887e48cd2bc3598950e2949c599f27b9c14d2a2c11384d2bb9c454d1  gemm-37x41x53.a.bin
60efc5337595142f4ec8d78f08e8e2663f4a67630672e1d21d7cc2f21f90e19e  gemm-37x41x53.b.bin
a0bb2a40eeebf8093a0236ffac4f9c053d7c8fca2267c1e08a3cdd40a5551d2b  gemm-37x41x53.native-scalar.bin
2259215901630df152c4b1e0df09edf21118c82c285b4a9968901fca8ca2ca52  gemm-64.a.bin
9d19ef5ea664105fe1cfbdc69868867fcecde4652eb739d1912527d4652d3eeb  gemm-64.b.bin
25db4c0dde78a6a12cf5449e4c79542e7be1f742993c243a42a31519e8fa6f11  gemm-64.native-scalar.bin
3bd0b6d3537a65b67f3653b9b27ddcc13ad215b7dd388ccbac7a608411734c3d  manifest.json
562e641919b9d368e848150bd5adff9c5f78022dfae32d7bbfa88b3bc63ae91f  topk-batch.native-scalar.bin
e583c60181f74537ccb440169920ca11583822f4e65653b8dd34ea420ebd21be  topk-batch.scores.bin
98fa0af29bc71ec2b760aab079d2091327c9294f52258debc07524215c0062d2  topk-nonfinite.native-scalar.bin
a8c42cd6034f57cb241fa7a4b9023dff42547a234f64d2a9aa152ce2084bcb45  topk-nonfinite.scores.bin
0c0b103cd5f6513b413e98ddba0f27fee9bd4658d3289523035841c98a59aa5c  topk-ties.native-scalar.bin
f951986cfa3ee3f33f94b487d0a65b36cd546269a83b47ac250518d55cbde166  topk-ties.scores.bin
```

定数と関数の所在は `grep -n "ELEMENTWISE_CASES\|pub fn" wasm/crates/parity-host/src/lib.rs` で確かめる。

### 要素ごと演算の 8 行（3/4）

`wasm/node/cases.json` の既定の入力に足した生成の行。並びは部分 Top-k の行の後に、ケースの順・組の順。

| id | ビルド | export | ケース | 参照の出どころ |
| -- | ------ | ------ | ------ | -------------- |
| `elementwise-relu-scalar-elementwise-scalar` | `scalar` | `elementwise_scalar` | `elementwise-relu` | `native-scalar` |
| `elementwise-relu-scalar-elementwise-simd` | `scalar` | `elementwise_simd` | `elementwise-relu` | `native-scalar` |
| `elementwise-relu-simd128-elementwise-scalar` | `simd128` | `elementwise_scalar` | `elementwise-relu` | `native-scalar` |
| `elementwise-relu-simd128-elementwise-simd` | `simd128` | `elementwise_simd` | `elementwise-relu` | `native-scalar` |
| `elementwise-add-scalar-scalar-elementwise-scalar` | `scalar` | `elementwise_scalar` | `elementwise-add-scalar` | `native-scalar` |
| `elementwise-add-scalar-scalar-elementwise-simd` | `scalar` | `elementwise_simd` | `elementwise-add-scalar` | `native-scalar` |
| `elementwise-add-scalar-simd128-elementwise-scalar` | `simd128` | `elementwise_scalar` | `elementwise-add-scalar` | `native-scalar` |
| `elementwise-add-scalar-simd128-elementwise-simd` | `simd128` | `elementwise_simd` | `elementwise-add-scalar` | `native-scalar` |

`wasm/node/cases-reference-unavailable.json` に足した 2 行。`make test-wasm-node-check` が `--cases` で読む。

| id | 参照の出どころ | 回帰テストで出る状態 |
| -- | -------------- | -------------------- |
| `elementwise-relu-native-neon` | `native-neon` | `not-run (reference-unavailable)` |
| `elementwise-add-scalar-native-neon` | `native-neon` | `not-run (reference-unavailable)` |

#### 確かめ方

```console
$ grep -c '"elementwise_' wasm/node/cases.json
8
$ grep -c '"reference_source": "native-neon"' wasm/node/cases-reference-unavailable.json
5
$ make test-wasm-node-check 2>&1 | tail -n 1
passed: 505, failed: 0
```

### 完了の条件の確かめ（4/4）

本リポで観測できる事実を、コマンドと出力だけで記録する（TASK-997）。

時点のコミット: `558960a`（この変更の土台）。(2)〜(8) は、このコミットの main への push で流れた CI の run `42042476628` から取る（event `push`・conclusion `success`・headSha は `558960a09d3f54d61672d58ac753fa7266242be9`）。

| # | 確かめ | コマンド | 結果 |
| - | ------ | -------- | ---- |
| (1) | 2 ケースの入力とネイティブの参照の作り方 | `cargo test --manifest-path wasm/Cargo.toml --workspace --locked -- --list` と同じコマンドの `test result:` の行 | 試験の名前 9 行と `test result:` の 5 行（下のコードブロック） |
| (2) | 8 行の比べ | `gh run view 42042476628 --job 127296593691 --log` の `make wasm-node-check` の段 | ジョブ `wasm32 build + size + node (macos-latest)` は `success`。`node:`・`wasm-bindgen:`・要素ごと演算の `row:` 8 行・`count:` 8 行・`summary:` の行（下のコードブロック） |
| (3) | 入力の特殊値の件数 | 同じログの `make wasm-node-check` の段 | `input-special:` 2 行（下のコードブロック） |
| (4) | 保存した出力からの数え直し | 同じログの `make wasm-node-check` の段 | `recount:` 8 行と `recount-summary:` の行（下のコードブロック） |
| (5) | 出力の sha256 | 同じログの `make wasm-node-check` の段 | `sha256:` 12 行（出力 8・ケースのファイル 4。下のコードブロック） |
| (6) | `wasm-macos` ジョブの環境の行 | 同じジョブの `Set up job`・`Install pinned Node and wasm-bindgen CLI`・`Show tool versions` の段 | 下のコードブロック |
| (7) | 回帰テスト | `gh run view 42042476628 --job 127296592527 --log` の `make test-wasm-node-check` の段 | ジョブ `make deny (cargo-deny)` は `success`。`ok:` 3 行と最終行は `passed: 505, failed: 0` |
| (8) | `tests/cpu_ops_reference.rs` の 1 本と `tests/cpu_ops_elementwise_written.rs` の 2 本の試験 | `gh run view 42042476628 --job 127296593588 --log` の `make verify` の段 | ジョブ `make verify + build-profiles (macos-latest)` は `success`。3 本は `ok`、`test result:` は 2 行（下のコードブロック） |
| (9) | unsafe・外部の依存の追加 0 | `git diff --stat` と `grep -rn unsafe`（下のコードブロック） | stat は空、`grep` は 0 件 |

(1) の出力。

```console
$ cargo test --manifest-path wasm/Cargo.toml --workspace --locked -- --list 2>&1 | grep -E '^(elementwise_cases_share_input_bytes|elementwise_input_follows_fresh_generator|elementwise_input_has_no_special_values|elementwise_manifest_entries_match_files|gen_refuses_existing_elementwise_file|native_elementwise_error_text|native_elementwise_matches_default_cpu_ops|native_elementwise_matches_expression|native_elementwise_matches_reference_cpu_ops): test$'
elementwise_cases_share_input_bytes: test
elementwise_input_follows_fresh_generator: test
elementwise_input_has_no_special_values: test
elementwise_manifest_entries_match_files: test
gen_refuses_existing_elementwise_file: test
native_elementwise_error_text: test
native_elementwise_matches_default_cpu_ops: test
native_elementwise_matches_expression: test
native_elementwise_matches_reference_cpu_ops: test
$ cargo test --manifest-path wasm/Cargo.toml --workspace --locked 2>&1 | grep '^test result:'
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.73s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

(2) の出力。ログの行は `ジョブ名<TAB>段の名前<TAB>時刻 本文` の形で、段の名前で絞って時刻までを落とす。

```console
$ gh run view 42042476628 --job 127296593691 --log | awk -F'\t' '$2=="make wasm-node-check"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | grep -E '^(node|wasm-bindgen|summary): |^(row|count): elementwise-'
node: v24.13.0
wasm-bindgen: 0.2.128
row: elementwise-relu-scalar-elementwise-scalar match
row: elementwise-relu-scalar-elementwise-simd match
row: elementwise-relu-simd128-elementwise-scalar match
row: elementwise-relu-simd128-elementwise-simd match
row: elementwise-add-scalar-scalar-elementwise-scalar match
row: elementwise-add-scalar-scalar-elementwise-simd match
row: elementwise-add-scalar-simd128-elementwise-scalar match
row: elementwise-add-scalar-simd128-elementwise-simd match
count: elementwise-relu-scalar-elementwise-scalar mismatches=0 n=1000003
count: elementwise-relu-scalar-elementwise-simd mismatches=0 n=1000003
count: elementwise-relu-simd128-elementwise-scalar mismatches=0 n=1000003
count: elementwise-relu-simd128-elementwise-simd mismatches=0 n=1000003
count: elementwise-add-scalar-scalar-elementwise-scalar mismatches=0 n=1000003
count: elementwise-add-scalar-scalar-elementwise-simd mismatches=0 n=1000003
count: elementwise-add-scalar-simd128-elementwise-scalar mismatches=0 n=1000003
count: elementwise-add-scalar-simd128-elementwise-simd mismatches=0 n=1000003
summary: rows=32 match=32 mismatch=0 not-run=0
```

(3) の出力（(2) と同じ前段に `grep -E '^input-special: '` をつなぐ）。

```console
input-special: elementwise-relu nan=0 inf=0 zero=0
input-special: elementwise-add-scalar nan=0 inf=0 zero=0
```

(4) の出力（同じ前段に `grep -E '^recount: elementwise-|^recount-summary: '` をつなぐ）。

```console
recount: elementwise-relu-scalar-elementwise-scalar mismatches=0 n=1000003
recount: elementwise-relu-scalar-elementwise-simd mismatches=0 n=1000003
recount: elementwise-relu-simd128-elementwise-scalar mismatches=0 n=1000003
recount: elementwise-relu-simd128-elementwise-simd mismatches=0 n=1000003
recount: elementwise-add-scalar-scalar-elementwise-scalar mismatches=0 n=1000003
recount: elementwise-add-scalar-scalar-elementwise-simd mismatches=0 n=1000003
recount: elementwise-add-scalar-simd128-elementwise-scalar mismatches=0 n=1000003
recount: elementwise-add-scalar-simd128-elementwise-simd mismatches=0 n=1000003
recount-summary: rows=27 zero=27 nonzero=0 uncompared=0
```

(5) の出力（同じ前段に `grep -E '^sha256: elementwise-'` をつなぐ）。

```console
sha256: elementwise-relu-scalar-elementwise-scalar 52444bc37b060fd87456b23132c1f5b2a4e32e2d008fc1d1d453cac5b9913588
sha256: elementwise-relu-scalar-elementwise-simd 52444bc37b060fd87456b23132c1f5b2a4e32e2d008fc1d1d453cac5b9913588
sha256: elementwise-relu-simd128-elementwise-scalar 52444bc37b060fd87456b23132c1f5b2a4e32e2d008fc1d1d453cac5b9913588
sha256: elementwise-relu-simd128-elementwise-simd 52444bc37b060fd87456b23132c1f5b2a4e32e2d008fc1d1d453cac5b9913588
sha256: elementwise-add-scalar-scalar-elementwise-scalar 25996e901beab1e6891ba2b25366fee752e1c350cad2cc239f4a6878c0c3d509
sha256: elementwise-add-scalar-scalar-elementwise-simd 25996e901beab1e6891ba2b25366fee752e1c350cad2cc239f4a6878c0c3d509
sha256: elementwise-add-scalar-simd128-elementwise-scalar 25996e901beab1e6891ba2b25366fee752e1c350cad2cc239f4a6878c0c3d509
sha256: elementwise-add-scalar-simd128-elementwise-simd 25996e901beab1e6891ba2b25366fee752e1c350cad2cc239f4a6878c0c3d509
sha256: elementwise-relu.input e9b99b8c78de25bd6fcf651b4674abbb8e6525d4480ce60c5ef8f52f0061ec52
sha256: elementwise-relu.native-scalar 52444bc37b060fd87456b23132c1f5b2a4e32e2d008fc1d1d453cac5b9913588
sha256: elementwise-add-scalar.input e9b99b8c78de25bd6fcf651b4674abbb8e6525d4480ce60c5ef8f52f0061ec52
sha256: elementwise-add-scalar.native-scalar 25996e901beab1e6891ba2b25366fee752e1c350cad2cc239f4a6878c0c3d509
```

(6) の出力。`Set up job` の段は `Operating System` と `Runner Image` の group の行、`Install pinned Node and wasm-bindgen CLI` の段は Node の tar の行、`Show tool versions` の段は末尾の 4 行。

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

(7) の出力。

```console
$ gh run view 42042476628 --job 127296592527 --log | awk -F'\t' '$2=="make test-wasm-node-check"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | grep -E '^ok: (22 bundled elementwise rows are exactly|77 reference-unavailable rows|77 the file has five rows)'
ok: 22 bundled elementwise rows are exactly the eight (case, export, build) pairs
ok: 77 reference-unavailable rows
ok: 77 the file has five rows, all native-neon
$ gh run view 42042476628 --job 127296592527 --log | awk -F'\t' '$2=="make test-wasm-node-check"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | tail -n 1
passed: 505, failed: 0
```

(8) の出力（`make verify` の段のうち、試験の名前を含む `running` の行から `test result:` までの塊）。

```console
$ gh run view 42042476628 --job 127296593588 --log | awk -F'\t' '$2=="make verify"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | awk '/^running [0-9]+ tests?$/{b="";k=0} /^test elementwise_simd_matches_scalar_bits \.\.\. /{b=b $0 "\n";k=1} /^test result:/{if(k){printf "%s%s\n",b,$0;exit}}'
test elementwise_simd_matches_scalar_bits ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.13s
$ gh run view 42042476628 --job 127296593588 --log | awk -F'\t' '$2=="make verify"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | awk '/^running [0-9]+ tests?$/{b="";k=0} /^test (relu_writes_every_element|add_scalar_writes_every_element) \.\.\. /{b=b $0 "\n";k=1} /^test result:/{if(k){printf "%s%s\n",b,$0;exit}}'
test add_scalar_writes_every_element ... ok
test relu_writes_every_element ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.71s
```

(9) の出力。1/4 の base は `f76f567`。

```console
$ git diff --stat f76f567..558960a -- wasm/deps-approved.toml deps-approved.toml Cargo.toml Cargo.lock wasm/Cargo.toml wasm/Cargo.lock crates .github
$ grep -rn unsafe wasm/crates/parity-host
```

### 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | Node での照合に要素ごと演算の種類を足す | #6287 |
| 2/4 | 要素ごと演算の 2 ケースの入力とネイティブの参照を host 側の道具に足す | #6288 |
| 3/4 | 要素ごと演算の 8 行を照合の入力に足す | #6289 |
| 4/4 | 完了の条件の確かめ | #6290 |

## 今の値の補い（base 42731d8）

記録時点の表と既存の節は書き換えない。以下は base `42731d8` で観測した今の値（実行環境は GPU なしの Linux。node・wasm-bindgen の CLI は入れていないので `make wasm-node-check` の実物の実行は流していない）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 行の 2 つの形の表の生成の行の `export`（`:137`） | `gemm_scalar`・`gemm_simd` → `topk_scalar`・`topk_simd`・`elementwise_scalar`・`elementwise_simd` の 4 つが加わった 6 つ | `grep -n 'call:' wasm/node/run-glue.cjs` |
| 「判定と出力の行」の表（`:177`〜`:181`）の「4 バイトの語ごとの比べ」 | 語ごとの比べは 4 バイト → 種類ごとの比べの単位（`gemm`・`elementwise` は 4 バイト、`topk` は 8 バイト）。違いがあれば `mismatch (mismatches <件数>/<単位の数>)`、数が違えば `mismatch (length expected <n> got <m>)`（単位の数）。`topk` の生成の行には `nonfinite: <ケース> input=<件数> output=<件数>`、`elementwise` の生成の行には `input-special: <ケース> nan=<件数> inf=<件数> zero=<件数>` の行が加わる | `sed -n 80,95p scripts/wasm-node-check.sh`、`grep -n 'mismatches\|length expected\|nonfinite:\|input-special:' scripts/wasm-node-check.sh` |
| 確かめ方の `grep -c '"case"' wasm/node/cases.json`（`:231`〜`:232`） | `9` → `27` | `grep -c '"case"' wasm/node/cases.json` |
| 「保存の形のファイル」の manifest の鍵（`:247`〜`:252`） | `a_file`・`b_file`・`native_scalar_file` → `topk` の要素は `scores_file`、`elementwise` の要素は `input_file` を持ち、どちらも `native_scalar_file` を持つ | `grep -n 'scores_file\|input_file' wasm/crates/parity-host/src/lib.rs` |
| 「今の値の補い（base 5ea11ff）」の `make test-wasm-node-check` の最終行（`:503`） | `passed: 342, failed: 0` → `passed: 505, failed: 0` | `make test-wasm-node-check` |
| TASK-996 の確かめ方の `make test-wasm-node-check`（`:574`） | `passed: 416, failed: 0` → `passed: 505, failed: 0` | `make test-wasm-node-check` |
| `wasm-parity-host gen <dir>` の記述の `wasm/node/cases.json`（`:581`） | 14 行 → 32 行 | `jq '.rows \| length' wasm/node/cases.json` |
| `manifest.json` の `cases`（`:603`） | 6 要素・書く名前は 16 個 → 8 要素（GEMM 3・部分 Top-k 3・要素ごと演算 2）・書く名前は 20 個（`.bin` が 19 個と `manifest.json`） | `d=$(mktemp -d); (cd wasm && cargo run -q -p wasm-parity-host -- gen "$d"); ls "$d" \| wc -l; grep -c '"name"' "$d/manifest.json"` |
| `sha256sum *` の `manifest.json`（`:668`） | `92c07bf7987315eac089e4d7c0d094715f4a7d890e08fca1d0027ea5619e0e14` → `3bd0b6d3537a65b67f3653b9b27ddcc13ad215b7dd388ccbac7a608411734c3d`（`topk` の要素の鍵と `elementwise` の要素が加わった `gen` の出力） | 上の `gen` を流し、`(cd "$d" && sha256sum manifest.json)` |
| TASK-996 の 4/4 の (8) のコマンド（`:866`）の切り方 | `Running tests/cpu_ops_topk_reference.rs` の行から最初の `test result:` まで切る形（`Running` の行の順に依る） → `running N tests` の行で区切り、対象の試験の行を含む組の `test result:` を出す形（順に依らない。TASK-997 の 4/4 の `:1234`・`:1237` と同じ形）。下に今の確かめ方を書く | 下のコマンド |

順に依らないコマンドの形（`<run>`・`<job>` は `make verify` の段を持つ CI のジョブ。`gh` と認証が要る）。

```console
$ gh run view <run> --job <job> --log | awk -F'\t' '$2=="make verify"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | awk '/^running [0-9]+ tests?$/{b="";k=0} /^test (topk_simd_matches_scalar_bytes|topk_simd_scalar_byte_sequences_match) \.\.\. /{b=b $0 "\n";k=1} /^test result:/{if(k){printf "%s%s\n",b,$0;exit}}'
```

## simd128 のビルドの GEMM の比べ（TASK-995）

### 記録の所在

記録の正は private spec 側（`docs/spec/05-tasks.md` の TASK-995。ポインタのみ）。本文は転記しない
（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

### Node での照合の閾値の比べ（1/4）

`scripts/wasm-node-check.sh` の判定に、完全一致でなく T-1 の閾値で語ごとに比べる行の種類を足す。比べの種類は `(export, build)` の組と種類の表 `KINDS` から引き、`cases.json` の行・`outputs.json`・manifest の鍵は変えない。判定の側は行の `export`・`build`、`--recount` の側は `outputs.json` の行の `build` で引く（引く関数は `uses_t1`）。

#### 種類の表

表の全項に欄 `t1`（閾値で比べる `(export, build)` の組の列）を足す。

| 種類 | `t1` の欄 |
| ---- | --------- |
| `gemm` | `("gemm_simd", "simd128")` の 1 組 |
| `topk` | 空 |
| `elementwise` | 空 |

欄に無い組の行（`gemm` の `gemm_scalar` の 2 行・`gemm_simd` の `scalar` の行を含む）は、今までどおりビットの完全一致で比べる。

#### 比べの規則

閾値で比べる行は、参照と実測の 4 バイトの語を f32 として読み、語ごとに次の順で判定する（共通の道具 `T1` と特殊値つきの要素ごとの判定と同じ順）。誤差は `f64`、比べは厳密な `<`、相対の分母は参照。

| 参照 | 実測 | 判定 |
| ---- | ---- | ---- |
| NaN | NaN | 一致（payload・符号は見ない） |
| NaN | NaN 以外 | 不一致 |
| ±Inf | 同じ符号の Inf | 一致 |
| ±Inf | それ以外 | 不一致 |
| 有限 | 有限でない | 不一致 |
| 有限 | 有限 | 絶対誤差が絶対の閾値未満、または相対誤差が相対の閾値未満なら一致。参照が `+0.0`・`-0.0` のときは相対誤差は無く、絶対誤差だけで判定する |

#### 判定と出力の行

| 場面 | 状態・行 |
| ---- | -------- |
| 不一致の語がある | `mismatch (mismatches <件数>/<n>)` |
| 語の数が違う | `mismatch (length expected <n> got <m>)`。`count:`・`sha256:`・`t1:` は出さない |
| 全語が一致 | `match`。`count:`・`sha256:` は今までの形で、閾値で比べる行は続けて `t1:` を出す |
| `native-neon` の `gemm_simd`・`simd128` の行 | `not-run (reference-unavailable)`。`count:`・`t1:` を出さない |

`t1: <行の id> max-abs=<値> max-abs-at=<添字か -> max-rel=<値> max-rel-at=<添字か -> abs-over=<件数> rel-over=<件数>` は、閾値で比べる行だけが `sha256: <行の id>` の行の次に出す（判定・`--recount` の両方）。

| 欄 | 数え方 |
| -- | ------ |
| `max-abs`・`max-abs-at` | 参照・実測がどちらも有限の語の絶対誤差の最大と、最初にその値を取った語の添字。`0.0` から始め、厳密に大きいときだけ更新し、1 度も更新しなければ添字は `-` |
| `max-rel`・`max-rel-at` | 同じ語の相対誤差（参照が零の語は無し）の最大と添字。更新の規則は同じ |
| `abs-over` | 絶対誤差が絶対の閾値以上の語の数 |
| `rel-over` | 相対誤差があり、相対の閾値以上の語の数 |

数値は Python の `repr` で出す。この行は出すだけで、判定と終了コードには関わらない（`abs-over`・`rel-over` が 0 でなくても `match` の場面がある）。実行の中の数え直しは、この行も判定の側と突き合わせる（違えば `recount disagrees` で終了コード 2）。

#### 閾値の所在

閾値はスクリプトの目印 `# BEGIN t1-thresholds`〜`# END t1-thresholds` の間の 2 つの定数 `T1_RELATIVE`・`T1_ABSOLUTE` だけに書く（共通の道具の `T1` と同じ値。引数・環境変数・入力では変えない）。頁には数値を書かない。

#### 回帰テスト

`scripts/test-wasm-node-check.sh` の足した項目。合成のケース `gemm-t1`（m=1・k=1・n=8。参照に有限の語・零・NaN・±Inf を持つ）を `mk_host_fixture` に足している。

| 項目 | 確かめること |
| ---- | ------------ |
| 89 | 全語が同じ行の `count:`・`sha256:`・`t1:` と並び |
| 90 | 閾値で比べるのは `gemm_simd`・`simd128` の行だけで、ほかの 2 行は同じ出力でもビットの比べで不一致 |
| 91 | 絶対の最大と相対の最大が別の語で、不一致 0・終了コード 0 |
| 92 | 相対誤差がちょうど閾値の語（上下とも不一致）と、下回る語 |
| 93 | 参照が零の語（`-0.0`・絶対の閾値の直下と直上・負の側） |
| 94 | NaN（quiet の最小・signaling の最小・符号違い）と ±Inf の組、集計に入らないこと |
| 95・96 | 語数の違い・4 の倍数でない出力・呼び出しの失敗、`native-neon` の行 |
| 97 | `--output-dir` と `--recount` の `t1:` の行・並び、保存の形の語と `build` を変えた場合 |
| 98 | 数え直しの側の `t1_count` の引数だけを入れ替えた変異体が `recount disagrees` で止まること |

#### 確かめ方

```console
$ make test-wasm-node-check 2>&1 | tail -n 1
passed: 542, failed: 0
$ grep -n "BEGIN t1-thresholds\|^def t1_count\|^def uses_t1\|\"t1\":" scripts/wasm-node-check.sh
```

### 閾値の比べと共通の道具の突き合わせ（2/4）

`wasm/crates/parity-host/tests/t1_compare.rs` は、スクリプトの `--recount` の閾値の比べを共通の道具（`T1`・`compare_f32_with_special_values`）の答えと突き合わせる。試験は `m=1`・`k=1` の `gemm` のケースの保存の形（`gemm_simd`・`simd128` の 1 行）を一時ディレクトリに書き、`bash scripts/wasm-node-check.sh --recount <dir>` を流す。python3 が無いときは試験が落ちる。

| 試験 | 何と何を比べるか |
| ---- | ---------------- |
| `script_thresholds_match_core` | 目印の間の `T1_RELATIVE`・`T1_ABSOLUTE` の値と、`T1` の getter の値（ビット一致）。目印が 1 組でない・行が無いときは落ちる |
| `recount_agrees_with_core_on_boundary_words` | 境界の語の 20 組を 1 組ずつと全部を並べた列で、`recount: row` の `mismatches` と終了コードを、共通の道具の不一致の件数と比べる。`t1: row` の `max-abs`・`max-rel`・`abs-over`・`rel-over` を、試験の中で式で数えた値と比べる |
| `recount_agrees_with_core_on_generated_words` | `SplitMix64` の 4,096 語の参照と、揺れを足した実測で同じ比べをし、不一致の件数が 0 でも全語でもないことを確かめる |

#### 確かめ方

```console
$ cargo test --manifest-path wasm/Cargo.toml -p wasm-parity-host --test t1_compare --locked
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.73s
$ cargo test --manifest-path wasm/Cargo.toml -p wasm-parity-host --test t1_compare --locked -- --list
recount_agrees_with_core_on_boundary_words: test
recount_agrees_with_core_on_generated_words: test
script_thresholds_match_core: test
```

時点のコミット: `cfea6d2`（この変更の土台）。

### simd128 のビルドの `gemm_simd` の 3 行（3/4）

`wasm/node/cases.json` の既定の入力に足した生成の行。並びは要素ごと演算の行の後に、ケースの順。

| id | ビルド | export | ケース | 参照の出どころ |
| -- | ------ | ------ | ------ | -------------- |
| `gemm-64-simd128-gemm-simd` | `simd128` | `gemm_simd` | `gemm-64` | `native-scalar` |
| `gemm-256-simd128-gemm-simd` | `simd128` | `gemm_simd` | `gemm-256` | `native-scalar` |
| `gemm-37x41x53-simd128-gemm-simd` | `simd128` | `gemm_simd` | `gemm-37x41x53` | `native-scalar` |

`wasm/node/cases-reference-unavailable.json` に足した 3 行。`make test-wasm-node-check` が `--cases` で読む。

| id | 参照の出どころ | 回帰テストで出る状態 |
| -- | -------------- | -------------------- |
| `gemm-64-native-neon` | `native-neon` | `not-run (reference-unavailable)` |
| `gemm-256-native-neon` | `native-neon` | `not-run (reference-unavailable)` |
| `gemm-37x41x53-native-neon` | `native-neon` | `not-run (reference-unavailable)` |

#### 確かめ方

```console
$ grep -c '"export": "gemm_simd"' wasm/node/cases.json
6
$ grep -c '"reference_source": "native-neon"' wasm/node/cases-reference-unavailable.json
8
$ make test-wasm-node-check 2>&1 | tail -n 1
passed: 542, failed: 0
```

### 完了の条件の確かめ（4/4）

本リポで観測できる事実を、コマンドと出力だけで記録する（TASK-995）。

時点のコミット: `75cfdde`（この変更の土台）。(2)〜(8) は、このコミットの main への push で流れた CI の run `42118684042` から取る（event `push`・conclusion `success`・headSha は `75cfdde6fa36fb7f4a2d5cd9e75b962f6ee52d96`）。

| # | 確かめ | コマンド | 結果 |
| - | ------ | -------- | ---- |
| (1) | 閾値の比べの試験 | `cargo test --manifest-path wasm/Cargo.toml --workspace --locked -- --list` と同じコマンドの `test result:` の行 | 試験の名前 3 行と `test result:` の 6 行（下のコードブロック） |
| (2) | 3 行の比べ | `gh run view 42118684042 --job 127565001893 --log` の `make wasm-node-check` の段 | ジョブ `wasm32 build + size + node (macos-latest)` は `success`。`node:`・`wasm-bindgen:`・`isa-` の `row:` 2 行・`gemm_simd` の `row:` 3 行・`count:` 3 行・`summary:` の行（下のコードブロック） |
| (3) | 最大誤差の集計 | 同じログの `make wasm-node-check` の段 | `t1:` 3 行（下のコードブロック） |
| (4) | 保存した出力からの数え直し | 同じログの `make wasm-node-check` の段 | `recount:` 3 行と `recount-summary:` の行（下のコードブロック） |
| (5) | 出力の sha256 | 同じログの `make wasm-node-check` の段 | `sha256:` 12 行（出力 3・ケースのファイル 9。下のコードブロック） |
| (6) | `wasm-macos` ジョブの環境の行 | 同じジョブの `Set up job`・`Install pinned Node and wasm-bindgen CLI`・`Show tool versions` の段 | 下のコードブロック |
| (7) | 回帰テスト | `gh run view 42118684042 --job 127565001825 --log` の `make test-wasm-node-check` の段 | ジョブ `make deny (cargo-deny)` は `success`。`grep` で絞った `ok:` 40 行（ログ全体の `ok:` は 542 行で最終行の件数と同じ）と最終行 `passed: 542, failed: 0` |
| (8) | 閾値の突き合わせの試験の実物の実行 | 同じ run の `wasm-macos` ジョブの `make wasm-verify` の段 | 3 本は `ok`、`test result:` は 1 行（下のコードブロック） |
| (9) | unsafe・外部の依存の追加 0 | `git diff --stat` と `grep -rn unsafe`（下のコードブロック） | stat は空、`grep` は 0 件 |

(1) の出力。

```console
$ cargo test --manifest-path wasm/Cargo.toml --workspace --locked -- --list 2>&1 | grep -E '^(recount_agrees_with_core_on_boundary_words|recount_agrees_with_core_on_generated_words|script_thresholds_match_core): '
recount_agrees_with_core_on_boundary_words: test
recount_agrees_with_core_on_generated_words: test
script_thresholds_match_core: test
$ cargo test --manifest-path wasm/Cargo.toml --workspace --locked 2>&1 | grep '^test result:'
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.75s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.77s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

(2) の出力。ログの行は `ジョブ名<TAB>段の名前<TAB>時刻 本文` の形で、段の名前で絞って時刻までを落とす。

```console
$ gh run view 42118684042 --job 127565001893 --log | awk -F'\t' '$2=="make wasm-node-check"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | grep -E '^(node|wasm-bindgen|summary): |^row: isa-(scalar|simd128) |^(row|count): gemm-(64|256|37x41x53)-simd128-gemm-simd '
node: v24.13.0
wasm-bindgen: 0.2.128
row: isa-scalar match
row: isa-simd128 match
row: gemm-64-simd128-gemm-simd match
row: gemm-256-simd128-gemm-simd match
row: gemm-37x41x53-simd128-gemm-simd match
count: gemm-64-simd128-gemm-simd mismatches=0 n=4096
count: gemm-256-simd128-gemm-simd mismatches=0 n=65536
count: gemm-37x41x53-simd128-gemm-simd mismatches=0 n=1517
summary: rows=35 match=35 mismatch=0 not-run=0
```

(3) の出力（(2) と同じ前段に `grep -E '^t1: '` をつなぐ）。

```console
t1: gemm-64-simd128-gemm-simd max-abs=2.86102294921875e-06 max-abs-at=1063 max-rel=0.0005874369857257919 max-rel-at=1828 abs-over=0 rel-over=0
t1: gemm-256-simd128-gemm-simd max-abs=1.52587890625e-05 max-abs-at=60159 max-rel=0.012768817995209193 max-rel-at=49160 abs-over=26 rel-over=9
t1: gemm-37x41x53-simd128-gemm-simd max-abs=2.86102294921875e-06 max-abs-at=60 max-rel=0.0002225181404437151 max-rel-at=1292 abs-over=0 rel-over=0
```

(4) の出力（同じ前段に `grep -E '^recount: gemm-(64|256|37x41x53)-simd128-gemm-simd |^recount-summary: '` をつなぐ）。

```console
recount: gemm-64-simd128-gemm-simd mismatches=0 n=4096
recount: gemm-256-simd128-gemm-simd mismatches=0 n=65536
recount: gemm-37x41x53-simd128-gemm-simd mismatches=0 n=1517
recount-summary: rows=30 zero=30 nonzero=0 uncompared=0
```

(5) の出力（同じ前段に `grep -E '^sha256: gemm-(64|256|37x41x53)(\.(a|b|native-scalar)|-simd128-gemm-simd) '` をつなぐ）。

```console
sha256: gemm-64-simd128-gemm-simd df0ff87ae22cf6956ae71aa4abd2579c95ce09a8b732aa13a4689ce080bba9e4
sha256: gemm-256-simd128-gemm-simd 85edcd2d5973add713f0aacab2e5b170b49096b364545582b9e75643561ecc10
sha256: gemm-37x41x53-simd128-gemm-simd dcd8a97f3e4925ef029b8f5f28ee208f6ccbda62675f5220b82df94ee6e7e7a9
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

(6) の出力。`Set up job` の段は `Operating System` と `Runner Image` の group の行、`Install pinned Node and wasm-bindgen CLI` の段は Node の tar の行、`Show tool versions` の段は末尾の 4 行。

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

(7) の出力。

```console
$ gh run view 42118684042 --job 127565001825 --log | awk -F'\t' '$2=="make test-wasm-node-check"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | grep -E '^ok: (22 bundled generated rows are exactly|77 reference-unavailable rows|77 the file has eight rows|89 |9[0-8] )'
ok: 22 bundled generated rows are exactly 3 cases x (scalar gemm_scalar, scalar gemm_simd, simd128 gemm_scalar, simd128 gemm_simd)
ok: 77 reference-unavailable rows
ok: 77 the file has eight rows, all native-neon
ok: 89 identical words are within the threshold
ok: 89 output order is count, sha256, t1
ok: 90 only the gemm_simd simd128 row is compared by the threshold
ok: 91 the absolute and relative maxima are at different words
ok: 92 relative error exactly at the limit (above)
ok: 92 relative error exactly at the limit (below)
ok: 92 relative error under the limit
ok: 93 reference zero, the other sign of zero
ok: 93 reference zero, just under the absolute limit
ok: 93 reference zero, just over the absolute limit
ok: 93 reference zero, negative just over the absolute limit
ok: 94 reference NaN matches a NaN (7f800001)
ok: 94 reference NaN matches a NaN (ffc00001)
ok: 94 reference NaN matches a NaN (7fc00000)
ok: 94 reference NaN against a finite word
ok: 94 finite reference against a non-finite word (7fc00000)
ok: 94 finite reference against a non-finite word (7f800001)
ok: 94 finite reference against a non-finite word (7f800000)
ok: 94 finite reference against a non-finite word (ff800000)
ok: 94 reference +Inf against +Inf
ok: 94 reference +Inf against another word (ff800000)
ok: 94 reference +Inf against another word (7f7fffff)
ok: 94 reference +Inf against another word (7fc00000)
ok: 94 reference -Inf against +Inf
ok: 95 length differs
ok: 95 a 33-byte output is not a multiple of 4
ok: 95 a call error
ok: 96 native-neon gemm_simd simd128 row
ok: 97 saved form of a threshold row
ok: 97 the file names are exactly the saved form
ok: 97 --recount prints the same t1 line
ok: 97 --recount order is recount, sha256, t1, recount-summary
ok: 97 --recount of a saved form over the threshold
ok: 97 --recount of a row whose build is scalar compares the bits
ok: 98 the mutant differs from the original
ok: 98 a disagreeing recount of the t1 lines stops the run
ok: 98 nothing written to either directory
$ gh run view 42118684042 --job 127565001825 --log | awk -F'\t' '$2=="make test-wasm-node-check"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | tail -n 1
passed: 542, failed: 0
```

(8) の出力（`make wasm-verify` の段のうち、試験の名前を含む `running` の行から `test result:` までの塊）。

```console
$ gh run view 42118684042 --job 127565001893 --log | awk -F'\t' '$2=="make wasm-verify"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | awk '/^running [0-9]+ tests?$/{b="";k=0} /^test (script_thresholds_match_core|recount_agrees_with_core_on_boundary_words|recount_agrees_with_core_on_generated_words) \.\.\. /{b=b $0 "\n";k=1} /^test result:/{if(k){printf "%s%s\n",b,$0;exit}}'
test script_thresholds_match_core ... ok
test recount_agrees_with_core_on_generated_words ... ok
test recount_agrees_with_core_on_boundary_words ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.54s
```

(9) の出力。1/4 の base は `450a581`。

```console
$ git diff --stat 450a581..75cfdde -- wasm/deps-approved.toml deps-approved.toml Cargo.toml Cargo.lock wasm/Cargo.toml wasm/Cargo.lock crates .github wasm/crates/parity-host/src wasm/crates/parity-host/Cargo.toml wasm/node/run-glue.cjs
$ grep -rn unsafe wasm/crates/parity-host
```

### 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | Node での照合に閾値の比べを足す | #6277 |
| 2/4 | 閾値の比べを共通の道具の口と突き合わせる試験を足す | #6278 |
| 3/4 | simd128 のビルドの `gemm_simd` の 3 行を照合の入力に足す | #6279 |
| 4/4 | 完了の条件の確かめ | #6280 |

## 今の値の補い（base e416527）

記録時点の表と既存の節は書き換えない。以下は base `e416527` で観測した今の値（実行環境は GPU なしの Linux。node・wasm-bindgen の CLI は入れていないので `make wasm-node-check` の実物の実行は流していない。`summary:`・`recount-summary:` の行の数は `cases.json` の行数から数える）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 「判定と出力の行」の表（`:177`〜`:181`）と「今の値の補い（base 42731d8）」の同じ行（`:1266`）の語ごとの比べ | ビットの完全一致の比べ（`gemm`・`elementwise` は 4 バイト、`topk` は 8 バイト）だけ → `gemm` の `gemm_simd`・`simd128` の行は閾値で語ごとに比べ、`t1: <行の id> max-abs=… max-rel=… abs-over=… rel-over=…` の行も出す。ほかの行はビットの完全一致のまま | `grep -n 't1\|T1' scripts/wasm-node-check.sh`、上の「Node での照合の閾値の比べ（1/4）」 |
| 確かめ方の `grep -c '"case"' wasm/node/cases.json`（`:231`〜`:232`・`:1267`） | `9`・`27` → `50` | `grep -c '"case"' wasm/node/cases.json` |
| `cases-reference-unavailable.json` の `native-neon` の行（`:709`〜`:710`・`:1089`〜`:1094`） | `3`・`5` → `8` | `grep -c '"reference_source": "native-neon"' wasm/node/cases-reference-unavailable.json` |
| `make test-wasm-node-check` の最終行（`:1269`・`:1270`・`:1362`・`:1415`・`:1569`） | `passed: 505, failed: 0`・`passed: 542, failed: 0` → `passed: 544, failed: 0` | `make test-wasm-node-check 2>&1 \| tail -n 1` |
| `wasm/node/cases.json` の行数（`:1271`） | 32 行・35 行 → 55 行 | `jq '.rows \| length' wasm/node/cases.json` |
| `manifest.json` の `cases` と書く名前（`:982`・`:1272`） | 8 要素（GEMM 3・部分 Top-k 3・要素ごと演算 2）・書く名前は 20 個 → 13 要素（GEMM 3・部分 Top-k 8・要素ごと演算 2）・書く名前は 30 個（`.bin` が 29 個と `manifest.json`） | `d=$(mktemp -d); (cd wasm && cargo run -q -p wasm-parity-host -- gen "$d"); ls "$d" \| wc -l; jq -r '.cases[].kind' "$d/manifest.json" \| sort \| uniq -c` |
| `sha256sum *` の `manifest.json`（`:668`・`:1273`） | `3bd0b6d3537a65b67f3653b9b27ddcc13ad215b7dd388ccbac7a608411734c3d` → `fae32ffa06473452882cfb5c5991c0ac2ed0af3807b09beee867b50ba5c3f987` | 上の `gen` を流し、`(cd "$d" && sha256sum manifest.json)` |
| 回帰テストの項目 `22 bundled topk rows are exactly the ten (case, export, build) pairs`（`:855`） | `the ten` → `the thirty` | `grep -n 'bundled topk rows' scripts/test-wasm-node-check.sh` |
| `wasm-parity-host` の `tests/gen.rs` の `test result:`（`:1011`・`:1132`・`:1447`） | `28 passed` → `36 passed` | `cargo test --manifest-path wasm/Cargo.toml --workspace --locked 2>&1 \| grep '^test result:'` |
| `summary:` の行（`:1466`） | `rows=35 match=35 mismatch=0 not-run=0` → `cases.json` が 55 行なので行の数は `rows=55` | `jq '.rows \| length' wasm/node/cases.json` |
| `recount-summary:` の行（`:1483`） | `rows=30 zero=30 nonzero=0 uncompared=0` → 保存の形のある行の数は `cases.json` の 55 行から `detect_isa_bits` 2 行・`roundtrip_f32` 3 行を除いた `rows=50` | `jq -r '.rows[].export' wasm/node/cases.json \| sort \| uniq -c` |
| 「Node での照合の閾値の比べ（1/4）」の綴り（`:1291`） | 文中の `T-1` → `T1`（スクリプトの目印の間のコメントの綴りに揃った） | `sed -n 86,93p scripts/wasm-node-check.sh` |

## 特殊値の入力の比べ（TASK-999）

### 記録の所在

記録の正は private spec 側（`docs/spec/05-tasks.md` の TASK-999。ポインタのみ）。本文は転記しない
（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

### Node での照合の特殊値の入力の種類（1/6）

`scripts/wasm-node-check.sh` の種類の表 `KINDS` に、ケースの種類を 2 つ足し、wasm の出力を CPU 参照のファイルとも比べる口を足す。実物の `wasm/node/cases.json` と host 側の道具（`wasm/crates/parity-host`）はこの段では変えない。

#### 種類の表

| manifest の `kind` | ファイル（鍵） | 閾値で比べる組 | `sha256:` の接尾辞 |
| ------------------ | -------------- | -------------- | ------------------ |
| `gemm-special` | `a_file`・`b_file`・`native_scalar_file`・`cpu_reference_file` | `gemm_scalar`・`gemm_simd` × `scalar`・`simd128` の 4 組 | `a`・`b`・`native-scalar`・`cpu-reference` |
| `elementwise-special` | `input_file`・`native_scalar_file`・`cpu_reference_file`（`op_code`・`c_bits` の欄は `elementwise` と同じ） | 同じ 4 組 | `input`・`native-scalar`・`cpu-reference` |

- 表の欄 `cpu` が CPU 参照のファイルの鍵を持つ（既存の 3 種類は `None`）。
- 鍵の集合は完全一致で、`cpu_reference_file`・`cpu_reference_sha256` が足りなくても余分でも終了コード 2。ファイルは長さ（出力と同じ語数）・sha256・通常のファイルであることを既存の種類と同じ表で照らす。
- 生成の行の `export` は `gemm_scalar`・`gemm_simd`・`elementwise_scalar`・`elementwise_simd` のまま。行の `export` が許される種類は表の `exports` で引く（`GEN_EXPORTS` は表から重複なしで作る）。

#### 語の比べ

閾値で比べる組の語の規則は `t1_count` で、「simd128 のビルドの GEMM の比べ」の節の規則と同じ。`cpu_count` は CPU 参照のファイルを読み、同じ `t1_count` で wasm の出力と語ごとに比べて不一致の語の数を返す（CPU 参照のファイルを持たない種類は `None`）。

#### 判定と出力の行

- 行は、ネイティブの参照との不一致が 0 件で、かつ CPU 参照との不一致が 0 件のときだけ `match`。
- ネイティブの参照との不一致があれば `row: <id> mismatch (mismatches <件数>/<語数>)`、そうでなく CPU 参照との不一致があれば `row: <id> mismatch (cpu-reference mismatches <件数>/<語数>)`。
- `cpu-count: <id> mismatches=<件数> n=<語数>` を、CPU 参照を持つ種類の行にだけ `count:` の直後に出す（`count:` → `cpu-count:` → `sha256:` → `t1:` の順）。
- `--output-dir` の保存の形に `<ケース>.cpu-reference.bin` が入り、`--recount` は `recount-cpu: <id> mismatches=<件数> n=<語数>` を `recount:` の直後に出す。`recount-summary:` の `zero` はネイティブの参照・CPU 参照のどちらとも 0 件の行の数、終了コードは CPU 参照との不一致があれば 1。
- 実行の中の数え直しは、判定の `(不一致の数, 語数, CPU 参照との不一致の数)` と同じ 3 つ組を突き合わせ、違えば `error: recount disagrees`（終了コード 2）。
- 参照の出どころが使えない行（`reference-unavailable`）は、これまでどおり比べず `count:`・`cpu-count:` も出さない。
- `--record-dir` の部品は `ng` のとき `detail` を持たない（CPU 参照との不一致の語も部品に入らない）。

#### export とケースの種類の照らし

行の `export` が、行のケースの種類の `exports` に無ければ終了コード 2（`row export does not match the case kind`）。`gemm_*` は `gemm` と `gemm-special` の両方を、`elementwise_*` は `elementwise` と `elementwise-special` の両方を指せる。`topk_*` は特殊値の 2 種類を指せない。

#### 回帰テスト

`scripts/test-wasm-node-check.sh` の末尾の節（項目 99〜110）。`gemm-special`（m=2・k=3・n=4）と `elementwise-special`（n=8）のそれぞれを、4 組すべての行で流す。

| 項目 | 確かめること |
| ---- | ------------ |
| 99 | 全行一致（0）・`cpu-count:`・`t1:`・`sha256: <ケース>.cpu-reference`・`recount-cpu:` の行と順・`input-special:` の行は `elementwise-special` にだけ出る |
| 100 | NaN の payload だけの違い・`+0.0` と `-0.0` の違いは一致。同じ変更が既存の `gemm`・`elementwise` の行では不一致のまま |
| 101 | 片方だけ NaN・Inf の符号違い・有限と NaN は不一致 |
| 102 | 閾値の内は一致・外は不一致 |
| 103 | ネイティブの参照とは同じで CPU 参照とは違う出力は終了コード 1（`--recount` も 1・部品は `ng`）。両方と違う出力は `mismatches` の詳細 |
| 104 | manifest の不正（鍵の過不足・sha256・長さ・ファイルの欠け・`kind` の書き換え）は終了コード 2 で、Node の本体を起動しない |
| 105 | 保存の形の名前の集合と `--recount` の 0・1（出力の語・CPU 参照のファイルだけの変更）・2 |
| 106 | export とケースの種類の照らし |
| 107 | 実物の形の名前の特殊値の行を足した合成の入力が、項目 22 の確かめを直さずに通る |
| 108 | 参照の出どころが使えない行は `not-run (reference-unavailable)` |
| 109 | 変えていない `wasm/node/run-glue.cjs` に特殊値の種類の manifest を当て、呼び方・引数・ビット列・番兵を本物の node で確かめる |
| 110 | 数え直しの側の CPU 参照の比べだけを外した変異体が `recount disagrees` で止まり、何も書かない |

#### 確かめ方

```bash
make test-wasm-node-check 2>&1 | tail -n 1
grep -n 'def cpu_count\|"gemm-special"\|"elementwise-special"' scripts/wasm-node-check.sh
```

### GEMM の 14 ケースの入力と 2 つの参照（2/6）

`wasm-parity-host gen <dir>` が、既存のケースに続けて特殊値の GEMM の 14 ケースの入力と 2 つの参照（`DefaultCpuOps.gemm_scalar` の出力と、core の `CaseId::reference()` の CPU 参照）を書く。`wasm/node/cases.json` は変わらず、行は足さない。

#### ケース

定数 `GEMM_SPECIAL_CASES`（`wasm/crates/parity-host/src/lib.rs`）が持つ。入力は core の `CaseId::input()` の GEMM の入力をそのまま使い、順は `CaseId::ALL` のうち GEMM のもの。

| 名前 | `CaseId` | `m`・`k`・`n` |
| ---- | -------- | ------------- |
| `special-gemm-g-01`〜`special-gemm-g-09` | `G01`〜`G09` | 1・4・1 |
| `special-gemm-g-10` | `G10` | 1・1・1 |
| `special-gemm-g-11` | `G11` | 1・4・1 |
| `special-gemm-g-12` | `G12` | 37・53・41 |
| `special-gemm-g-13` | `G13` | 1・4・1 |
| `special-gemm-e-1` | `E1` | 37・53・41 |

2 つの参照は、`native_scalar` が同じ入力を `DefaultCpuOps.gemm_scalar` に渡した出力、`gemm_special_cpu_reference` が `CaseId::reference()` の `as_f32()` を返したもの。

#### 書くファイル

| ファイル | 中身 |
| -------- | ---- |
| `<ケース>.a.bin`・`<ケース>.b.bin` | 入力の f32 を添字の順にリトルエンディアンで並べたもの |
| `<ケース>.native-scalar.bin` | `DefaultCpuOps.gemm_scalar` の出力の f32 を同じ形で並べたもの |
| `<ケース>.cpu-reference.bin` | CPU 参照の f32 を同じ形で並べたもの |
| `manifest.json` の `gemm-special` の要素 | `name`・`kind`（`"gemm-special"`）・`m`・`k`・`n`・`a_file`・`b_file`・`native_scalar_file`・`cpu_reference_file`・`a_sha256`・`b_sha256`・`native_scalar_sha256`・`cpu_reference_sha256` の 13 鍵を 1 欄 1 行で持つ |

今の値として、`manifest.json` の `cases` は既存の 13 ケースの後に 14 ケースを並べた 27 要素で、書く名前は 86 個（`.bin` が 85 個と `manifest.json`）。CPU 参照が `Err` か f32 でない出力のときは固定の文言 `error: cpu reference failed` を標準エラーに出し、終了コードは 2。

#### 試験

`wasm/crates/parity-host/tests/gen.rs` の末尾の節。期待は core の呼び出しで作り、値を手で写さない。

| 試験 | 確かめること |
| ---- | ------------ |
| `gemm_special_cases_match_core_inputs` | 表の `id` の列が `CaseId::ALL` のうち GEMM の入力の全部と順まで同じ。名前が `special-gemm-<識別名>`。14 個 |
| `gemm_special_files_hold_core_inputs` | `.a.bin`・`.b.bin` が core の入力とビット列で同じ（14 ケース） |
| `gemm_special_cpu_reference_matches_core` | `.cpu-reference.bin` が `CaseId::reference()` の出力とビット列で同じ |
| `gemm_special_native_matches_default_cpu_ops` | `.native-scalar.bin` が `DefaultCpuOps.gemm_scalar` の出力とビット列で同じ |
| `gemm_special_outputs_follow_design` | 2 つの参照を `classify` した列がどちらも core の `design()` と同じ |
| `gemm_special_inputs_hold_special_values` | 14 ケースの入力の全語で、NaN・`+Inf`・`-Inf`・`+0`・`-0` の件数がどれも 0 より大きく、core の入力から数えた件数と同じ |
| `gemm_special_native_agrees_with_cpu_reference_by_rule` | 2 つの参照が `compare_f32_with_special_values`（`T1`）で一致 |
| `gemm_special_manifest_entries_match_files` | 鍵がちょうど 13 個。`kind`・`m`・`k`・`n`・ファイル名の文字種・ファイルの長さ・sha256 が合う |
| `gen_refuses_existing_gemm_special_file` | `special-gemm-g-01.a.bin` が先にあると `FileExists`。ディレクトリの中身はそのファイルだけで、中身も変わらない |
| `cpu_reference_error_text` | `GenError::CpuReferenceFailed` の文言が固定 |
| `gemm_special_case_rejects_non_gemm_input` | GEMM でない `CaseId`（`E2`）の行が `InputFailed` |
| `gemm_special_cpu_reference_rejects_non_f32_output` | f32 でない出力の `CaseId`（`E3`）の行が `CpuReferenceFailed` |

既存の `manifest_hashes_match_files`・`topk_manifest_entries_match_files`・`elementwise_manifest_entries_match_files`・`signed_zero_manifest_entries_match_files` の件数の期待は、特殊値の GEMM のケースの数を足した和。

#### 確かめ方

時点のコミット: `81062ff`（この変更の土台）。

```console
$ cargo test --manifest-path wasm/Cargo.toml --workspace --locked 2>&1 | grep '^test result:'
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 48 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.76s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.72s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

一時のディレクトリ（`d` は `mktemp -d`）に当てた出力。

```console
$ cargo run -q --manifest-path wasm/Cargo.toml -p wasm-parity-host --locked -- gen "$d"
$ ls "$d" | wc -l
86
$ grep -c '"name"' "$d/manifest.json"
27
```

```bash
grep -n 'GEMM_SPECIAL_CASES\|fn gemm_special_case_output\|CpuReferenceFailed' wasm/crates/parity-host/src/lib.rs
```

### 要素ごと演算と部分 Top-k のケース（3/6）

`wasm-parity-host gen <dir>` が、既存のケースに続けて特殊値の要素ごと演算の 2 ケース（2 つの参照つき）と部分 Top-k の `topk-e-3` の 1 ケースを書く。`wasm/node/cases.json` は変わらず、行は足さない。

#### ケース

定数 `ELEMENTWISE_SPECIAL_CASES` と `TOPK_E3_CASES`（どちらも `wasm/crates/parity-host/src/lib.rs`）が持つ。入力は core の `CaseId::input()` をそのまま使う。

| 名前 | `CaseId` | 演算 | `op_code` | `c_bits` | `n`（`k`） |
| ---- | -------- | ---- | --------- | -------- | ---------- |
| `special-elementwise-relu-e-2` | `CaseId::E2` | `Relu` | 0 | `00000000` | 37 |
| `special-elementwise-add-scalar-e-2` | `CaseId::E2` | `AddScalar(0.5)` | 1 | `0.5_f32.to_bits()` | 37 |
| `topk-e-3` | `CaseId::E3` | 部分 Top-k（`kind` は `topk`） | | | 37（`k` は 5） |

特殊値の要素ごと演算の参照は 2 つで、`native_elementwise` が同じ入力を `DefaultCpuOps.elementwise_scalar` に渡した出力、`elementwise_special_cpu_reference` が返す CPU 参照。`Relu` は `CaseId::reference()` の `as_f32()`、`AddScalar(c)` は入力の各要素に `c` を足した列。CPU 参照が `Err` か f32 でない出力のときは `CpuReferenceFailed`。`topk-e-3` は既存の `topk_case_output` で書き、記録は `DefaultCpuOps.topk_partial_scalar` の出力。

#### 書くファイル

| ファイル | 中身 |
| -------- | ---- |
| `<ケース>.input.bin` | 入力の f32 を添字の順にリトルエンディアンで並べたもの（特殊値の要素ごと演算。2 ケースで同じバイト列） |
| `<ケース>.native-scalar.bin` | `DefaultCpuOps.elementwise_scalar` の出力の f32 を同じ形で並べたもの |
| `<ケース>.cpu-reference.bin` | CPU 参照の f32 を同じ形で並べたもの |
| `topk-e-3.scores.bin`・`topk-e-3.native-scalar.bin` | 既存の部分 Top-k と同じ形（スコアの f32、1 件 8 バイトの記録） |
| `manifest.json` の `elementwise-special` の要素 | `name`・`kind`（`"elementwise-special"`）・`n`・`op_code`・`c_bits`・`input_file`・`native_scalar_file`・`cpu_reference_file`・`input_sha256`・`native_scalar_sha256`・`cpu_reference_sha256` の 11 鍵を 1 欄 1 行で持つ |
| `manifest.json` の `topk-e-3` の要素 | 既存の `topk` の要素と同じ 8 鍵 |

今の値として、`manifest.json` の `cases` は 2/6 までの 27 要素の後に 3 要素を並べた 30 要素で、書く名前は 94 個（`.bin` が 93 個と `manifest.json`）。

#### 試験

`wasm/crates/parity-host/tests/gen.rs` の末尾の節。期待は core の呼び出しと式で作り、値を手で写さない。

| 試験 | 確かめること |
| ---- | ------------ |
| `elementwise_special_cases_hold_core_input` | 2 ケースの `.input.bin` が `CaseId::E2` の入力とビット列で同じで、2 ファイルのバイト列が等しい |
| `elementwise_special_input_holds_special_values` | 入力の NaN・`+Inf`・`-Inf`・`+0`・`-0` の件数がどれも 0 より大きく、core の入力から数えた件数と同じ |
| `elementwise_special_native_matches_default_cpu_ops` | `.native-scalar.bin` が `DefaultCpuOps.elementwise_scalar` の出力とビット列で同じ |
| `elementwise_special_relu_cpu_reference_matches_core` | `Relu` の `.cpu-reference.bin` が `CaseId::reference()` の出力とビット列で同じ |
| `elementwise_special_add_scalar_cpu_reference_matches_expression` | `AddScalar` の `.cpu-reference.bin` が入力の `x + 0.5` の列とビット列で同じ |
| `elementwise_special_relu_maps_nan_to_zero` | `Relu` で入力が NaN の位置の 2 つの参照が `0.0` で、参照に NaN が無い |
| `elementwise_special_native_agrees_with_cpu_reference_by_rule` | 2 ケースとも 2 つの参照が `compare_f32_with_special_values`（`T1`）で一致 |
| `elementwise_special_manifest_entries_match_files` | 鍵がちょうど 11 個。`kind`・`n`・`op_code`・`c_bits`・ファイル名の文字種・ファイルの長さ・sha256 が合う |
| `gen_refuses_existing_elementwise_special_file` | `special-elementwise-relu-e-2.input.bin` が先にあると `FileExists`。ディレクトリの中身はそのファイルだけで、中身も変わらない |
| `elementwise_special_case_rejects_non_relu_input` | `Relu` の入力でない `CaseId`（`E3`）の行が `InputFailed` |
| `elementwise_special_cpu_reference_rejects_non_f32_output` | f32 でない出力の `CaseId`（`E3`）の `Relu` の行が `CpuReferenceFailed` |
| `topk_e3_case_holds_core_input` | `.scores.bin` と `k` が `CaseId::E3` の入力と同じ |
| `topk_e3_native_matches_core` | 記録が `CaseId::reference()` の出力と `compare_topk` で一致し、比べた件数が `k` |
| `topk_e3_manifest_entry_matches_files` | `kind` が `topk`、鍵がちょうど 8 個。`n`・`k`・ファイルの長さ・sha256 が合う |
| `topk_e3_case_rejects_non_topk_input` | 部分 Top-k の入力でない `CaseId`（`E2`）の行が `InputFailed` |

既存の `manifest_hashes_match_files`・`topk_manifest_entries_match_files`・`elementwise_manifest_entries_match_files`・`signed_zero_manifest_entries_match_files`・`gemm_special_manifest_entries_match_files` の件数の期待は、特殊値の要素ごと演算のケースの数と `topk-e-3` のケースの数を足した和。

#### 確かめ方

時点のコミット: `469e592`（この変更の土台）。

```console
$ cargo test --manifest-path wasm/Cargo.toml --workspace --locked 2>&1 | grep '^test result:'
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 63 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.60s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.72s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

一時のディレクトリ（`d` は `mktemp -d`）に当てた出力。

```console
$ cargo run -q --manifest-path wasm/Cargo.toml -p wasm-parity-host --locked -- gen "$d"
$ ls "$d" | wc -l
94
$ grep -c '"name"' "$d/manifest.json"
30
```

```bash
grep -n 'ELEMENTWISE_SPECIAL_CASES\|TOPK_E3_CASES\|fn elementwise_special_case_output\|fn elementwise_special_cpu_reference' wasm/crates/parity-host/src/lib.rs
```

### 特殊値の入力の比べと共通の道具の突き合わせ（4/6）

`wasm/crates/parity-host/tests/special_compare.rs`（TASK-999）は、スクリプトの `--recount` の `recount:`・`recount-cpu:` の件数を、共通の道具（`T1`・`compare_f32_with_special_values`）の件数と突き合わせる。試験は保存の形を一時ディレクトリに書き、`bash scripts/wasm-node-check.sh --recount <dir>` を流す。python3 が無いときは試験が落ちる。

| 試験 | 何と何を比べるか |
| ---- | ---------------- |
| `recount_agrees_with_core_on_special_words` | `gemm-special`・`elementwise-special` の保存の形に、NaN の payload 違い・符号違い・±0 違い・片方だけ NaN・Inf の符号違い・閾値の内外の語を置く。ネイティブ側と CPU 参照側に別々に置いた場合と、全部の語を並べた列、両方が 0 でなく件数が違う列で、4 つの `(export, build)` の組の `recount:`・`recount-cpu:` の `mismatches`・終了コード・`recount-summary:` を、共通の道具の件数と比べる |
| `recount_agrees_with_core_on_generated_special_files` | `generate` が書いた全部の `special-*` のケースのファイルから保存の形を作り（ネイティブの出力を行の出力に置く）、終了コード 0・`recount-summary:` の `nonzero=0`・`recount-cpu:` の `mismatches` が共通の道具の件数（0）と等しいことを確かめる。ケース名の集合は `GEMM_SPECIAL_CASES`・`ELEMENTWISE_SPECIAL_CASES` とちょうど等しい |
| `existing_kinds_stay_bit_exact` | 規則では等しくビットが違う語を既存の `gemm`（`gemm_scalar` の 2 行）・`elementwise`（4 行）に当てると、`recount:` の `mismatches` が語の数と等しく終了コード 1。`recount-cpu:`・`t1:` の行は出ない。同じ語を特殊値の 2 種類に当てると `mismatches=0`・終了コード 0 |

#### 確かめ方

```console
$ cargo test --manifest-path wasm/Cargo.toml -p wasm-parity-host --test special_compare --locked
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.48s
$ cargo test --manifest-path wasm/Cargo.toml -p wasm-parity-host --test special_compare --locked -- --list
existing_kinds_stay_bit_exact: test
recount_agrees_with_core_on_generated_special_files: test
recount_agrees_with_core_on_special_words: test
```

時点のコミット: `bc51973`（この変更の土台）。

### 特殊値の入力の 68 行（5/6）

`wasm/node/cases.json` の既定の入力に足した生成の行。並びは今までの行の後に、ケースの順・組の順。ケースごとに 4 組（`scalar`・`simd128` のビルドと、2 つの export の組）。

| ケース | ビルド | export | 参照の出どころ |
| ------ | ------ | ------ | -------------- |
| `special-gemm-g-01` | `scalar`・`simd128` | `gemm_scalar`・`gemm_simd` | `native-scalar` |
| `special-gemm-g-02` | `scalar`・`simd128` | `gemm_scalar`・`gemm_simd` | `native-scalar` |
| `special-gemm-g-03` | `scalar`・`simd128` | `gemm_scalar`・`gemm_simd` | `native-scalar` |
| `special-gemm-g-04` | `scalar`・`simd128` | `gemm_scalar`・`gemm_simd` | `native-scalar` |
| `special-gemm-g-05` | `scalar`・`simd128` | `gemm_scalar`・`gemm_simd` | `native-scalar` |
| `special-gemm-g-06` | `scalar`・`simd128` | `gemm_scalar`・`gemm_simd` | `native-scalar` |
| `special-gemm-g-07` | `scalar`・`simd128` | `gemm_scalar`・`gemm_simd` | `native-scalar` |
| `special-gemm-g-08` | `scalar`・`simd128` | `gemm_scalar`・`gemm_simd` | `native-scalar` |
| `special-gemm-g-09` | `scalar`・`simd128` | `gemm_scalar`・`gemm_simd` | `native-scalar` |
| `special-gemm-g-10` | `scalar`・`simd128` | `gemm_scalar`・`gemm_simd` | `native-scalar` |
| `special-gemm-g-11` | `scalar`・`simd128` | `gemm_scalar`・`gemm_simd` | `native-scalar` |
| `special-gemm-g-12` | `scalar`・`simd128` | `gemm_scalar`・`gemm_simd` | `native-scalar` |
| `special-gemm-g-13` | `scalar`・`simd128` | `gemm_scalar`・`gemm_simd` | `native-scalar` |
| `special-gemm-e-1` | `scalar`・`simd128` | `gemm_scalar`・`gemm_simd` | `native-scalar` |
| `special-elementwise-relu-e-2` | `scalar`・`simd128` | `elementwise_scalar`・`elementwise_simd` | `native-scalar` |
| `special-elementwise-add-scalar-e-2` | `scalar`・`simd128` | `elementwise_scalar`・`elementwise_simd` | `native-scalar` |
| `topk-e-3` | `scalar`・`simd128` | `topk_scalar`・`topk_simd` | `native-scalar` |

GEMM の 14 ケース × 4 組、要素ごと演算の 2 ケース × 4 組、部分 Top-k の 1 ケース × 4 組。

#### 確かめ方

```console
$ grep -c '"case": "special-gemm-' wasm/node/cases.json
56
$ grep -c '"case": "special-elementwise-' wasm/node/cases.json
8
$ grep -c '"case": "topk-e-3"' wasm/node/cases.json
4
$ make test-wasm-node-check 2>&1 | tail -n 1
passed: 642, failed: 0
```

時点のコミット: `44834c2`（この変更の土台）。

### 完了の条件の確かめ（6/6）

本リポで観測できる事実を、コマンドと出力だけで記録する（TASK-999）。

時点のコミット: `3a3dff1`（この変更の土台）。(2)〜(9) は、5/6 のマージ `3a3dff1` の main への push で流れた CI の run `42158595604` から取る（event `push`・conclusion `success`・headSha は `3a3dff188bea14d1d3f7cc37e9d68d3adee6e90e`）。

| # | 確かめ | コマンド | 結果 |
| - | ------ | -------- | ---- |
| (1) | 入力と 2 つの参照の作り方の試験 | `cargo test --manifest-path wasm/Cargo.toml --workspace --locked -- --list` と同じコマンドの `test result:` の行 | 試験の名前 27 行と `test result:` の 7 行（下のコードブロック） |
| (2) | GEMM の比べ | `gh run view 42158595604 --job 127698571460 --log` の `make wasm-node-check` の段 | ジョブ `wasm32 build + size + node (macos-latest)` は `success`。`node:`・`wasm-bindgen:`・`summary:` の行と、`special-gemm-` の `row:`・`count:`・`cpu-count:` の 168 行の計 171 行（下のコードブロック） |
| (3) | 要素ごと演算の比べ | 同じログの `make wasm-node-check` の段 | `special-elementwise-` の `row:`・`count:`・`cpu-count:` の 24 行と `input-special:` の 2 行の計 26 行（下のコードブロック） |
| (4) | 部分 Top-k の比べ | 同じログの `make wasm-node-check` の段 | `topk-e-3`・`topk-e-4`・`topk-signed-zero-` の `row:`・`count:` の 48 行と `nonfinite:` の 6 行の計 54 行（下のコードブロック） |
| (5) | 語の比べの値 | 同じログの `make wasm-node-check` の段 | `t1:` 64 行（下のコードブロック） |
| (6) | 保存した出力からの数え直し | 同じログの `make wasm-node-check` の段 | `recount:` 68 行・`recount-cpu:` 64 行と `recount-summary:` の行の計 133 行（下のコードブロック） |
| (7) | 出力の sha256 | 同じログの `make wasm-node-check` の段 | `sha256:` 132 行（`special-` 126 行・`topk-e-3` 6 行。下のコードブロック） |
| (8) | `wasm-macos` ジョブの環境の行 | 同じジョブの `Set up job`・`Install pinned Node and wasm-bindgen CLI`・`Show tool versions` の段 | 下のコードブロック |
| (9) | 回帰テスト | `gh run view 42158595604 --job 127698570918 --log` の `make test-wasm-node-check` の段と、`wasm-macos` ジョブの `make wasm-verify` の段 | ジョブ `make deny (cargo-deny)` は `success`。`grep` で絞った `ok:` 100 行と最終行 `passed: 642, failed: 0`。`make wasm-verify` の段は試験 3 本が `ok`、`test result:` は 1 行（下のコードブロック） |
| (10) | unsafe・外部の依存の追加 0 | `git diff --stat` と `grep -rn unsafe`（下のコードブロック） | stat は空、`grep` は 0 件 |

(1) の出力。

```console
$ cargo test --manifest-path wasm/Cargo.toml --workspace --locked -- --list 2>&1 | grep -E '^(gemm_special_cases_match_core_inputs|gemm_special_files_hold_core_inputs|gemm_special_cpu_reference_matches_core|gemm_special_native_matches_default_cpu_ops|gemm_special_outputs_follow_design|gemm_special_inputs_hold_special_values|gemm_special_native_agrees_with_cpu_reference_by_rule|gemm_special_manifest_entries_match_files|gen_refuses_existing_gemm_special_file|cpu_reference_error_text|gemm_special_case_rejects_non_gemm_input|gemm_special_cpu_reference_rejects_non_f32_output|elementwise_special_cases_hold_core_input|elementwise_special_input_holds_special_values|elementwise_special_native_matches_default_cpu_ops|elementwise_special_relu_cpu_reference_matches_core|elementwise_special_add_scalar_cpu_reference_matches_expression|elementwise_special_relu_maps_nan_to_zero|elementwise_special_native_agrees_with_cpu_reference_by_rule|elementwise_special_manifest_entries_match_files|gen_refuses_existing_elementwise_special_file|elementwise_special_case_rejects_non_relu_input|elementwise_special_cpu_reference_rejects_non_f32_output|topk_e3_case_holds_core_input|topk_e3_native_matches_core|topk_e3_manifest_entry_matches_files|topk_e3_case_rejects_non_topk_input): '
cpu_reference_error_text: test
elementwise_special_add_scalar_cpu_reference_matches_expression: test
elementwise_special_case_rejects_non_relu_input: test
elementwise_special_cases_hold_core_input: test
elementwise_special_cpu_reference_rejects_non_f32_output: test
elementwise_special_input_holds_special_values: test
elementwise_special_manifest_entries_match_files: test
elementwise_special_native_agrees_with_cpu_reference_by_rule: test
elementwise_special_native_matches_default_cpu_ops: test
elementwise_special_relu_cpu_reference_matches_core: test
elementwise_special_relu_maps_nan_to_zero: test
gemm_special_case_rejects_non_gemm_input: test
gemm_special_cases_match_core_inputs: test
gemm_special_cpu_reference_matches_core: test
gemm_special_cpu_reference_rejects_non_f32_output: test
gemm_special_files_hold_core_inputs: test
gemm_special_inputs_hold_special_values: test
gemm_special_manifest_entries_match_files: test
gemm_special_native_agrees_with_cpu_reference_by_rule: test
gemm_special_native_matches_default_cpu_ops: test
gemm_special_outputs_follow_design: test
gen_refuses_existing_elementwise_special_file: test
gen_refuses_existing_gemm_special_file: test
topk_e3_case_holds_core_input: test
topk_e3_case_rejects_non_topk_input: test
topk_e3_manifest_entry_matches_files: test
topk_e3_native_matches_core: test
$ cargo test --manifest-path wasm/Cargo.toml --workspace --locked 2>&1 | grep '^test result:'
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 63 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.83s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.94s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.79s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

(2) の出力。ログの行は `ジョブ名<TAB>段の名前<TAB>時刻 本文` の形で、段の名前で絞って時刻までを落とす。

```console
$ gh run view 42158595604 --job 127698571460 --log | awk -F'\t' '$2=="make wasm-node-check"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | grep -E '^(node|wasm-bindgen|summary): |^(row|count|cpu-count): special-gemm-'
node: v24.13.0
wasm-bindgen: 0.2.128
row: special-gemm-g-01-scalar-gemm-scalar match
row: special-gemm-g-01-scalar-gemm-simd match
row: special-gemm-g-01-simd128-gemm-scalar match
row: special-gemm-g-01-simd128-gemm-simd match
row: special-gemm-g-02-scalar-gemm-scalar match
row: special-gemm-g-02-scalar-gemm-simd match
row: special-gemm-g-02-simd128-gemm-scalar match
row: special-gemm-g-02-simd128-gemm-simd match
row: special-gemm-g-03-scalar-gemm-scalar match
row: special-gemm-g-03-scalar-gemm-simd match
row: special-gemm-g-03-simd128-gemm-scalar match
row: special-gemm-g-03-simd128-gemm-simd match
row: special-gemm-g-04-scalar-gemm-scalar match
row: special-gemm-g-04-scalar-gemm-simd match
row: special-gemm-g-04-simd128-gemm-scalar match
row: special-gemm-g-04-simd128-gemm-simd match
row: special-gemm-g-05-scalar-gemm-scalar match
row: special-gemm-g-05-scalar-gemm-simd match
row: special-gemm-g-05-simd128-gemm-scalar match
row: special-gemm-g-05-simd128-gemm-simd match
row: special-gemm-g-06-scalar-gemm-scalar match
row: special-gemm-g-06-scalar-gemm-simd match
row: special-gemm-g-06-simd128-gemm-scalar match
row: special-gemm-g-06-simd128-gemm-simd match
row: special-gemm-g-07-scalar-gemm-scalar match
row: special-gemm-g-07-scalar-gemm-simd match
row: special-gemm-g-07-simd128-gemm-scalar match
row: special-gemm-g-07-simd128-gemm-simd match
row: special-gemm-g-08-scalar-gemm-scalar match
row: special-gemm-g-08-scalar-gemm-simd match
row: special-gemm-g-08-simd128-gemm-scalar match
row: special-gemm-g-08-simd128-gemm-simd match
row: special-gemm-g-09-scalar-gemm-scalar match
row: special-gemm-g-09-scalar-gemm-simd match
row: special-gemm-g-09-simd128-gemm-scalar match
row: special-gemm-g-09-simd128-gemm-simd match
row: special-gemm-g-10-scalar-gemm-scalar match
row: special-gemm-g-10-scalar-gemm-simd match
row: special-gemm-g-10-simd128-gemm-scalar match
row: special-gemm-g-10-simd128-gemm-simd match
row: special-gemm-g-11-scalar-gemm-scalar match
row: special-gemm-g-11-scalar-gemm-simd match
row: special-gemm-g-11-simd128-gemm-scalar match
row: special-gemm-g-11-simd128-gemm-simd match
row: special-gemm-g-12-scalar-gemm-scalar match
row: special-gemm-g-12-scalar-gemm-simd match
row: special-gemm-g-12-simd128-gemm-scalar match
row: special-gemm-g-12-simd128-gemm-simd match
row: special-gemm-g-13-scalar-gemm-scalar match
row: special-gemm-g-13-scalar-gemm-simd match
row: special-gemm-g-13-simd128-gemm-scalar match
row: special-gemm-g-13-simd128-gemm-simd match
row: special-gemm-e-1-scalar-gemm-scalar match
row: special-gemm-e-1-scalar-gemm-simd match
row: special-gemm-e-1-simd128-gemm-scalar match
row: special-gemm-e-1-simd128-gemm-simd match
count: special-gemm-g-01-scalar-gemm-scalar mismatches=0 n=1
cpu-count: special-gemm-g-01-scalar-gemm-scalar mismatches=0 n=1
count: special-gemm-g-01-scalar-gemm-simd mismatches=0 n=1
cpu-count: special-gemm-g-01-scalar-gemm-simd mismatches=0 n=1
count: special-gemm-g-01-simd128-gemm-scalar mismatches=0 n=1
cpu-count: special-gemm-g-01-simd128-gemm-scalar mismatches=0 n=1
count: special-gemm-g-01-simd128-gemm-simd mismatches=0 n=1
cpu-count: special-gemm-g-01-simd128-gemm-simd mismatches=0 n=1
count: special-gemm-g-02-scalar-gemm-scalar mismatches=0 n=1
cpu-count: special-gemm-g-02-scalar-gemm-scalar mismatches=0 n=1
count: special-gemm-g-02-scalar-gemm-simd mismatches=0 n=1
cpu-count: special-gemm-g-02-scalar-gemm-simd mismatches=0 n=1
count: special-gemm-g-02-simd128-gemm-scalar mismatches=0 n=1
cpu-count: special-gemm-g-02-simd128-gemm-scalar mismatches=0 n=1
count: special-gemm-g-02-simd128-gemm-simd mismatches=0 n=1
cpu-count: special-gemm-g-02-simd128-gemm-simd mismatches=0 n=1
count: special-gemm-g-03-scalar-gemm-scalar mismatches=0 n=1
cpu-count: special-gemm-g-03-scalar-gemm-scalar mismatches=0 n=1
count: special-gemm-g-03-scalar-gemm-simd mismatches=0 n=1
cpu-count: special-gemm-g-03-scalar-gemm-simd mismatches=0 n=1
count: special-gemm-g-03-simd128-gemm-scalar mismatches=0 n=1
cpu-count: special-gemm-g-03-simd128-gemm-scalar mismatches=0 n=1
count: special-gemm-g-03-simd128-gemm-simd mismatches=0 n=1
cpu-count: special-gemm-g-03-simd128-gemm-simd mismatches=0 n=1
count: special-gemm-g-04-scalar-gemm-scalar mismatches=0 n=1
cpu-count: special-gemm-g-04-scalar-gemm-scalar mismatches=0 n=1
count: special-gemm-g-04-scalar-gemm-simd mismatches=0 n=1
cpu-count: special-gemm-g-04-scalar-gemm-simd mismatches=0 n=1
count: special-gemm-g-04-simd128-gemm-scalar mismatches=0 n=1
cpu-count: special-gemm-g-04-simd128-gemm-scalar mismatches=0 n=1
count: special-gemm-g-04-simd128-gemm-simd mismatches=0 n=1
cpu-count: special-gemm-g-04-simd128-gemm-simd mismatches=0 n=1
count: special-gemm-g-05-scalar-gemm-scalar mismatches=0 n=1
cpu-count: special-gemm-g-05-scalar-gemm-scalar mismatches=0 n=1
count: special-gemm-g-05-scalar-gemm-simd mismatches=0 n=1
cpu-count: special-gemm-g-05-scalar-gemm-simd mismatches=0 n=1
count: special-gemm-g-05-simd128-gemm-scalar mismatches=0 n=1
cpu-count: special-gemm-g-05-simd128-gemm-scalar mismatches=0 n=1
count: special-gemm-g-05-simd128-gemm-simd mismatches=0 n=1
cpu-count: special-gemm-g-05-simd128-gemm-simd mismatches=0 n=1
count: special-gemm-g-06-scalar-gemm-scalar mismatches=0 n=1
cpu-count: special-gemm-g-06-scalar-gemm-scalar mismatches=0 n=1
count: special-gemm-g-06-scalar-gemm-simd mismatches=0 n=1
cpu-count: special-gemm-g-06-scalar-gemm-simd mismatches=0 n=1
count: special-gemm-g-06-simd128-gemm-scalar mismatches=0 n=1
cpu-count: special-gemm-g-06-simd128-gemm-scalar mismatches=0 n=1
count: special-gemm-g-06-simd128-gemm-simd mismatches=0 n=1
cpu-count: special-gemm-g-06-simd128-gemm-simd mismatches=0 n=1
count: special-gemm-g-07-scalar-gemm-scalar mismatches=0 n=1
cpu-count: special-gemm-g-07-scalar-gemm-scalar mismatches=0 n=1
count: special-gemm-g-07-scalar-gemm-simd mismatches=0 n=1
cpu-count: special-gemm-g-07-scalar-gemm-simd mismatches=0 n=1
count: special-gemm-g-07-simd128-gemm-scalar mismatches=0 n=1
cpu-count: special-gemm-g-07-simd128-gemm-scalar mismatches=0 n=1
count: special-gemm-g-07-simd128-gemm-simd mismatches=0 n=1
cpu-count: special-gemm-g-07-simd128-gemm-simd mismatches=0 n=1
count: special-gemm-g-08-scalar-gemm-scalar mismatches=0 n=1
cpu-count: special-gemm-g-08-scalar-gemm-scalar mismatches=0 n=1
count: special-gemm-g-08-scalar-gemm-simd mismatches=0 n=1
cpu-count: special-gemm-g-08-scalar-gemm-simd mismatches=0 n=1
count: special-gemm-g-08-simd128-gemm-scalar mismatches=0 n=1
cpu-count: special-gemm-g-08-simd128-gemm-scalar mismatches=0 n=1
count: special-gemm-g-08-simd128-gemm-simd mismatches=0 n=1
cpu-count: special-gemm-g-08-simd128-gemm-simd mismatches=0 n=1
count: special-gemm-g-09-scalar-gemm-scalar mismatches=0 n=1
cpu-count: special-gemm-g-09-scalar-gemm-scalar mismatches=0 n=1
count: special-gemm-g-09-scalar-gemm-simd mismatches=0 n=1
cpu-count: special-gemm-g-09-scalar-gemm-simd mismatches=0 n=1
count: special-gemm-g-09-simd128-gemm-scalar mismatches=0 n=1
cpu-count: special-gemm-g-09-simd128-gemm-scalar mismatches=0 n=1
count: special-gemm-g-09-simd128-gemm-simd mismatches=0 n=1
cpu-count: special-gemm-g-09-simd128-gemm-simd mismatches=0 n=1
count: special-gemm-g-10-scalar-gemm-scalar mismatches=0 n=1
cpu-count: special-gemm-g-10-scalar-gemm-scalar mismatches=0 n=1
count: special-gemm-g-10-scalar-gemm-simd mismatches=0 n=1
cpu-count: special-gemm-g-10-scalar-gemm-simd mismatches=0 n=1
count: special-gemm-g-10-simd128-gemm-scalar mismatches=0 n=1
cpu-count: special-gemm-g-10-simd128-gemm-scalar mismatches=0 n=1
count: special-gemm-g-10-simd128-gemm-simd mismatches=0 n=1
cpu-count: special-gemm-g-10-simd128-gemm-simd mismatches=0 n=1
count: special-gemm-g-11-scalar-gemm-scalar mismatches=0 n=1
cpu-count: special-gemm-g-11-scalar-gemm-scalar mismatches=0 n=1
count: special-gemm-g-11-scalar-gemm-simd mismatches=0 n=1
cpu-count: special-gemm-g-11-scalar-gemm-simd mismatches=0 n=1
count: special-gemm-g-11-simd128-gemm-scalar mismatches=0 n=1
cpu-count: special-gemm-g-11-simd128-gemm-scalar mismatches=0 n=1
count: special-gemm-g-11-simd128-gemm-simd mismatches=0 n=1
cpu-count: special-gemm-g-11-simd128-gemm-simd mismatches=0 n=1
count: special-gemm-g-12-scalar-gemm-scalar mismatches=0 n=1517
cpu-count: special-gemm-g-12-scalar-gemm-scalar mismatches=0 n=1517
count: special-gemm-g-12-scalar-gemm-simd mismatches=0 n=1517
cpu-count: special-gemm-g-12-scalar-gemm-simd mismatches=0 n=1517
count: special-gemm-g-12-simd128-gemm-scalar mismatches=0 n=1517
cpu-count: special-gemm-g-12-simd128-gemm-scalar mismatches=0 n=1517
count: special-gemm-g-12-simd128-gemm-simd mismatches=0 n=1517
cpu-count: special-gemm-g-12-simd128-gemm-simd mismatches=0 n=1517
count: special-gemm-g-13-scalar-gemm-scalar mismatches=0 n=1
cpu-count: special-gemm-g-13-scalar-gemm-scalar mismatches=0 n=1
count: special-gemm-g-13-scalar-gemm-simd mismatches=0 n=1
cpu-count: special-gemm-g-13-scalar-gemm-simd mismatches=0 n=1
count: special-gemm-g-13-simd128-gemm-scalar mismatches=0 n=1
cpu-count: special-gemm-g-13-simd128-gemm-scalar mismatches=0 n=1
count: special-gemm-g-13-simd128-gemm-simd mismatches=0 n=1
cpu-count: special-gemm-g-13-simd128-gemm-simd mismatches=0 n=1
count: special-gemm-e-1-scalar-gemm-scalar mismatches=0 n=1517
cpu-count: special-gemm-e-1-scalar-gemm-scalar mismatches=0 n=1517
count: special-gemm-e-1-scalar-gemm-simd mismatches=0 n=1517
cpu-count: special-gemm-e-1-scalar-gemm-simd mismatches=0 n=1517
count: special-gemm-e-1-simd128-gemm-scalar mismatches=0 n=1517
cpu-count: special-gemm-e-1-simd128-gemm-scalar mismatches=0 n=1517
count: special-gemm-e-1-simd128-gemm-simd mismatches=0 n=1517
cpu-count: special-gemm-e-1-simd128-gemm-simd mismatches=0 n=1517
summary: rows=123 match=123 mismatch=0 not-run=0
```

(3) の出力（(2) と同じ前段に `grep` をつなぐ）。

```console
$ ... | grep -E '^(row|count|cpu-count): special-elementwise-|^input-special: special-elementwise-'
row: special-elementwise-relu-e-2-scalar-elementwise-scalar match
row: special-elementwise-relu-e-2-scalar-elementwise-simd match
row: special-elementwise-relu-e-2-simd128-elementwise-scalar match
row: special-elementwise-relu-e-2-simd128-elementwise-simd match
row: special-elementwise-add-scalar-e-2-scalar-elementwise-scalar match
row: special-elementwise-add-scalar-e-2-scalar-elementwise-simd match
row: special-elementwise-add-scalar-e-2-simd128-elementwise-scalar match
row: special-elementwise-add-scalar-e-2-simd128-elementwise-simd match
count: special-elementwise-relu-e-2-scalar-elementwise-scalar mismatches=0 n=37
cpu-count: special-elementwise-relu-e-2-scalar-elementwise-scalar mismatches=0 n=37
count: special-elementwise-relu-e-2-scalar-elementwise-simd mismatches=0 n=37
cpu-count: special-elementwise-relu-e-2-scalar-elementwise-simd mismatches=0 n=37
count: special-elementwise-relu-e-2-simd128-elementwise-scalar mismatches=0 n=37
cpu-count: special-elementwise-relu-e-2-simd128-elementwise-scalar mismatches=0 n=37
count: special-elementwise-relu-e-2-simd128-elementwise-simd mismatches=0 n=37
cpu-count: special-elementwise-relu-e-2-simd128-elementwise-simd mismatches=0 n=37
count: special-elementwise-add-scalar-e-2-scalar-elementwise-scalar mismatches=0 n=37
cpu-count: special-elementwise-add-scalar-e-2-scalar-elementwise-scalar mismatches=0 n=37
count: special-elementwise-add-scalar-e-2-scalar-elementwise-simd mismatches=0 n=37
cpu-count: special-elementwise-add-scalar-e-2-scalar-elementwise-simd mismatches=0 n=37
count: special-elementwise-add-scalar-e-2-simd128-elementwise-scalar mismatches=0 n=37
cpu-count: special-elementwise-add-scalar-e-2-simd128-elementwise-scalar mismatches=0 n=37
count: special-elementwise-add-scalar-e-2-simd128-elementwise-simd mismatches=0 n=37
cpu-count: special-elementwise-add-scalar-e-2-simd128-elementwise-simd mismatches=0 n=37
input-special: special-elementwise-relu-e-2 nan=7 inf=12 zero=12
input-special: special-elementwise-add-scalar-e-2 nan=7 inf=12 zero=12
```

(4) の出力（同じ前段に `grep` をつなぐ）。

```console
$ ... | grep -E '^(row|count|nonfinite): topk-(e-3|e-4|signed-zero-)'
row: topk-e-4-scalar-topk-scalar match
row: topk-e-4-scalar-topk-simd match
row: topk-e-4-simd128-topk-scalar match
row: topk-e-4-simd128-topk-simd match
row: topk-signed-zero-adjacent-scalar-topk-scalar match
row: topk-signed-zero-adjacent-scalar-topk-simd match
row: topk-signed-zero-adjacent-simd128-topk-scalar match
row: topk-signed-zero-adjacent-simd128-topk-simd match
row: topk-signed-zero-separated-scalar-topk-scalar match
row: topk-signed-zero-separated-scalar-topk-simd match
row: topk-signed-zero-separated-simd128-topk-scalar match
row: topk-signed-zero-separated-simd128-topk-simd match
row: topk-signed-zero-cut-at-k-scalar-topk-scalar match
row: topk-signed-zero-cut-at-k-scalar-topk-simd match
row: topk-signed-zero-cut-at-k-simd128-topk-scalar match
row: topk-signed-zero-cut-at-k-simd128-topk-simd match
row: topk-signed-zero-repeated-scalar-topk-scalar match
row: topk-signed-zero-repeated-scalar-topk-simd match
row: topk-signed-zero-repeated-simd128-topk-scalar match
row: topk-signed-zero-repeated-simd128-topk-simd match
row: topk-e-3-scalar-topk-scalar match
row: topk-e-3-scalar-topk-simd match
row: topk-e-3-simd128-topk-scalar match
row: topk-e-3-simd128-topk-simd match
count: topk-e-4-scalar-topk-scalar mismatches=0 n=8
count: topk-e-4-scalar-topk-simd mismatches=0 n=8
count: topk-e-4-simd128-topk-scalar mismatches=0 n=8
count: topk-e-4-simd128-topk-simd mismatches=0 n=8
count: topk-signed-zero-adjacent-scalar-topk-scalar mismatches=0 n=2
count: topk-signed-zero-adjacent-scalar-topk-simd mismatches=0 n=2
count: topk-signed-zero-adjacent-simd128-topk-scalar mismatches=0 n=2
count: topk-signed-zero-adjacent-simd128-topk-simd mismatches=0 n=2
count: topk-signed-zero-separated-scalar-topk-scalar mismatches=0 n=4
count: topk-signed-zero-separated-scalar-topk-simd mismatches=0 n=4
count: topk-signed-zero-separated-simd128-topk-scalar mismatches=0 n=4
count: topk-signed-zero-separated-simd128-topk-simd mismatches=0 n=4
count: topk-signed-zero-cut-at-k-scalar-topk-scalar mismatches=0 n=2
count: topk-signed-zero-cut-at-k-scalar-topk-simd mismatches=0 n=2
count: topk-signed-zero-cut-at-k-simd128-topk-scalar mismatches=0 n=2
count: topk-signed-zero-cut-at-k-simd128-topk-simd mismatches=0 n=2
count: topk-signed-zero-repeated-scalar-topk-scalar mismatches=0 n=4
count: topk-signed-zero-repeated-scalar-topk-simd mismatches=0 n=4
count: topk-signed-zero-repeated-simd128-topk-scalar mismatches=0 n=4
count: topk-signed-zero-repeated-simd128-topk-simd mismatches=0 n=4
count: topk-e-3-scalar-topk-scalar mismatches=0 n=5
count: topk-e-3-scalar-topk-simd mismatches=0 n=5
count: topk-e-3-simd128-topk-scalar mismatches=0 n=5
count: topk-e-3-simd128-topk-simd mismatches=0 n=5
nonfinite: topk-e-4 input=3 output=0
nonfinite: topk-signed-zero-adjacent input=0 output=0
nonfinite: topk-signed-zero-separated input=0 output=0
nonfinite: topk-signed-zero-cut-at-k input=0 output=0
nonfinite: topk-signed-zero-repeated input=0 output=0
nonfinite: topk-e-3 input=0 output=0
```

(5) の出力（同じ前段に `grep` をつなぐ）。

```console
$ ... | grep -E '^t1: special-'
t1: special-gemm-g-01-scalar-gemm-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-01-scalar-gemm-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-01-simd128-gemm-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-01-simd128-gemm-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-02-scalar-gemm-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-02-scalar-gemm-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-02-simd128-gemm-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-02-simd128-gemm-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-03-scalar-gemm-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-03-scalar-gemm-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-03-simd128-gemm-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-03-simd128-gemm-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-04-scalar-gemm-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-04-scalar-gemm-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-04-simd128-gemm-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-04-simd128-gemm-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-05-scalar-gemm-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-05-scalar-gemm-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-05-simd128-gemm-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-05-simd128-gemm-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-06-scalar-gemm-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-06-scalar-gemm-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-06-simd128-gemm-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-06-simd128-gemm-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-07-scalar-gemm-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-07-scalar-gemm-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-07-simd128-gemm-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-07-simd128-gemm-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-08-scalar-gemm-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-08-scalar-gemm-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-08-simd128-gemm-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-08-simd128-gemm-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-09-scalar-gemm-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-09-scalar-gemm-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-09-simd128-gemm-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-09-simd128-gemm-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-10-scalar-gemm-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-10-scalar-gemm-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-10-simd128-gemm-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-10-simd128-gemm-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-11-scalar-gemm-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-11-scalar-gemm-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-11-simd128-gemm-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-11-simd128-gemm-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-12-scalar-gemm-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-12-scalar-gemm-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-12-simd128-gemm-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-12-simd128-gemm-simd max-abs=2.86102294921875e-06 max-abs-at=377 max-rel=1.7634507203696192e-05 max-rel-at=496 abs-over=0 rel-over=0
t1: special-gemm-g-13-scalar-gemm-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-13-scalar-gemm-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-13-simd128-gemm-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-g-13-simd128-gemm-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-e-1-scalar-gemm-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-e-1-scalar-gemm-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-e-1-simd128-gemm-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-gemm-e-1-simd128-gemm-simd max-abs=2.86102294921875e-06 max-abs-at=45 max-rel=1.7634507203696192e-05 max-rel-at=496 abs-over=0 rel-over=0
t1: special-elementwise-relu-e-2-scalar-elementwise-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-elementwise-relu-e-2-scalar-elementwise-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-elementwise-relu-e-2-simd128-elementwise-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-elementwise-relu-e-2-simd128-elementwise-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-elementwise-add-scalar-e-2-scalar-elementwise-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-elementwise-add-scalar-e-2-scalar-elementwise-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-elementwise-add-scalar-e-2-simd128-elementwise-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-elementwise-add-scalar-e-2-simd128-elementwise-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
```

(6) の出力（同じ前段に `grep` をつなぐ）。

```console
$ ... | grep -E '^(recount|recount-cpu): (special-|topk-e-3-)|^recount-summary: '
recount: special-gemm-g-01-scalar-gemm-scalar mismatches=0 n=1
recount-cpu: special-gemm-g-01-scalar-gemm-scalar mismatches=0 n=1
recount: special-gemm-g-01-scalar-gemm-simd mismatches=0 n=1
recount-cpu: special-gemm-g-01-scalar-gemm-simd mismatches=0 n=1
recount: special-gemm-g-01-simd128-gemm-scalar mismatches=0 n=1
recount-cpu: special-gemm-g-01-simd128-gemm-scalar mismatches=0 n=1
recount: special-gemm-g-01-simd128-gemm-simd mismatches=0 n=1
recount-cpu: special-gemm-g-01-simd128-gemm-simd mismatches=0 n=1
recount: special-gemm-g-02-scalar-gemm-scalar mismatches=0 n=1
recount-cpu: special-gemm-g-02-scalar-gemm-scalar mismatches=0 n=1
recount: special-gemm-g-02-scalar-gemm-simd mismatches=0 n=1
recount-cpu: special-gemm-g-02-scalar-gemm-simd mismatches=0 n=1
recount: special-gemm-g-02-simd128-gemm-scalar mismatches=0 n=1
recount-cpu: special-gemm-g-02-simd128-gemm-scalar mismatches=0 n=1
recount: special-gemm-g-02-simd128-gemm-simd mismatches=0 n=1
recount-cpu: special-gemm-g-02-simd128-gemm-simd mismatches=0 n=1
recount: special-gemm-g-03-scalar-gemm-scalar mismatches=0 n=1
recount-cpu: special-gemm-g-03-scalar-gemm-scalar mismatches=0 n=1
recount: special-gemm-g-03-scalar-gemm-simd mismatches=0 n=1
recount-cpu: special-gemm-g-03-scalar-gemm-simd mismatches=0 n=1
recount: special-gemm-g-03-simd128-gemm-scalar mismatches=0 n=1
recount-cpu: special-gemm-g-03-simd128-gemm-scalar mismatches=0 n=1
recount: special-gemm-g-03-simd128-gemm-simd mismatches=0 n=1
recount-cpu: special-gemm-g-03-simd128-gemm-simd mismatches=0 n=1
recount: special-gemm-g-04-scalar-gemm-scalar mismatches=0 n=1
recount-cpu: special-gemm-g-04-scalar-gemm-scalar mismatches=0 n=1
recount: special-gemm-g-04-scalar-gemm-simd mismatches=0 n=1
recount-cpu: special-gemm-g-04-scalar-gemm-simd mismatches=0 n=1
recount: special-gemm-g-04-simd128-gemm-scalar mismatches=0 n=1
recount-cpu: special-gemm-g-04-simd128-gemm-scalar mismatches=0 n=1
recount: special-gemm-g-04-simd128-gemm-simd mismatches=0 n=1
recount-cpu: special-gemm-g-04-simd128-gemm-simd mismatches=0 n=1
recount: special-gemm-g-05-scalar-gemm-scalar mismatches=0 n=1
recount-cpu: special-gemm-g-05-scalar-gemm-scalar mismatches=0 n=1
recount: special-gemm-g-05-scalar-gemm-simd mismatches=0 n=1
recount-cpu: special-gemm-g-05-scalar-gemm-simd mismatches=0 n=1
recount: special-gemm-g-05-simd128-gemm-scalar mismatches=0 n=1
recount-cpu: special-gemm-g-05-simd128-gemm-scalar mismatches=0 n=1
recount: special-gemm-g-05-simd128-gemm-simd mismatches=0 n=1
recount-cpu: special-gemm-g-05-simd128-gemm-simd mismatches=0 n=1
recount: special-gemm-g-06-scalar-gemm-scalar mismatches=0 n=1
recount-cpu: special-gemm-g-06-scalar-gemm-scalar mismatches=0 n=1
recount: special-gemm-g-06-scalar-gemm-simd mismatches=0 n=1
recount-cpu: special-gemm-g-06-scalar-gemm-simd mismatches=0 n=1
recount: special-gemm-g-06-simd128-gemm-scalar mismatches=0 n=1
recount-cpu: special-gemm-g-06-simd128-gemm-scalar mismatches=0 n=1
recount: special-gemm-g-06-simd128-gemm-simd mismatches=0 n=1
recount-cpu: special-gemm-g-06-simd128-gemm-simd mismatches=0 n=1
recount: special-gemm-g-07-scalar-gemm-scalar mismatches=0 n=1
recount-cpu: special-gemm-g-07-scalar-gemm-scalar mismatches=0 n=1
recount: special-gemm-g-07-scalar-gemm-simd mismatches=0 n=1
recount-cpu: special-gemm-g-07-scalar-gemm-simd mismatches=0 n=1
recount: special-gemm-g-07-simd128-gemm-scalar mismatches=0 n=1
recount-cpu: special-gemm-g-07-simd128-gemm-scalar mismatches=0 n=1
recount: special-gemm-g-07-simd128-gemm-simd mismatches=0 n=1
recount-cpu: special-gemm-g-07-simd128-gemm-simd mismatches=0 n=1
recount: special-gemm-g-08-scalar-gemm-scalar mismatches=0 n=1
recount-cpu: special-gemm-g-08-scalar-gemm-scalar mismatches=0 n=1
recount: special-gemm-g-08-scalar-gemm-simd mismatches=0 n=1
recount-cpu: special-gemm-g-08-scalar-gemm-simd mismatches=0 n=1
recount: special-gemm-g-08-simd128-gemm-scalar mismatches=0 n=1
recount-cpu: special-gemm-g-08-simd128-gemm-scalar mismatches=0 n=1
recount: special-gemm-g-08-simd128-gemm-simd mismatches=0 n=1
recount-cpu: special-gemm-g-08-simd128-gemm-simd mismatches=0 n=1
recount: special-gemm-g-09-scalar-gemm-scalar mismatches=0 n=1
recount-cpu: special-gemm-g-09-scalar-gemm-scalar mismatches=0 n=1
recount: special-gemm-g-09-scalar-gemm-simd mismatches=0 n=1
recount-cpu: special-gemm-g-09-scalar-gemm-simd mismatches=0 n=1
recount: special-gemm-g-09-simd128-gemm-scalar mismatches=0 n=1
recount-cpu: special-gemm-g-09-simd128-gemm-scalar mismatches=0 n=1
recount: special-gemm-g-09-simd128-gemm-simd mismatches=0 n=1
recount-cpu: special-gemm-g-09-simd128-gemm-simd mismatches=0 n=1
recount: special-gemm-g-10-scalar-gemm-scalar mismatches=0 n=1
recount-cpu: special-gemm-g-10-scalar-gemm-scalar mismatches=0 n=1
recount: special-gemm-g-10-scalar-gemm-simd mismatches=0 n=1
recount-cpu: special-gemm-g-10-scalar-gemm-simd mismatches=0 n=1
recount: special-gemm-g-10-simd128-gemm-scalar mismatches=0 n=1
recount-cpu: special-gemm-g-10-simd128-gemm-scalar mismatches=0 n=1
recount: special-gemm-g-10-simd128-gemm-simd mismatches=0 n=1
recount-cpu: special-gemm-g-10-simd128-gemm-simd mismatches=0 n=1
recount: special-gemm-g-11-scalar-gemm-scalar mismatches=0 n=1
recount-cpu: special-gemm-g-11-scalar-gemm-scalar mismatches=0 n=1
recount: special-gemm-g-11-scalar-gemm-simd mismatches=0 n=1
recount-cpu: special-gemm-g-11-scalar-gemm-simd mismatches=0 n=1
recount: special-gemm-g-11-simd128-gemm-scalar mismatches=0 n=1
recount-cpu: special-gemm-g-11-simd128-gemm-scalar mismatches=0 n=1
recount: special-gemm-g-11-simd128-gemm-simd mismatches=0 n=1
recount-cpu: special-gemm-g-11-simd128-gemm-simd mismatches=0 n=1
recount: special-gemm-g-12-scalar-gemm-scalar mismatches=0 n=1517
recount-cpu: special-gemm-g-12-scalar-gemm-scalar mismatches=0 n=1517
recount: special-gemm-g-12-scalar-gemm-simd mismatches=0 n=1517
recount-cpu: special-gemm-g-12-scalar-gemm-simd mismatches=0 n=1517
recount: special-gemm-g-12-simd128-gemm-scalar mismatches=0 n=1517
recount-cpu: special-gemm-g-12-simd128-gemm-scalar mismatches=0 n=1517
recount: special-gemm-g-12-simd128-gemm-simd mismatches=0 n=1517
recount-cpu: special-gemm-g-12-simd128-gemm-simd mismatches=0 n=1517
recount: special-gemm-g-13-scalar-gemm-scalar mismatches=0 n=1
recount-cpu: special-gemm-g-13-scalar-gemm-scalar mismatches=0 n=1
recount: special-gemm-g-13-scalar-gemm-simd mismatches=0 n=1
recount-cpu: special-gemm-g-13-scalar-gemm-simd mismatches=0 n=1
recount: special-gemm-g-13-simd128-gemm-scalar mismatches=0 n=1
recount-cpu: special-gemm-g-13-simd128-gemm-scalar mismatches=0 n=1
recount: special-gemm-g-13-simd128-gemm-simd mismatches=0 n=1
recount-cpu: special-gemm-g-13-simd128-gemm-simd mismatches=0 n=1
recount: special-gemm-e-1-scalar-gemm-scalar mismatches=0 n=1517
recount-cpu: special-gemm-e-1-scalar-gemm-scalar mismatches=0 n=1517
recount: special-gemm-e-1-scalar-gemm-simd mismatches=0 n=1517
recount-cpu: special-gemm-e-1-scalar-gemm-simd mismatches=0 n=1517
recount: special-gemm-e-1-simd128-gemm-scalar mismatches=0 n=1517
recount-cpu: special-gemm-e-1-simd128-gemm-scalar mismatches=0 n=1517
recount: special-gemm-e-1-simd128-gemm-simd mismatches=0 n=1517
recount-cpu: special-gemm-e-1-simd128-gemm-simd mismatches=0 n=1517
recount: special-elementwise-relu-e-2-scalar-elementwise-scalar mismatches=0 n=37
recount-cpu: special-elementwise-relu-e-2-scalar-elementwise-scalar mismatches=0 n=37
recount: special-elementwise-relu-e-2-scalar-elementwise-simd mismatches=0 n=37
recount-cpu: special-elementwise-relu-e-2-scalar-elementwise-simd mismatches=0 n=37
recount: special-elementwise-relu-e-2-simd128-elementwise-scalar mismatches=0 n=37
recount-cpu: special-elementwise-relu-e-2-simd128-elementwise-scalar mismatches=0 n=37
recount: special-elementwise-relu-e-2-simd128-elementwise-simd mismatches=0 n=37
recount-cpu: special-elementwise-relu-e-2-simd128-elementwise-simd mismatches=0 n=37
recount: special-elementwise-add-scalar-e-2-scalar-elementwise-scalar mismatches=0 n=37
recount-cpu: special-elementwise-add-scalar-e-2-scalar-elementwise-scalar mismatches=0 n=37
recount: special-elementwise-add-scalar-e-2-scalar-elementwise-simd mismatches=0 n=37
recount-cpu: special-elementwise-add-scalar-e-2-scalar-elementwise-simd mismatches=0 n=37
recount: special-elementwise-add-scalar-e-2-simd128-elementwise-scalar mismatches=0 n=37
recount-cpu: special-elementwise-add-scalar-e-2-simd128-elementwise-scalar mismatches=0 n=37
recount: special-elementwise-add-scalar-e-2-simd128-elementwise-simd mismatches=0 n=37
recount-cpu: special-elementwise-add-scalar-e-2-simd128-elementwise-simd mismatches=0 n=37
recount: topk-e-3-scalar-topk-scalar mismatches=0 n=5
recount: topk-e-3-scalar-topk-simd mismatches=0 n=5
recount: topk-e-3-simd128-topk-scalar mismatches=0 n=5
recount: topk-e-3-simd128-topk-simd mismatches=0 n=5
recount-summary: rows=118 zero=118 nonzero=0 uncompared=0
```

(7) の出力（同じ前段に `grep` をつなぐ）。

```console
$ ... | grep -E '^sha256: (special-|topk-e-3[-.])'
sha256: special-gemm-g-01-scalar-gemm-scalar ef1eaf26cea96eb18f8fa3137abdf23f52852a855c22ae6f169d21a379dcd739
sha256: special-gemm-g-01-scalar-gemm-simd ef1eaf26cea96eb18f8fa3137abdf23f52852a855c22ae6f169d21a379dcd739
sha256: special-gemm-g-01-simd128-gemm-scalar ef1eaf26cea96eb18f8fa3137abdf23f52852a855c22ae6f169d21a379dcd739
sha256: special-gemm-g-01-simd128-gemm-simd ef1eaf26cea96eb18f8fa3137abdf23f52852a855c22ae6f169d21a379dcd739
sha256: special-gemm-g-02-scalar-gemm-scalar d7173dd8a9e81deded3f2e4719ef149effbd21afd81da9d10ed50b5dff69a708
sha256: special-gemm-g-02-scalar-gemm-simd d7173dd8a9e81deded3f2e4719ef149effbd21afd81da9d10ed50b5dff69a708
sha256: special-gemm-g-02-simd128-gemm-scalar d7173dd8a9e81deded3f2e4719ef149effbd21afd81da9d10ed50b5dff69a708
sha256: special-gemm-g-02-simd128-gemm-simd d7173dd8a9e81deded3f2e4719ef149effbd21afd81da9d10ed50b5dff69a708
sha256: special-gemm-g-03-scalar-gemm-scalar d9265d7acb11b75517ee713afcd928260e65557829bab071f03d67ee93edb8ea
sha256: special-gemm-g-03-scalar-gemm-simd d9265d7acb11b75517ee713afcd928260e65557829bab071f03d67ee93edb8ea
sha256: special-gemm-g-03-simd128-gemm-scalar d9265d7acb11b75517ee713afcd928260e65557829bab071f03d67ee93edb8ea
sha256: special-gemm-g-03-simd128-gemm-simd d9265d7acb11b75517ee713afcd928260e65557829bab071f03d67ee93edb8ea
sha256: special-gemm-g-04-scalar-gemm-scalar d9265d7acb11b75517ee713afcd928260e65557829bab071f03d67ee93edb8ea
sha256: special-gemm-g-04-scalar-gemm-simd d9265d7acb11b75517ee713afcd928260e65557829bab071f03d67ee93edb8ea
sha256: special-gemm-g-04-simd128-gemm-scalar d9265d7acb11b75517ee713afcd928260e65557829bab071f03d67ee93edb8ea
sha256: special-gemm-g-04-simd128-gemm-simd d9265d7acb11b75517ee713afcd928260e65557829bab071f03d67ee93edb8ea
sha256: special-gemm-g-05-scalar-gemm-scalar ef1eaf26cea96eb18f8fa3137abdf23f52852a855c22ae6f169d21a379dcd739
sha256: special-gemm-g-05-scalar-gemm-simd ef1eaf26cea96eb18f8fa3137abdf23f52852a855c22ae6f169d21a379dcd739
sha256: special-gemm-g-05-simd128-gemm-scalar ef1eaf26cea96eb18f8fa3137abdf23f52852a855c22ae6f169d21a379dcd739
sha256: special-gemm-g-05-simd128-gemm-simd ef1eaf26cea96eb18f8fa3137abdf23f52852a855c22ae6f169d21a379dcd739
sha256: special-gemm-g-06-scalar-gemm-scalar ef1eaf26cea96eb18f8fa3137abdf23f52852a855c22ae6f169d21a379dcd739
sha256: special-gemm-g-06-scalar-gemm-simd ef1eaf26cea96eb18f8fa3137abdf23f52852a855c22ae6f169d21a379dcd739
sha256: special-gemm-g-06-simd128-gemm-scalar ef1eaf26cea96eb18f8fa3137abdf23f52852a855c22ae6f169d21a379dcd739
sha256: special-gemm-g-06-simd128-gemm-simd ef1eaf26cea96eb18f8fa3137abdf23f52852a855c22ae6f169d21a379dcd739
sha256: special-gemm-g-07-scalar-gemm-scalar ef1eaf26cea96eb18f8fa3137abdf23f52852a855c22ae6f169d21a379dcd739
sha256: special-gemm-g-07-scalar-gemm-simd ef1eaf26cea96eb18f8fa3137abdf23f52852a855c22ae6f169d21a379dcd739
sha256: special-gemm-g-07-simd128-gemm-scalar ef1eaf26cea96eb18f8fa3137abdf23f52852a855c22ae6f169d21a379dcd739
sha256: special-gemm-g-07-simd128-gemm-simd ef1eaf26cea96eb18f8fa3137abdf23f52852a855c22ae6f169d21a379dcd739
sha256: special-gemm-g-08-scalar-gemm-scalar df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119
sha256: special-gemm-g-08-scalar-gemm-simd df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119
sha256: special-gemm-g-08-simd128-gemm-scalar df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119
sha256: special-gemm-g-08-simd128-gemm-simd df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119
sha256: special-gemm-g-09-scalar-gemm-scalar df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119
sha256: special-gemm-g-09-scalar-gemm-simd df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119
sha256: special-gemm-g-09-simd128-gemm-scalar df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119
sha256: special-gemm-g-09-simd128-gemm-simd df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119
sha256: special-gemm-g-10-scalar-gemm-scalar df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119
sha256: special-gemm-g-10-scalar-gemm-simd df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119
sha256: special-gemm-g-10-simd128-gemm-scalar df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119
sha256: special-gemm-g-10-simd128-gemm-simd df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119
sha256: special-gemm-g-11-scalar-gemm-scalar e00e5eb9444182f352323374ef4e08ebcb784725fdd4fd612d7730540b3e0c8c
sha256: special-gemm-g-11-scalar-gemm-simd e00e5eb9444182f352323374ef4e08ebcb784725fdd4fd612d7730540b3e0c8c
sha256: special-gemm-g-11-simd128-gemm-scalar e00e5eb9444182f352323374ef4e08ebcb784725fdd4fd612d7730540b3e0c8c
sha256: special-gemm-g-11-simd128-gemm-simd e00e5eb9444182f352323374ef4e08ebcb784725fdd4fd612d7730540b3e0c8c
sha256: special-gemm-g-12-scalar-gemm-scalar a2265245b09483aa71cef9f93c06484070b57b1c4d4e54cf38d7363d8f200b23
sha256: special-gemm-g-12-scalar-gemm-simd a2265245b09483aa71cef9f93c06484070b57b1c4d4e54cf38d7363d8f200b23
sha256: special-gemm-g-12-simd128-gemm-scalar a2265245b09483aa71cef9f93c06484070b57b1c4d4e54cf38d7363d8f200b23
sha256: special-gemm-g-12-simd128-gemm-simd d2aec72e106a27f25c8297a501ff10d2268db6c4cf208d0754518b812678cf7c
sha256: special-gemm-g-13-scalar-gemm-scalar ef1eaf26cea96eb18f8fa3137abdf23f52852a855c22ae6f169d21a379dcd739
sha256: special-gemm-g-13-scalar-gemm-simd ef1eaf26cea96eb18f8fa3137abdf23f52852a855c22ae6f169d21a379dcd739
sha256: special-gemm-g-13-simd128-gemm-scalar ef1eaf26cea96eb18f8fa3137abdf23f52852a855c22ae6f169d21a379dcd739
sha256: special-gemm-g-13-simd128-gemm-simd ef1eaf26cea96eb18f8fa3137abdf23f52852a855c22ae6f169d21a379dcd739
sha256: special-gemm-e-1-scalar-gemm-scalar e9f2e2339d1d038df372496a9f7e8668b4f5ae652ceddc10c3fef46db8c60989
sha256: special-gemm-e-1-scalar-gemm-simd e9f2e2339d1d038df372496a9f7e8668b4f5ae652ceddc10c3fef46db8c60989
sha256: special-gemm-e-1-simd128-gemm-scalar e9f2e2339d1d038df372496a9f7e8668b4f5ae652ceddc10c3fef46db8c60989
sha256: special-gemm-e-1-simd128-gemm-simd 1978dbccf014fdbd5e9079d4f61eac7f6b428c2fe50e398fb714d6510bd8cfa4
sha256: special-elementwise-relu-e-2-scalar-elementwise-scalar 036c590c92cf29d207fd7fd7a3717f3f250de87f0a7648a58f998f39409ccf44
sha256: special-elementwise-relu-e-2-scalar-elementwise-simd 036c590c92cf29d207fd7fd7a3717f3f250de87f0a7648a58f998f39409ccf44
sha256: special-elementwise-relu-e-2-simd128-elementwise-scalar 036c590c92cf29d207fd7fd7a3717f3f250de87f0a7648a58f998f39409ccf44
sha256: special-elementwise-relu-e-2-simd128-elementwise-simd 036c590c92cf29d207fd7fd7a3717f3f250de87f0a7648a58f998f39409ccf44
sha256: special-elementwise-add-scalar-e-2-scalar-elementwise-scalar ca3c4c9c481f53fde39f39a569b161c551468080b2d22f570c51b2e222550091
sha256: special-elementwise-add-scalar-e-2-scalar-elementwise-simd ca3c4c9c481f53fde39f39a569b161c551468080b2d22f570c51b2e222550091
sha256: special-elementwise-add-scalar-e-2-simd128-elementwise-scalar ca3c4c9c481f53fde39f39a569b161c551468080b2d22f570c51b2e222550091
sha256: special-elementwise-add-scalar-e-2-simd128-elementwise-simd ca3c4c9c481f53fde39f39a569b161c551468080b2d22f570c51b2e222550091
sha256: topk-e-3-scalar-topk-scalar 7ede4e9d2f20d9c647c8a203ecace7e8d3e6b9f7c65cfd071d7f9bd9ceaf88dd
sha256: topk-e-3-scalar-topk-simd 7ede4e9d2f20d9c647c8a203ecace7e8d3e6b9f7c65cfd071d7f9bd9ceaf88dd
sha256: topk-e-3-simd128-topk-scalar 7ede4e9d2f20d9c647c8a203ecace7e8d3e6b9f7c65cfd071d7f9bd9ceaf88dd
sha256: topk-e-3-simd128-topk-simd 7ede4e9d2f20d9c647c8a203ecace7e8d3e6b9f7c65cfd071d7f9bd9ceaf88dd
sha256: special-gemm-g-01.a 3a4efa941207dbe83e974aea027a06735174f607b015ff3b7ab106e78bcbee4b
sha256: special-gemm-g-01.b f6bb1294da2f78cd935b01c7656280df5eaa0439e9d97bc03775825a41a508e4
sha256: special-gemm-g-01.native-scalar ef1eaf26cea96eb18f8fa3137abdf23f52852a855c22ae6f169d21a379dcd739
sha256: special-gemm-g-01.cpu-reference ef1eaf26cea96eb18f8fa3137abdf23f52852a855c22ae6f169d21a379dcd739
sha256: special-gemm-g-02.a 0f6a80d4c237da1d8cc3cc651044d3e5f85ee71631a76f347a98c7dd09d32f76
sha256: special-gemm-g-02.b f6bb1294da2f78cd935b01c7656280df5eaa0439e9d97bc03775825a41a508e4
sha256: special-gemm-g-02.native-scalar d7173dd8a9e81deded3f2e4719ef149effbd21afd81da9d10ed50b5dff69a708
sha256: special-gemm-g-02.cpu-reference d7173dd8a9e81deded3f2e4719ef149effbd21afd81da9d10ed50b5dff69a708
sha256: special-gemm-g-03.a fe99987b7f7c2dc1f85a9747705ada0b1646b4ce5bb08c51311faec73510ae60
sha256: special-gemm-g-03.b f6bb1294da2f78cd935b01c7656280df5eaa0439e9d97bc03775825a41a508e4
sha256: special-gemm-g-03.native-scalar d9265d7acb11b75517ee713afcd928260e65557829bab071f03d67ee93edb8ea
sha256: special-gemm-g-03.cpu-reference d9265d7acb11b75517ee713afcd928260e65557829bab071f03d67ee93edb8ea
sha256: special-gemm-g-04.a 0f6a80d4c237da1d8cc3cc651044d3e5f85ee71631a76f347a98c7dd09d32f76
sha256: special-gemm-g-04.b c068171f78940ada61be944d828edb826995b62b134cc29d749ee1bf724febff
sha256: special-gemm-g-04.native-scalar d9265d7acb11b75517ee713afcd928260e65557829bab071f03d67ee93edb8ea
sha256: special-gemm-g-04.cpu-reference d9265d7acb11b75517ee713afcd928260e65557829bab071f03d67ee93edb8ea
sha256: special-gemm-g-05.a 33cba49caa2c1f3116ae8ae96532a41c6a26caffe950c0d2866705cae6b8ba21
sha256: special-gemm-g-05.b f6bb1294da2f78cd935b01c7656280df5eaa0439e9d97bc03775825a41a508e4
sha256: special-gemm-g-05.native-scalar ef1eaf26cea96eb18f8fa3137abdf23f52852a855c22ae6f169d21a379dcd739
sha256: special-gemm-g-05.cpu-reference ef1eaf26cea96eb18f8fa3137abdf23f52852a855c22ae6f169d21a379dcd739
sha256: special-gemm-g-06.a 4c9c4f354e74153db012329d71c8562ec23e498148174b2c49de58f45d47cdbe
sha256: special-gemm-g-06.b 039a0e81e9615a4fec52911ee99a4a12485ad75883717aa00f6022b6874757b4
sha256: special-gemm-g-06.native-scalar ef1eaf26cea96eb18f8fa3137abdf23f52852a855c22ae6f169d21a379dcd739
sha256: special-gemm-g-06.cpu-reference ef1eaf26cea96eb18f8fa3137abdf23f52852a855c22ae6f169d21a379dcd739
sha256: special-gemm-g-07.a 81914a623f4dc55b208d08d60f0948da987c6ed3e3bd9a205dbb78f405cebe56
sha256: special-gemm-g-07.b 039a0e81e9615a4fec52911ee99a4a12485ad75883717aa00f6022b6874757b4
sha256: special-gemm-g-07.native-scalar ef1eaf26cea96eb18f8fa3137abdf23f52852a855c22ae6f169d21a379dcd739
sha256: special-gemm-g-07.cpu-reference ef1eaf26cea96eb18f8fa3137abdf23f52852a855c22ae6f169d21a379dcd739
sha256: special-gemm-g-08.a 374708fff7719dd5979ec875d56cd2286f6d3cf7ec317a3b25632aab28ec37bb
sha256: special-gemm-g-08.b ad73b9acd6e4a74b2f5bb5386658ce3bb146cd040a1867646ab3b973fb6632b1
sha256: special-gemm-g-08.native-scalar df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119
sha256: special-gemm-g-08.cpu-reference df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119
sha256: special-gemm-g-09.a df141773470ba96aff5d6e9264a0bd638c9c6f3bb7ad30881cf4db943b5dd021
sha256: special-gemm-g-09.b ad73b9acd6e4a74b2f5bb5386658ce3bb146cd040a1867646ab3b973fb6632b1
sha256: special-gemm-g-09.native-scalar df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119
sha256: special-gemm-g-09.cpu-reference df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119
sha256: special-gemm-g-10.a 6d58692645c9d1cfaf13541cbd258f86193ef63c2f1d38f6bbca9617372d7bd6
sha256: special-gemm-g-10.b e00e5eb9444182f352323374ef4e08ebcb784725fdd4fd612d7730540b3e0c8c
sha256: special-gemm-g-10.native-scalar df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119
sha256: special-gemm-g-10.cpu-reference df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119
sha256: special-gemm-g-11.a 53cf7d9fe502513c35c3834ca4437899d0f93bd72bd6efcf8268800afe0576c4
sha256: special-gemm-g-11.b 9054c63df67b53389612706831644506d7cbc4031c83c31aef0c50a9ebc180d6
sha256: special-gemm-g-11.native-scalar e00e5eb9444182f352323374ef4e08ebcb784725fdd4fd612d7730540b3e0c8c
sha256: special-gemm-g-11.cpu-reference e00e5eb9444182f352323374ef4e08ebcb784725fdd4fd612d7730540b3e0c8c
sha256: special-gemm-g-12.a b37e90e0f9fa3429e0e4cb768382ff6e13ae845b2ce18083cd45c19fc84215eb
sha256: special-gemm-g-12.b a702b6a9402a4df8d0b737383bbb8ae9c78e000d74c0f2fb18a50d95ca97bc19
sha256: special-gemm-g-12.native-scalar a2265245b09483aa71cef9f93c06484070b57b1c4d4e54cf38d7363d8f200b23
sha256: special-gemm-g-12.cpu-reference a2265245b09483aa71cef9f93c06484070b57b1c4d4e54cf38d7363d8f200b23
sha256: special-gemm-g-13.a ad73b9acd6e4a74b2f5bb5386658ce3bb146cd040a1867646ab3b973fb6632b1
sha256: special-gemm-g-13.b 2cfd8740d4e30c6cda1fb1dd08d309cc04717c07e431885022d402aac9af4de4
sha256: special-gemm-g-13.native-scalar ef1eaf26cea96eb18f8fa3137abdf23f52852a855c22ae6f169d21a379dcd739
sha256: special-gemm-g-13.cpu-reference ef1eaf26cea96eb18f8fa3137abdf23f52852a855c22ae6f169d21a379dcd739
sha256: special-gemm-e-1.a a01110f99c2dce87d09c0622ac1bf5715d515ed9750f1df11337a33689ccd875
sha256: special-gemm-e-1.b a702b6a9402a4df8d0b737383bbb8ae9c78e000d74c0f2fb18a50d95ca97bc19
sha256: special-gemm-e-1.native-scalar e9f2e2339d1d038df372496a9f7e8668b4f5ae652ceddc10c3fef46db8c60989
sha256: special-gemm-e-1.cpu-reference e9f2e2339d1d038df372496a9f7e8668b4f5ae652ceddc10c3fef46db8c60989
sha256: special-elementwise-relu-e-2.input f3973b98178c51aecfe234068e78dccd4b0cc17e682d3f1176279297c71d21c6
sha256: special-elementwise-relu-e-2.native-scalar 036c590c92cf29d207fd7fd7a3717f3f250de87f0a7648a58f998f39409ccf44
sha256: special-elementwise-relu-e-2.cpu-reference 036c590c92cf29d207fd7fd7a3717f3f250de87f0a7648a58f998f39409ccf44
sha256: special-elementwise-add-scalar-e-2.input f3973b98178c51aecfe234068e78dccd4b0cc17e682d3f1176279297c71d21c6
sha256: special-elementwise-add-scalar-e-2.native-scalar ca3c4c9c481f53fde39f39a569b161c551468080b2d22f570c51b2e222550091
sha256: special-elementwise-add-scalar-e-2.cpu-reference ca3c4c9c481f53fde39f39a569b161c551468080b2d22f570c51b2e222550091
sha256: topk-e-3.scores 037f422cc160e7f2f7e940b66409641bf5cdb1516202a671366be426f3f23598
sha256: topk-e-3.native-scalar 7ede4e9d2f20d9c647c8a203ecace7e8d3e6b9f7c65cfd071d7f9bd9ceaf88dd
```

(8) の出力。`Set up job` の段は `Operating System` と `Runner Image` の group の行、`Install pinned Node and wasm-bindgen CLI` の段は Node の tar の行、`Show tool versions` の段は末尾の 4 行。

```console
##[group]Operating System
macOS
26.7.2
25G332
##[endgroup]
##[group]Runner Image
Image: macos-26-arm64
Version: 20261102.0473.1
Included Software: https://github.com/actions/runner-images/blob/macos-26-arm64/20261102.0473/images/macos/macos-26-arm64-Readme.md
Image Release: https://github.com/actions/runner-images/releases/tag/macos-26-arm64%2F20261102.0473
##[endgroup]
node-v24.13.0-darwin-arm64.tar.gz: OK
node: v24.13.0
wasm-bindgen: wasm-bindgen 0.2.128
jq-1.8.2
Python 3.15.2
```

(9) の出力。

```console
$ gh run view 42158595604 --job 127698570918 --log | awk -F'\t' '$2=="make test-wasm-node-check"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | grep -E '^ok: (22 bundled (generated|topk|elementwise) rows are exactly|99 |10[0-9] |110 )'
ok: 22 bundled generated rows are exactly 17 cases x (scalar gemm_scalar, scalar gemm_simd, simd128 gemm_scalar, simd128 gemm_simd)
ok: 22 bundled topk rows are exactly the thirty-four (case, export, build) pairs
ok: 22 bundled elementwise rows are exactly the sixteen (case, export, build) pairs
ok: 99 gemm-special all rows match
ok: 99 gemm-special output order
ok: 99 gemm-special no input-special line
ok: 100 gemm-special 5=7fc00001 is within the rule
ok: 100 gemm-special 5=ffc00001 is within the rule
ok: 100 gemm-special 5=7f800001 is within the rule
ok: 100 gemm-special 3=80000000 is within the rule
ok: 100 gemm-special the same kind of change is a mismatch for the existing kind
ok: 101 gemm-special 5=3f800000 is a mismatch
ok: 101 gemm-special 6=ff800000 is a mismatch
ok: 101 gemm-special 4=7fc00000 is a mismatch
ok: 101 gemm-special 7=7f800000 is a mismatch
ok: 102 gemm-special inside the threshold
ok: 102 gemm-special outside the threshold
ok: 103 gemm-special output equal to native but not to the CPU reference
ok: 103 gemm-special --recount of the saved form is 1
ok: 103 gemm-special the record parts are ng and do not carry the detail
ok: 103 gemm-special output different from both references
ok: 104 gemm-special manifest dropkey
ok: 104 gemm-special node body not started (dropkey)
ok: 104 gemm-special manifest dropsha
ok: 104 gemm-special node body not started (dropsha)
ok: 104 gemm-special manifest extrakey
ok: 104 gemm-special node body not started (extrakey)
ok: 104 gemm-special manifest badsha
ok: 104 gemm-special node body not started (badsha)
ok: 104 gemm-special manifest shortfile
ok: 104 gemm-special node body not started (shortfile)
ok: 104 gemm-special manifest rmfile
ok: 104 gemm-special node body not started (rmfile)
ok: 104 gemm-special manifest kind
ok: 104 gemm-special node body not started (kind)
ok: 105 gemm-special --output-dir
ok: 105 gemm-special the saved names include the CPU reference file
ok: 105 gemm-special --recount is 0
ok: 105 gemm-special --recount of a changed output is 1
ok: 105 gemm-special --recount of a changed CPU reference file only is 1
ok: 105 gemm-special --recount of a broken saved form (badcpusha) is 2
ok: 105 gemm-special --recount of a broken saved form (rmcpu) is 2
ok: 105 gemm-special --recount of a broken saved form (export) is 2
ok: 108 gemm-special native-neon rows
ok: 99 elementwise-special all rows match
ok: 99 elementwise-special output order
ok: 99 elementwise-special input-special line
ok: 100 elementwise-special 5=7fc00001 is within the rule
ok: 100 elementwise-special 5=ffc00001 is within the rule
ok: 100 elementwise-special 5=7f800001 is within the rule
ok: 100 elementwise-special 3=80000000 is within the rule
ok: 100 elementwise-special the same kind of change is a mismatch for the existing kind
ok: 101 elementwise-special 5=3f800000 is a mismatch
ok: 101 elementwise-special 6=ff800000 is a mismatch
ok: 101 elementwise-special 4=7fc00000 is a mismatch
ok: 101 elementwise-special 7=7f800000 is a mismatch
ok: 102 elementwise-special inside the threshold
ok: 102 elementwise-special outside the threshold
ok: 103 elementwise-special output equal to native but not to the CPU reference
ok: 103 elementwise-special --recount of the saved form is 1
ok: 103 elementwise-special the record parts are ng and do not carry the detail
ok: 103 elementwise-special output different from both references
ok: 104 elementwise-special manifest dropkey
ok: 104 elementwise-special node body not started (dropkey)
ok: 104 elementwise-special manifest dropsha
ok: 104 elementwise-special node body not started (dropsha)
ok: 104 elementwise-special manifest extrakey
ok: 104 elementwise-special node body not started (extrakey)
ok: 104 elementwise-special manifest badsha
ok: 104 elementwise-special node body not started (badsha)
ok: 104 elementwise-special manifest shortfile
ok: 104 elementwise-special node body not started (shortfile)
ok: 104 elementwise-special manifest rmfile
ok: 104 elementwise-special node body not started (rmfile)
ok: 104 elementwise-special manifest kind
ok: 104 elementwise-special node body not started (kind)
ok: 105 elementwise-special --output-dir
ok: 105 elementwise-special the saved names include the CPU reference file
ok: 105 elementwise-special --recount is 0
ok: 105 elementwise-special --recount of a changed output is 1
ok: 105 elementwise-special --recount of a changed CPU reference file only is 1
ok: 105 elementwise-special --recount of a broken saved form (badcpusha) is 2
ok: 105 elementwise-special --recount of a broken saved form (rmcpu) is 2
ok: 105 elementwise-special --recount of a broken saved form (export) is 2
ok: 108 elementwise-special native-neon rows
ok: 106 both kinds of an export are accepted together
ok: 106 gemm_scalar on special-elementwise-add-a
ok: 106 node body not started (gemm_scalar on special-elementwise-add-a)
ok: 106 elementwise_scalar on special-gemm-a
ok: 106 node body not started (elementwise_scalar on special-gemm-a)
ok: 106 topk_scalar on special-gemm-a
ok: 106 node body not started (topk_scalar on special-gemm-a)
ok: 106 topk_simd on special-elementwise-add-a
ok: 106 node body not started (topk_simd on special-elementwise-add-a)
ok: 109 run-glue special rows
ok: 109 gemm and elementwise special rows write the bit patterns with the fill word kept
ok: 109 nonzero returns are error rows and write nothing
ok: 110 the mutant differs from the original
ok: 110 a disagreeing recount of the CPU reference comparison stops the run
ok: 110 nothing written to either directory
$ gh run view 42158595604 --job 127698570918 --log | awk -F'\t' '$2=="make test-wasm-node-check"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | tail -n 1
passed: 642, failed: 0
$ gh run view 42158595604 --job 127698571460 --log | awk -F'\t' '$2=="make wasm-verify"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | awk '/^running [0-9]+ tests?$/{b="";k=0} /^test (existing_kinds_stay_bit_exact|recount_agrees_with_core_on_generated_special_files|recount_agrees_with_core_on_special_words) \.\.\. /{b=b $0 "\n";k=1} /^test result:/{if(k){printf "%s%s\n",b,$0;exit}}'
test existing_kinds_stay_bit_exact ... ok
test recount_agrees_with_core_on_generated_special_files ... ok
test recount_agrees_with_core_on_special_words ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.41s
```

(10) の出力。1/6 の base は `d60747f`。

```console
$ git diff --stat d60747f..3a3dff1 -- wasm/deps-approved.toml deps-approved.toml Cargo.toml Cargo.lock wasm/Cargo.toml wasm/Cargo.lock crates .github wasm/node/run-glue.cjs wasm/node/cases-reference-unavailable.json
$ grep -rn unsafe wasm/crates/parity-host
```

### 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/6 | Node での照合に特殊値の入力の種類と CPU 参照の比べを足す | #6291 |
| 2/6 | GEMM の 14 ケースの入力と 2 つの参照を host 側の道具に足す | #6292 |
| 3/6 | 要素ごと演算の 2 ケースと部分 Top-k の 1 ケースを host 側の道具に足す | #6293 |
| 4/6 | 特殊値の入力の比べを共通の道具と突き合わせる | #6294 |
| 5/6 | 特殊値の入力の 68 行を照合の入力に足す | #6295 |
| 6/6 | 完了の条件の確かめ | #6296 |

## 今の値の補い（base 0f8b01c）

記録時点の表と既存の節は書き換えない。以下は base `0f8b01c` で観測した今の値（実行環境は GPU なしの Linux x86_64。node・wasm-bindgen の CLI は入れていないので `make wasm-node-check` の実物の実行は流していない）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| `:575` の `grep … def kind_of_export` | `def kind_of_export` は当たらない（関数は消えた）。種類と export の照らしは `load_manifest` の中の `r["export"] not in KINDS[cases[r["case"]]["kind"]]["exports"]` にある。今のコマンドは `def load_manifest` を使う形 | `grep -n "^KINDS\|def nonfinite_lines\|def load_manifest" scripts/wasm-node-check.sh`、`grep -n 'not in KINDS\[cases' scripts/wasm-node-check.sh` |
| `:955` の `grep … def kind_of_export` | 同上。`def kind_of_export` を `def load_manifest` に替え、`def input_special_lines` は残る | `grep -n "^KINDS\|def input_special_lines\|def load_manifest" scripts/wasm-node-check.sh` |
| 「今の値の補い（base e416527）」の語ごとの比べ（`:1604`） | `gemm` の `gemm_simd`・`simd128` の行だけ閾値で語ごとに比べる → `gemm-special`・`elementwise-special` の 4 組（種類 2 つ × `scalar`・`simd128` の組で表 `KINDS` の `t1` の欄）も閾値で比べ、`t1:` の行を出す。さらに `KINDS` の `cpu` の欄を持つ種類は wasm の出力を CPU 参照とも閾値で比べ、不一致があれば行は mismatch（`cpu-count:`・`recount-cpu:` の行を出す） | `sed -n 28,40p scripts/wasm-node-check.sh`、`grep -n '"t1"\|"cpu"' scripts/wasm-node-check.sh` |
| 確かめ方の `grep -c '"case"' wasm/node/cases.json`（`:1605`） | `50` → `118` | `grep -c '"case"' wasm/node/cases.json` |
| `make test-wasm-node-check` の最終行（`:1607`） | `passed: 544, failed: 0` → `passed: 642, failed: 0` | `make test-wasm-node-check 2>&1 \| tail -n 1` |
| `wasm/node/cases.json` の行数（`:1608`） | 55 行 → 123 行（`gemm_scalar` 34・`gemm_simd` 34・`topk_scalar` 17・`topk_simd` 17・`roundtrip_f32` 3・`detect_isa_bits` 2・`elementwise_scalar` 8・`elementwise_simd` 8） | `jq '.rows \| length' wasm/node/cases.json`、`jq -r '.rows[].export' wasm/node/cases.json \| sort \| uniq -c` |
| `manifest.json` の `cases` と書く名前（`:1609`） | 13 要素（GEMM 3・部分 Top-k 8・要素ごと演算 2）・書く名前は 30 個 → 30 要素（`gemm` 3・`topk` 9・`elementwise` 2・`gemm-special` 14・`elementwise-special` 2）・書く名前は 94 個 | `d=$(mktemp -d); (cd wasm && cargo run -q -p wasm-parity-host -- gen "$d"); ls "$d" \| wc -l; jq -r '.cases[].kind' "$d/manifest.json" \| sort \| uniq -c` |
| `sha256sum *` の `manifest.json`（`:1610`） | `fae32ffa06473452882cfb5c5991c0ac2ed0af3807b09beee867b50ba5c3f987` → `e2351cce5ecb46c3dfb8a116d36eff8be43e9de3160283207823d5dc327894c1`（x86_64 の Linux で測った値）。x86_64 と arm64 で NaN の符号が違う語を含むケースがあり、`manifest.json` の sha256 は環境で異なる | 上の `gen` を流し、`(cd "$d" && sha256sum manifest.json)`、`uname -m` |
| 回帰テストの項目 22 の名前（`:1611`） | `3 cases x` → `17 cases x`（`22 bundled generated rows are exactly 17 cases x (…)`）。`the thirty` → `the thirty-four`、`the eight` → `the sixteen`（`22 bundled topk rows are exactly the thirty-four (case, export, build) pairs`・`22 bundled elementwise rows are exactly the sixteen (case, export, build) pairs`） | `grep -n 'bundled .* rows' scripts/test-wasm-node-check.sh` |
| `wasm-parity-host` の `tests/gen.rs` の `test result:`（`:1612`） | `36 passed` → `64 passed` | `cargo test --manifest-path wasm/Cargo.toml --workspace --locked 2>&1 \| grep '^test result:'` |
| `summary:` の行（`:1613`） | `rows=55` → `rows=123` | `jq '.rows \| length' wasm/node/cases.json` |
| `recount-summary:` の行（`:1614`） | `rows=50` → `rows=118`（`cases.json` の 123 行から `detect_isa_bits` 2 行・`roundtrip_f32` 3 行を除く） | `jq -r '.rows[].export' wasm/node/cases.json \| sort \| uniq -c` |

## 今の値の補い（base fbbd70d）

記録時点の表と既存の節は書き換えない。以下は base `fbbd70d` で観測した今の値（`summary:`・`recount-summary:` の行は main の push の CI の run 42220227269 の `wasm32 build + size + node (macos-latest)` のログ。ほかの値の実行環境は GPU なしの Linux x86_64 で、node・wasm-bindgen の CLI は入れていないので `make wasm-node-check` の実物の実行は流していない）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| `--recount` の口の表（`:261`） | 口が増えた。`--recount <dir> --expect-cases <path>` は、入力の JSON（`--cases` と同じ形）の `native-scalar` の生成の行と欄 `cases` を期待として、保存の形の行の集合（id・export・build・case）と `manifest.json` のケースの集合・種類・寸法を比べ、違えば終了コード 2。`--expect-cases` を渡さない `--recount` の動きは変わらず、保存の形の中の値だけで数える | `sed -n 21,24p scripts/wasm-node-check.sh` |
| `wasm/node/cases.json` の形（`:2746` の行） | 欄 `rows` のほかに欄 `cases` が増えた。ケースごとの `name`・`kind`・`dims` を持ち、実行の中の判定はこの欄を期待に使う（`manifest.json` のケースと違えば終了コード 2）。33 要素で、`gemm-64`・`gemm-256`・`gemm-37x41x53` の 3 要素は `simd_sum_order_differs: true` を持つ | `jq '.cases \| length' wasm/node/cases.json`、`jq -r '.cases[] \| select(.simd_sum_order_differs) \| .name' wasm/node/cases.json` |
| 「今の値の補い（base 0f8b01c）」の語ごとの比べ（`:2743`） | 比べが増えた。`simd_sum_order_differs` を持つケースの `simd128` の `gemm_simd` の行は、max-abs が 0 なら mismatch（`row: … mismatch (max-abs is zero (simd128 sum order not exercised))`）で、`recount-flat:` の行を出し、終了コード 1 | `grep -n 'recount-flat\|max-abs is zero' scripts/wasm-node-check.sh` |
| 回帰テストの項目の表（`:287`〜） | 項目 111 が増えた（`--expect-cases` の 0・1・2 と、実行の中の期待の照らし） | `grep -n '"111 ' scripts/test-wasm-node-check.sh` |
| 確かめ方の `grep -c '"case"' wasm/node/cases.json`（`:2744`） | `118` → `130` | `grep -c '"case"' wasm/node/cases.json` |
| `make test-wasm-node-check` の最終行（`:2745`） | `passed: 642, failed: 0` → `passed: 664, failed: 0` | `make test-wasm-node-check 2>&1 \| tail -n 1` |
| `wasm/node/cases.json` の行数（`:2746`） | 123 行 → 135 行（`gemm_scalar` 34・`gemm_simd` 34・`topk_scalar` 21・`topk_simd` 21・`roundtrip_f32` 3・`detect_isa_bits` 2・`elementwise_scalar` 10・`elementwise_simd` 10） | `jq '.rows \| length' wasm/node/cases.json`、`jq -r '.rows[].export' wasm/node/cases.json \| sort \| uniq -c` |
| `manifest.json` の `cases` と書く名前（`:2747`） | 30 要素・書く名前は 94 個 → 33 要素（`gemm` 3・`topk` 11・`elementwise` 3・`gemm-special` 14・`elementwise-special` 2）・書く名前は 100 個。増えた 3 要素は `elementwise-add-scalar-c-0-1`・`topk-pad-after-one-finite`・`topk-e-4-k-13` | `d=$(mktemp -d); cargo run -q --manifest-path wasm/Cargo.toml -p wasm-parity-host --locked -- gen "$d"; ls "$d" \| wc -l; jq -r '.cases[].kind' "$d/manifest.json" \| sort \| uniq -c` |
| `sha256sum *` の `manifest.json`（`:2748`） | `e2351cce5ecb46c3dfb8a116d36eff8be43e9de3160283207823d5dc327894c1` → `04d53603d0e3d8c709a264c8b81686cc95df9c0101942b00be4866494551d3fd`（x86_64 の Linux で測った値） | 上の `gen` を流し、`(cd "$d" && sha256sum manifest.json)`、`uname -m` |
| 回帰テストの項目 22 の名前（`:2749`） | `the thirty-four` → `the forty-two`、`the sixteen` → `the twenty` | `grep -n 'bundled .* rows' scripts/test-wasm-node-check.sh` |
| `wasm-parity-host` の `tests/gen.rs` の `test result:`（`:2750`） | `64 passed` → `68 passed`。別に `tests/recount_expected.rs` が増えて `4 passed` | `cargo test --manifest-path wasm/Cargo.toml --workspace --locked 2>&1 \| grep '^test result:'` |
| `summary:` の行（`:2751`） | `rows=123` → `summary: rows=135 match=135 mismatch=0 not-run=0` | CI のログの `summary:` の行 |
| `recount-summary:` の行（`:2752`） | `rows=118` → `recount-summary: rows=130 zero=130 nonzero=0 uncompared=0`（`cases.json` の 135 行から `detect_isa_bits` 2 行・`roundtrip_f32` 3 行を除く） | CI のログの `recount-summary:` の行 |
