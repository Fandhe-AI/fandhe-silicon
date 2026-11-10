# 比較基準の別 workspace の、実行時に照らす WGSL と Metal で直接流す GEMM の経路（TASK-1010）の所在と本リポの現状

比較基準の別 workspace `baseline-wgpu/` の腕の crate にある、実行時に環境変数で受けて照らす WGSL の読み込みと、その起動の形の所在と、本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-1010。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 置き場 | `baseline-wgpu/crates/gemm-arm`（crate の所在は [baseline-gemm-arm](baseline-gemm-arm.md)） | `baseline-wgpu/Cargo.toml` の `members` |
| 読み込み | `src/external.rs`（`ExternalWgsl`・エラー型 `ExternalWgslError`。`unsafe` を書かない） | `pub mod external;`（`src/lib.rs`） |
| 起動の形 | `src/layout.rs` の `tiled_workgroup_count`・`tiled_constants_bytes` | 同左 |
| 試験 | `baseline-wgpu/crates/gemm-arm/tests/external.rs`（device は使わない） | ファイルの有無 |
| 流れる場所 | `make baseline-verify`（fmt --check / clippy `-D warnings` / test）に含まれる | `scripts/baseline-verify.sh` |

## 実行時に照らす WGSL の読み込みと起動の形（5/11）

### 受け取り

- 環境変数 `FANDHE_SILICON_KERNEL_SOURCE`（ソースのファイルのパス）と `FANDHE_SILICON_KERNEL_SHA256`（64 桁の小文字の 16 進）を、どちらも必須で受ける。
- 作る口は `ExternalWgsl::from_env()` と `ExternalWgsl::from_lookup(lookup)` だけで、パス・バイト列・文字列を直に受けて作る公開の口は無い。`lookup` は名前から値を引く関数。
- 持つ値は照合を通ったテキストと計算した sha256 で、`text()`・`sha256()` で読む。`Debug` はテキストを出さない。

### 検査の順とエラー

検査は次の順で行い、最初に当たった失敗を返す。エラー型 `ExternalWgslError` は `#[non_exhaustive]` で値を持たず、文言にパス・sha256・ソースの断片・naga のエラーの中身を含まない。

| 順 | 検査 | 失敗 |
| -- | ---- | ---- |
| 1 | 2 つの値がそろう | `Missing` |
| 2 | sha256 が 64 桁の小文字の 16 進（UTF-8 でない値も含む） | `InvalidDigest` |
| 3 | 通常のファイルとして読める（開いた後のファイルでも確かめ直す） | `Unreadable` |
| 4 | 1 MiB（`MAX_SOURCE_BYTES`）以下 | `TooLarge` |
| 5 | sha256 の一致 | `DigestMismatch` |
| 6 | UTF-8 | `NotUtf8` |
| 7 | `wgpu::naga` の解析 | `Parse` |
| 8 | `Capabilities::empty()` での検証 | `Invalid` |
| 9 | compute の入口がちょうど 1 つ | `EntryPoint` |
| 10 | workgroup の大きさが `[16, 16, 1]`（`TILED_WORKGROUP_SIZE`） | `WorkgroupSize` |
| 11 | 束縛の形 | `Bindings` |

束縛の形は、`binding` を持つ global が group 0 の 0・1・2・3 の 4 つだけであることを求める。入口の名前は比べない。`binding` を持たない `var<workgroup>` は比べない。

| binding | アドレス空間 | 型 |
| ------- | ------------ | -- |
| 0・1 | storage（読みだけ） | 大きさが実行時に決まる `array<f32>` |
| 2 | storage（読み書き） | 大きさが実行時に決まる `array<f32>` |
| 3 | uniform | 16 バイトの構造体 |

### 起動の形

詰め物はしない。どちらも、次元が `u32` に収まらないとき `LayoutError::DimensionOverflow` を返す。

| 関数 | 返すもの |
| ---- | -------- |
| `tiled_workgroup_count(m, n)` | `[ceil(n / 16), ceil(m / 16), 1]`（x が列、y が行） |
| `tiled_constants_bytes(m, n, k)` | 元の `m`・`n`・`k` をこの順に little-endian の `u32` で並べ、後ろ 4 バイトは 0 の 16 バイト |

既存の `workgroup_count`・`constants_bytes`（詰めた値、`m`・`k`・`n` の順）は変えない。

### 試験

合成の WGSL と sha256 は試験の中で作り、環境変数は書き換えず `from_lookup` に渡す。

| 試験 | 確かめること |
| ---- | ------------ |
| `good_source_is_accepted` | 合成の WGSL が通り、sha256 とテキストが入力と一致する |
| `entry_point_name_is_not_compared` | 入口の名前を変えても通る |
| `missing_values_are_reported` | 片方・もう片方・両方の欠けが `Missing` |
| `malformed_digests_are_rejected`・`non_utf8_digest_is_rejected` | 大文字・桁数違い・16 進でない文字・UTF-8 でない値が `InvalidDigest` |
| `digest_mismatch_is_rejected` | 1 桁違う sha256 が `DigestMismatch` |
| `missing_path_and_directory_are_unreadable` | 存在しないパスとディレクトリが `Unreadable` |
| `size_limit_boundary` | 上限 + 1 が `TooLarge`、上限ちょうどは `TooLarge` でない |
| `non_utf8_source_is_rejected` | UTF-8 でないバイト列が `NotUtf8` |
| `checks_run_in_a_fixed_order` | 検査の順が上の表のとおり |
| `entry_point_shape_is_checked` | 入口 0 個・2 個・compute でない入口が拒否される |
| `workgroup_size_is_checked` | 大きさ違いが `WorkgroupSize` |
| `bindings_are_checked` | アクセス・要素の型・配列の長さ・構造体の大きさ・余分な束縛・欠けた束縛・group の違いが `Bindings` |
| `parse_and_validation_failures_are_reported` | 構文の誤りが `Parse`、検証に落ちる WGSL が `Invalid` |
| `errors_do_not_print_values` | `Display`・`Debug` にパス・sha256・ソースの断片が出ない |
| `launch_shape_values`・`launch_constants_are_m_n_k_unpadded`・`launch_shape_overflow_is_an_error` | 門の 5 行の起動の形と定数のバイト列の値、`u32` のあふれが `Err` |

### 確かめ方

```bash
make baseline-verify
cargo test --manifest-path baseline-wgpu/Cargo.toml --locked -p baseline-wgpu-gemm-arm --test external
```

## 2 つの device の経路（6/11）

実行時に受けた WGSL のセルを wgpu の device で 1 つ流す経路が 2 つある。違いはシェーダーの作成の口だけで、手順の本体は `src/device.rs` の `pub(crate)` の 1 つの関数を共有する。

| 入口 | 種別 | 置き場 | シェーダーの作成 |
| ---- | ---- | ------ | ---------------- |
| `run_g1(cell, inputs)` | safe な `pub fn` | `src/device.rs` | `create_shader_module`（検査あり） |
| `run_g2(cell, inputs)` | `pub unsafe fn` | `src/bridge.rs` | `create_shader_module_trusted`（`ShaderRuntimeChecks::unchecked()`） |

どちらも成果物（パス・バイト列・sha256）を引数で受けず、戻りは出力（`GateOutput::F32`）と流したソースの sha256 の組。

### 手順

| 順 | 内容 | 失敗 |
| -- | ---- | ---- |
| 1 | セルの腕が入口の腕か | `ArmError::WrongArm` |
| 2 | セルの dtype と入力が f32 か | `ArmError::InputMismatch` |
| 3 | 入力の長さが M×K・K×N か | `ArmError::LengthMismatch` |
| 4 | 検証レイヤーが有効か | `ArmError::ValidationLayerRequired` |
| 5 | `ExternalWgsl::from_env()` | `ArmError::Source(_)` |
| 6 | device の要求（`Features::empty()`、実験的な機能なし） | `NoAdapter`・`RequestDevice` |
| 7 | シェーダー・パイプラインの作成（入口名は指定しない）、buffer・束縛、dispatch（`tiled_workgroup_count`）、投入 1 回、読み戻し | `Validation`・`Readback` |

1〜5 は device を作る前に返る。1〜4 は safe な `check_g2_request(cell, inputs)`（`src/device.rs`）として単独で呼べ、`Ok` を返してもシェーダーは作らない。`run_g2` は最初にこれを呼ぶ。

### unsafe

`run_g2` は `unsafe fn` で、呼び出し側の条件は rustdoc の `# Safety` にある。`unsafe` を持つのは `src/bridge.rs` の `run_g2` と `create_unchecked_module` だけで、橋の外のファイルには `unsafe` も `#[allow(unsafe_code)]` も無い。個数は `tests/unsafe_count.rs` の表が持つ。

### 試験

| ファイル | 呼ぶもの | 確かめること |
| -------- | -------- | ------------ |
| `tests/external_device_misuse.rs` | `run_g1` | 別の腕のセルが `WrongArm`、binary16 の入力が `InputMismatch`、別の行の入力が `LengthMismatch`、検証レイヤーの環境変数が無いと `ValidationLayerRequired`、ソースの環境変数が無いと `Source(Missing)` |
| `tests/external_unchecked_misuse.rs` | `check_g2_request` | 同じ入力の拒否と、検証レイヤーの環境変数が立っていれば正しい入力が `Ok` |

どちらも device には届かない入力だけを渡し、環境変数は書き換えずに子プロセスで切り替える。

### 確かめ方

```bash
make baseline-verify
cargo test --manifest-path baseline-wgpu/Cargo.toml --locked -p baseline-wgpu-gemm-arm --test external_device_misuse --test external_unchecked_misuse --test unsafe_count
```

## 門の run の device の試験（7/11）

門の 1 セルを、枠の `run_selected_from_env` に「腕で `run_g1`・`run_g2` を選ぶ関数」として渡して流す試験を `#[ignore]` 付きで置く。device の要る専用の回でだけ `--ignored` で流し、既定の `cargo test` では流れない。

| ファイル | 試験 | 期待 |
| -------- | ---- | ---- |
| `tests/gate_external.rs` | セルの腕が `GemmGateArm::G1` なら `run_g1`、`GemmGateArm::G2` なら `run_g2` で流し、ほかの腕は `ArmError::WrongArm` を返す | `comparison().is_match()`、`bit_mismatches() == 0`、`max_abs_diff() == 0.0`、`validation_layer()`。標準出力に結果の行と `source_sha256=<64 桁の小文字の 16 進>` を 1 行ずつ出す |

- 試験は 1 本だけ（枠の入口がテストのバイナリにつき 1 回のため）
- 試験は新しい環境変数を読まない。ソースと sha256 は入口が `FANDHE_SILICON_KERNEL_SOURCE`・`FANDHE_SILICON_KERNEL_SHA256` で受ける
- 標準エラーには何も書かない
- 計算した sha256 は出力するだけで、期待の値とは比べない
- ファイルの先頭に `#![allow(unsafe_code)]` を置き、`run_g2`（`unsafe fn`）を呼ぶ `unsafe` ブロックを 1 つだけ書く。`run_g1` は `unsafe` の外で呼ぶ
- `src/` の `unsafe` の個数は変わらない

流すときの形:

```text
scripts/metal-session.sh run --combination <名前> --validation-layer --gate-cell <セルの識別名> -- env FANDHE_SILICON_KERNEL_SOURCE=<ファイル> FANDHE_SILICON_KERNEL_SHA256=<値> cargo test --manifest-path baseline-wgpu/Cargo.toml --locked -p baseline-wgpu-gemm-arm --test gate_external -- --ignored --nocapture
```

確かめ方: `make baseline-verify` で `gate_external` が `1 ignored` になる。

```bash
cargo test --manifest-path baseline-wgpu/Cargo.toml --locked -p baseline-wgpu-gemm-arm --test gate_external
cargo clippy --manifest-path baseline-wgpu/Cargo.toml -p baseline-wgpu-gemm-arm --all-targets --locked --target aarch64-apple-darwin -- -D warnings
```

## pipeline の作成の間の記録の受け取り（8/11）

`baseline-wgpu/crates/gemm-arm/src/capture.rs`（`unsafe` を書かない）が、処理 `f` の間に `log` facade へ流れる Debug の記録から、入口名を持つシェーダーのテキストを 1 つ受け取る `capture_one` を持つ。受け取ったテキストは解釈せず、`f` の戻り値と一緒に返す。

数える記録は、`f` の間の Debug の記録のうち、本文の 1 行目に単引用符で囲んだ入口名を持ち、2 行目以降が `// language: metal` で始まるもの（判定は `shader_text`。2 行目以降を返す）。ほかの記録は保持しない。

| 結果 | 当たる入力 |
| ---- | ---------- |
| `CaptureError::LoggerTaken` | ほかの記録の受け口が既に設置されている（`f` は呼ばない） |
| `CaptureError::NotFound` | 数える記録が 0 件 |
| `CaptureError::Ambiguous` | 数える記録が 2 件以上 |
| `CaptureError::TooLarge` | 数える記録が 1 件で、テキストが `MAX_SHADER_TEXT_BYTES`（1 MiB）を超える |
| `Ok((f の戻り値, テキスト))` | 数える記録がちょうど 1 件で上限以下 |

- 判定の順は表の `NotFound`、`Ambiguous`、`TooLarge`、`Ok`
- `f` の間だけ最大の level を Debug にし、終わったら（`f` が panic しても）`Off` に戻す
- process 全体で直列に動き、`f` の間はほかのスレッドが出した記録も数える
- `CaptureError` の `Display` は入口名・本文を含まない固定の英語

依存: `log` `=0.4.34`（承認の記録は `baseline-wgpu/deps-approved.toml`）。

試験は `tests/capture.rs`（device に触らない。どの OS でも流れる）。

```bash
cargo test --manifest-path baseline-wgpu/Cargo.toml --locked -p baseline-wgpu-gemm-arm --test capture
make baseline-verify
```

## 受け取ったテキストの言語版と束縛の番号の読み取り（9/11）

`baseline-wgpu/crates/gemm-arm/src/msl.rs`（`unsafe` を書かない。device に触れない）が、`capture_one` で受け取ったテキストを読む関数を 2 つ持つ。どちらも失敗は値を持たない `MslError`（`#[non_exhaustive]`）で、`Display` に入口名・番号・テキストの断片を含まない。

| 関数 | 戻り | 読むもの |
| ---- | ---- | -------- |
| `language_version(text)` | `(major, minor)` | 1 行目（最初の改行まで）が `// language: metal<major>.<minor>`（各 1〜2 桁の 10 進）の形であること。前後の空白・`\r`・余分な文字は拒否する。受ける版の判定はしない |
| `buffer_slots(text, entry)` | `Slots` | 入口 `entry` の引数の束縛の番号 5 つ。`Slots` の読み出しは `a()`・`b()`・`c()`・`dims()`・`sizes()`（大きさの表の引数の番号）で、作れるのは `buffer_slots` だけ |

### `buffer_slots` の検査の順

最初に当たった失敗を返す。

| 順 | 検査 | 失敗 |
| -- | ---- | ---- |
| 1 | 入口名が ASCII の識別子で、入口名つきの kernel の宣言と kernel の宣言がどちらもちょうど 1 つ | `EntryPoint` |
| 2 | 束縛の属性ごとに、番号が 1〜2 桁の 10 進で閉じ括弧が続き、31 未満 | `SlotNumber` |
| 3 | 属性の直前の名前が 5 つのどれかで、同じ名前が 2 回出ない | `UnknownSlot`・`DuplicateSlot` |
| 4 | 5 つの名前がそろう | `MissingSlot` |
| 5 | 5 つの番号が互いに違う | `SlotCollision` |
| 6 | 大きさの表の構造体がちょうど 1 つで、本体が 3 行（`size0`・`size1`・`size2` の順の `uint`）だけ | `SizesStruct` |

2 と 3 は属性を先頭から順に見るので、1 つの属性の中で番号の検査が名前の検査より先に当たる。

### 試験

`tests/msl.rs`（合成のテキストだけ。device は使わない）。

| 試験 | 確かめること |
| ---- | ------------ |
| `language_version_reads_the_first_line` | 1〜2 桁の版と、改行の無い 1 行だけのテキストを読む |
| `language_line_shape_is_checked` | 形の違う 1 行目が `LanguageLine` |
| `good_text_returns_the_slots` | 5 つの番号がそのまま読める。空白・字下げの違いと 30 は通る |
| `entry_point_shape_is_checked` | 入口 0 個・2 個・ほかの kernel の宣言・前方一致・識別子でない入口名が `EntryPoint` |
| `slot_numbers_are_checked` | 31 以上・3 桁・数字なし・閉じ括弧の欠けが `SlotNumber` |
| `slot_names_are_checked` | 名前の欠けが `MissingSlot`、重なりが `DuplicateSlot`、ほかの名前・名前の無い属性が `UnknownSlot` |
| `slot_collisions_are_rejected` | 番号の重なりが `SlotCollision` |
| `sizes_struct_is_checked` | 行の欠け・余分・並び・型の違い・構造体の有無と個数が `SizesStruct` |
| `checks_run_in_a_fixed_order` | 複数の誤りがあるとき、上の表の順で最初の失敗が返る |
| `errors_do_not_print_values` | `Display`・`Debug` に値が出ない |

### 確かめ方

```bash
make baseline-verify
cargo test --manifest-path baseline-wgpu/Cargo.toml --locked -p baseline-wgpu-gemm-arm --test msl
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 5/11 | 実行時に照らす WGSL の読み込みと起動の形 | #8194 |
| 6/11 | 2 つの device の経路 | #8195 |
| 7/11 | 門の run の device の試験 | #8196 |
| 8/11 | pipeline の作成の間の記録の受け取り | #8197 |
| 9/11 | 受け取ったテキストの言語版と束縛の番号の読み取り | #8198 |
