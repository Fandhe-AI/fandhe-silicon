# 相互運用の公開の型と `ExternalInterop`（TASK-717）の所在と本リポの現状

TASK-717 の作業の所在と、本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-717。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 公開に足した名前

すべて feature `ext-interop` の裏。場所は `crates/core/src/interop.rs`（`interop` モジュール）と `crates/core/src/device.rs`。

| 名前 | 種類 |
| ---- | ---- |
| `PhysicalGpuId` | 構造体（`device_uuid`・`driver_uuid: Option<..>` が公開。`Clone`・`Copy`） |
| `ExternalMemoryHandle`・`ExternalSemaphoreHandle`・`ExternalImageHandle` | 構造体（フィールド非公開。`Clone`・`Copy` なし。unix では `OwnedFd` を持つ） |
| `ExportedImageHandle` | 構造体（識別子だけ。`Clone`・`Copy`） |
| `ImportedImageHandle` | 構造体（識別子だけ。`Clone`・`Copy` なし。`import_image` の戻り値） |
| `ExternalImageDescriptor`・`DedicatedInfo` | 構造体（公開の欄。`Clone`・`Copy`） |
| `ExternalImageFormat`・`ExternalImageTiling`・`ExternalImageLayout` | enum（`#[non_exhaustive]`） |
| `ExternalInterop` | trait（必須は `export_memory`・`import_memory`・`export_semaphore`・`import_semaphore`。拡張は `export_image`・`import_image`・`export_semaphore_timeline`・`import_semaphore_timeline` の 4 つで、既定の本体を持つ） |
| `Device::external_interop`・`Device::physical_gpu_id` | `Device` の既定の本体を持つメソッド（どちらも既定は `None`） |

`ImportedSemaphoreHandle` は `crates/core/src/execution.rs` にあるものを使い、動かしていない。

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 組み立ての口 | 4 種のハンドルとも `__backend_new`（`#[doc(hidden)]`・safe）。unix では fd を `__backend_with_fd` で載せ、`__backend_take_fd` で 1 回だけ取り出す。fd の口以外は全 target で使える | `crates/core/tests/interop_types.rs` |
| 識別子の運び方 | `ExternalMemoryHandle` が export 元の `Option<PhysicalGpuId>` を非公開の欄に持つ。共通の crate は照合しない | 同上 |
| 拡張の操作 | 上書きしない型では `UnsupportedExtensionDefault { op }`（`op` は操作の名前）を返す | 同上 |
| `Device` の口 | 既定は `None`。`Box<dyn Device>` 越しに上書きした値が届く。既存の `Device` の実装は変更不要 | `crates/core/tests/interop_device_accessors.rs` |
| feature | `ext-interop` を外した構成では名前が現れない。`contract` の同名 feature が転送する | `make public-api-webgpu-check`、`make core-deps-check` |
| 範囲の外 | `check_same_physical_gpu` と取り込み前の照合は含まない | - |
| 外部依存・unsafe | 足していない | `make deps-check`、`#![forbid(unsafe_code)]` |

## 対応するテスト

| テスト | 内容 |
| ------ | ---- |
| `ExternalMemoryHandle` の rustdoc（`compile_fail,E0382`） | 同じハンドルを `import_memory` に 2 回渡す書き方が拒まれること（INTEROP-44）。同じ前置きで 1 回だけの取り込みが通ること |
| `ExternalSemaphoreHandle` の rustdoc（`compile_fail,E0599`） | `.clone()` が拒まれること（INTEROP-45） |
| `crates/core/tests/interop_types.rs` | `dyn ExternalInterop`、必須の 4 操作だけの実装、拡張の操作の既定の値、`Copy` の有無、識別子の保持、unix の fd の所有 |
| `crates/core/tests/interop_device_accessors.rs` | `Device` の 2 つの口の既定と上書き |
| `crates/core/tests/interop_extension_defaults.rs`（ERR-17） | 拡張 4 操作の返し。上書きしない型は操作ごとに `UnsupportedExtensionDefault { op: <操作の名前> }`、上書きした試験用の型は `Unsupported { op, backend }`、一部だけ上書きした型は上書きした操作だけが変わること。実際の背後実装の返しは含まない |
| `crates/core/tests/interop_extension_defaults.rs`（INTEROP-43） | 必須の 4 操作だけの型が `Box<dyn ExternalInterop>` 越しで、拡張 4 操作に既定の値を返すこと |

## 同一 GPU の判定（TASK-145）

feature `ext-interop` の裏。場所は `crates/core/src/interop/same_gpu.rs`（`interop` から再公開）。どちらも SDK を呼ばない純粋な関数。

| 名前 | 振る舞い |
| ---- | -------- |
| `check_same_physical_gpu(left, right)` | `Option<&PhysicalGpuId>` を 2 つ受ける。どちらかが `None` は `DeviceIdentityMismatch { reason: Undeterminable }`。`device_uuid` が違う、または両方の `driver_uuid` が `Some` で違うと `Mismatch`。`driver_uuid` が片方でも `None` なら比べず `device_uuid` の一致で `Ok(())` |
| `import_memory_checked(interop, target_gpu, handle, byte_len, usage)` | ハンドルが運ぶ export 元の識別子と `target_gpu` を `check_same_physical_gpu` で照らし、`Ok` のときだけ `interop.import_memory` を呼ぶ。拒否のときは呼ばず、ハンドルは消費される |

`import_memory_checked` を使わずに `ExternalInterop::import_memory` を直接呼ぶ呼び出しは、判定されない。`source_gpu` は export 元が付けた値で、真正性は確かめない。`driver_uuid` が片方でも `None` の組は driver の違いを検出しない。`device_uuid` は全ゼロでも一致として扱う。

対応するテスト: `crates/core/tests/interop_same_gpu.rs`（UUID を 1 バイト変えた組、`None` の組、計数つきのスタブで一致 1 回・拒否 0 回、`driver_uuid` の規則、unix での拒否後の fd の閉鎖）。

```bash
cargo test -p fandhe-silicon-core --features ext-interop --test interop_same_gpu
```

## 確かめ方

```bash
cargo test -p fandhe-silicon-core --features ext-interop
cargo test -p fandhe-silicon-core --all-features
cargo build -p fandhe-silicon-core --target wasm32-unknown-unknown --features ext-interop
make verify && make check && make deny
make public-api-check && make public-name-check && make public-api-webgpu-check && make public-api-sdk-check && make core-deps-check
```
