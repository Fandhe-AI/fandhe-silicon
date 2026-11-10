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

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 5/11 | 実行時に照らす WGSL の読み込みと起動の形 | #8194 |
