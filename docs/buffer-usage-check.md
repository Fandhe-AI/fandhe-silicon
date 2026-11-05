# 用途の対応表と用途と操作の対応の検査

TASK-1796（MEM-96・MEM-109 のポインタ）。private spec の本文は写さない。

## 所在

`crates/core/src/memory/` の `usage_mapping.rs` と `usage_check.rs`、定数の追加は `buffer.rs`。公開のパスは `fandhe_silicon_core::memory::` の下で、feature の条件はない。

| 名前 | 役割 |
| ---- | ---- |
| `BufferUsageKind` | 上の層の用途の 12 値。`contract_usage` で契約の `BufferUsage` に写し、`has_host_path` でホストの経路の有無を返す。`ALL` が 12 値の並び |
| `UsageRole` | 用途を要る操作の役割 17 種。`required_usage` で要る用途を返す。`ALL` が 17 種の並び |
| `validate_usage_for_role` | 確保時の申告 `declared` が役割に要る用途を含むかを判定する純粋関数 |
| `BufferUsage` の追加分 | `MAP_READ`（`1<<7`）・`MAP_WRITE`（`1<<8`）・`INDIRECT`（`1<<9`）・`QUERY_RESOLVE`（`1<<10`）・`BLAS_INPUT`（`1<<11`）・`TLAS_INPUT`（`1<<12`） |

## 現状の振る舞い

- `BufferUsage` の定数は 13 個。既存の 7 個のビットは変えず、生のビットから作る公開のコンストラクタは無い
- `BufferUsageKind::contract_usage` は名前で写し、常に 1 ビットだけの値を返す。`EXPORTABLE` はどの値からも返らない。コピーの元は `DOWNLOAD`、コピーの先は `UPLOAD` に写る
- `has_host_path` が `true` なのは対応付けの 2 値とコピーの 2 値
- `validate_usage_for_role` は、`declared` が要る用途を含めば `Ok`、含まなければ `InvalidArgument`（文言は英語で、役割の名前と足りない用途の値の名前を含む）。要る用途以外のビットが立っていても通り、空の申告はどの役割でも拒否する
- 関数は `declared` を値で受け、バッファの内容・ハンドル・背後実装に触れない。確保もしない（エラーの文字列を除く）。関数は渡された `declared` だけを見る
- 頂点バッファの役割が要るのは `VERTEX` だけ。複数の役割で使う操作では、呼び手が役割ごとに呼ぶ
- テクスチャからバッファへのコピーの先に当たる `UsageRole` は無く、この関数では検査しない。読み戻しの元・`upload` の宛先・描画先からの写しの先は末尾の節の 3 役割で検査する
- 検査の順（device のロストや別 device の確認との前後）と、検査を呼ぶ側の実装は、この関数の外にある

## 対応するテスト

| ファイル | 内容 |
| -------- | ---- |
| `crates/core/src/memory/usage_mapping.rs` の単体テスト | 12 行が 1 ビットずつで重ならない |
| `crates/core/src/memory/usage_check.rs` の単体テスト | 通る・欠けると拒否・各役割が 1 ビットを要る |
| `crates/core/tests/memory_usage_mapping.rs` | 12 行の期待（契約の値・ビット・ホストの経路）、既存 7 ビットの不変、13 値が重ならず和が `0x1FFF`、`EXPORTABLE` が出ないこと、写しの単射 |
| `crates/core/tests/memory_usage_check.rs` | 13 値の 8192 通りの申告 × 14 役割の全網羅（通るのは包含のときだけ。通る・拒否が各 `4096 × 14`）、空の申告の拒否、過申告、単独の申告、文言、拒否の前後でバッファの内容が全バイト一致する試験（通る場合の陽性対照つき） |

背後実装がこの検査を実際に呼ぶことは、ここでは確かめない。新しい値を背後実装の確保に渡したときの振る舞いは、この文書の試験の外。実機は使わない。

## 確かめ方

```bash
cargo test -p fandhe-silicon-core usage_
cargo test -p fandhe-silicon-core --test memory_usage_mapping --test memory_usage_check
make verify
```

## 読み戻しの元・upload の宛先・描画先からの写しの先の役割

TASK-1796 の続き。`UsageRole` に次の 3 役割を `ALL` の末尾へ足した（`ALL` は 17 種、並びは既存の 14 種の後ろに下の順）。

| 役割 | 対象の操作 | 要る用途 | 文言の役割の名前 |
| ---- | ---------- | -------- | ---------------- |
| `ReadbackSource` | `download_blocking`・`download_async` の読み出し元のバッファ | `DOWNLOAD` | `readback-source` |
| `UploadDestination` | `upload` の宛先のバッファ | `UPLOAD` | `upload-destination` |
| `CopyTargetToBufferDestination` | `copy_target_to_buffer` の書き込み先のバッファ | `UPLOAD` | `copy-target-to-buffer-destination` |

- 判定の規則（包含で `Ok`、欠けると `InvalidArgument`、過申告は通す、空の申告は拒否）は既存の役割と同じ
- 背後実装からこの検査を呼ぶ配線は無く、背後実装の振る舞いは変えていない

対応する試験:

| ファイル | 内容 |
| -------- | ---- |
| `crates/core/src/memory/usage_check.rs` の `usage_check_readback_upload_and_copy_target_roles` | 3 役割の要る用途、含む申告は `Ok`、含まない申告は `InvalidArgument` と文言 |
| `crates/core/tests/memory_usage_check.rs` | 並びの表を 17 行に、全網羅を 8192 通り × 17 役割（通る・拒否が各 `4096 × 17`）、文言が役割ごとに異なること、`usage_check_readback_upload_copy_target_roles` |

```bash
cargo test -p fandhe-silicon-core usage_check
make verify
```
