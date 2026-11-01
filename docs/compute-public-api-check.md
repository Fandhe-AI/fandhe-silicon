# compute の公開の型に当てた公開 API の検査（TASK-770）の所在と現状の事実

本リポのスクリプト・設定・実行出力から説明できる事実を記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-770。関連: COMP-44。ポインタのみ）。本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

| 前提タスク | 本リポの物 |
| ---------- | ---------- |
| TASK-10 | `scripts/public-api-check.sh`（[public-api-wgpu-check](public-api-wgpu-check.md)） |
| TASK-13 | `scripts/public-api-sdk-check.sh`（[public-api-sdk-check](public-api-sdk-check.md)） |
| TASK-113 | `scripts/public-name-check.sh`・`public-name-check.conf`（[public-name-check](public-name-check.md)） |
| TASK-142 | `make public-name-check` の `--source`・`--exact-fields`（[public-name-check](public-name-check.md) の「決定表の語彙の検査」節） |

compute の型の所在は [contract-compute-types](contract-compute-types.md)（TASK-769）を参照する。

## 対象の型と所在

公開のパスはどれも `fandhe_silicon_core::execution`。feature の条件はどれも無い（`crates/core/src/lib.rs` の `pub mod execution;` に cfg が付かない）。

| 名前 | 種類 | ファイル | feature の条件 |
| ---- | ---- | -------- | -------------- |
| `ComputeOps` | trait | `crates/core/src/execution/compute.rs`（`execution.rs` の `pub use compute::{…}` で再公開） | なし |
| `PipelineHandle`・`Binding`・`ConstantBinding`・`ThreadgroupMemory`・`BindingLayout` | struct | 同上 | なし |
| `CommandQueue::dispatch`・`CommandQueue::dispatch_ex` | trait のメソッド | `crates/core/src/execution/submit.rs` | なし |
| `PipelineInput`・`DType` | enum | `crates/core/src/execution.rs` | なし |
| `OperandSpec` | struct（公開の欄 `slot`・`dtype`・`shape`・`stride`・`alignment`） | `crates/core/src/execution.rs` | なし |

確かめ方（行番号は書かない）:

```sh
grep -nE 'pub (struct|enum|trait) (ComputeOps|PipelineHandle|Binding|ConstantBinding|ThreadgroupMemory|BindingLayout|PipelineInput|OperandSpec|DType)\b' crates/core/src/execution/compute.rs crates/core/src/execution.rs
grep -nE 'fn dispatch(_ex)?\b' crates/core/src/execution/submit.rs
grep -n 'pub mod execution' crates/core/src/lib.rs
```

## 検査ごとの走査の範囲

| 検査 | 入口 | 走査する crate と組 | 当てるもの | compute の型が入るか | 確かめ方 |
| ---- | ---- | ------------------- | ---------- | -------------------- | -------- |
| wgpu 系の参照 | `make public-api-check` | workspace の lib crate 全部。crate ごとに production（contract だけ本番の feature 列、ほかは既定）と `--all-features` の 2 組。検査の経路は `--document-hidden-items` を足す | wgpu 系の型・経路の参照の数 | 入る（観測 (2)） | `scripts/public-api-check.sh` 先頭のコメント |
| チップの SDK の参照 | `make public-api-sdk-check` | `--emit-json` で作る同じ 2 組の JSON と `cargo metadata` | チップの SDK の crate の参照の数 | 入る（観測 (2)） | `scripts/public-api-sdk-check.sh` 先頭のコメント |
| 名前 | `make public-name-check` | `--emit-json` で作る同じ 2 組の JSON | `public-name-check.conf` の `[names]` の語を、crate_id 0 で visibility が public または default の公開アイテム（enum の variant・struct の field を含む）の名前 1 つずつに部分一致で当てる | 入る（観測 (2)(3)） | `scripts/public-name-check.sh` の「照合の規則」のコメント |
| ソースの語・公開フィールドの集合 | `make public-name-check` の `--source`・`--exact-fields` | `crates/core/src/decision.rs` と `decision::DecisionInput` | `[words]`・`[calls]` と公開フィールドの集合 | compute のソースは渡していない | `grep -n 'public-name-check:' -A1 Makefile` |
| WebGPU 系の名前 | `make public-api-webgpu-check` | `--emit-json --without-feature` の組（`without-native-blocking-wait`・`without-ext-interop`） | ブロッキング待ちの名前・`PipelineInput` の形・拡張 trait の閉じ込め（[public-api-webgpu-check](public-api-webgpu-check.md)） | `PipelineInput` が入る | 観測 (1) |

`--emit-json` の経路には `--document-hidden-items` が付かない。検査の経路の JSON と `--emit-json` の JSON は同一ではない。

## 観測の記録

時点: `6225e91`（`git rev-parse --short HEAD`。作業ツリーが clean の状態で実行）。

### (1) 4 検査の終了コードと締めの行

4 つとも終了コード 0。core の行と `record:`・`ok:` の行を出力のまま示す。

```text
$ make public-api-check
scanned: crate=fandhe_silicon_core features=production format_version=61 public_items=3270 wgpu_type_refs=0 wgpu_other_refs=0
scanned: crate=fandhe_silicon_core features=all-features format_version=61 public_items=7293 wgpu_type_refs=0 wgpu_other_refs=0
record: commit=6225e91d0959c9e93874e27ee7b54287e4367d8a documents=10 public_items_total=11240 wgpu_type_refs_total=0 wgpu_other_refs_total=0
ok: 10 public API document(s) checked, no wgpu-family references

$ make public-api-sdk-check
scanned: crate=fandhe_silicon_core features=production format_version=61 public_items=2891 sdk_type_refs=0 sdk_other_refs=0 external_crates=alloc,core
scanned: crate=fandhe_silicon_core features=all-features format_version=61 public_items=6482 sdk_type_refs=0 sdk_other_refs=0 external_crates=alloc,core,std
record: commit=6225e91d0959c9e93874e27ee7b54287e4367d8a documents=10 public_items_total=9581 sdk_type_refs_total=0 sdk_other_refs_total=0 common_declared_deps=0 common_chip_deps=0
ok: 10 public API document(s) checked, common crate dependencies checked, no chip SDK references

$ make public-name-check
scanned: crate=fandhe_silicon_core features=production format_version=61 public_items=2891 named_items=1084 name_hits=0
scanned: crate=fandhe_silicon_core features=all-features format_version=61 public_items=6482 named_items=2345 name_hits=0
record: commit=6225e91d0959c9e93874e27ee7b54287e4367d8a documents=10 public_items_total=9581 name_hits_total=0 sources=1 source_hits_total=0 field_specs=1 field_hits_total=0
ok: 10 public API document(s) and 1 source file(s) checked, no listed words

$ make public-api-webgpu-check
scanned: crate=fandhe_silicon_core features=without-native-blocking-wait format_version=61 public_items=2891 public_functions=393 blocking_wait_names=0 pipeline_create_functions=1 pipeline_input_violations=0 external_interop_items=0 semaphore_handle_defs=1 submit_functions=3 submit_wait_params=3 interop_violations=0 webgpu_names=0
scanned: crate=fandhe_silicon_core features=without-ext-interop format_version=61 public_items=2891 external_interop_items=0 interop_op_names=0 semaphore_handle_defs=1 submit_functions=3 submit_wait_params=3 interop_violations=0 webgpu_names=0
record: commit=6225e91d0959c9e93874e27ee7b54287e4367d8a documents=2 public_items_total=2895 public_functions_total=393 blocking_wait_names_total=0 pipeline_create_functions_total=1 pipeline_input_checked=1 pipeline_input_violations_total=0 interop_documents=2 ext_interop_checked=1 external_interop_items_total=0 semaphore_handle_defined=1 submit_wait_params_total=3 interop_violations_total=0 webgpu_names_total=0
ok: 2 public API document(s) checked, no blocking-wait names, pipeline input ok, ext-interop ok
```

### (2) 走査の JSON に入っている名前

`scripts/public-api-check.sh --emit-json <dir>` が作る core の 2 つの JSON（`fandhe_silicon_core.production.json`・`fandhe_silicon_core.all-features.json`）について、`index` の各アイテムのうち crate_id が 0、visibility が `public` または `default`、`name` が下の表の名前のものを数えた。trait の中の `dispatch`・`dispatch_ex` は visibility が `default` になる（`scripts/public-name-check.sh` と同じ数え方）。

数え方（JSON のパスは `emitted:` の行の `file=` から取る。`<dir>` は一時ディレクトリで、数えた後に消す）:

```sh
python3 -I -c '
import json, sys, collections
names = "ComputeOps PipelineHandle Binding ConstantBinding ThreadgroupMemory BindingLayout dispatch dispatch_ex PipelineInput OperandSpec DType shape stride dtype".split()
for p in sys.argv[1:]:
    c = collections.Counter()
    for it in json.load(open(p))["index"].values():
        if it.get("crate_id") == 0 and it.get("visibility") in ("public", "default") and it.get("name") in names:
            c[it["name"]] += 1
    print(p, " ".join(f"{n}={c[n]}" for n in names))
' <dir>/fandhe_silicon_core.production.json <dir>/fandhe_silicon_core.all-features.json
```

| 名前 | production | all-features |
| ---- | ---------- | ------------ |
| `ComputeOps` | 1 | 1 |
| `PipelineHandle` | 1 | 1 |
| `Binding` | 1 | 1 |
| `ConstantBinding` | 1 | 1 |
| `ThreadgroupMemory` | 1 | 1 |
| `BindingLayout` | 1 | 1 |
| `dispatch` | 1 | 1 |
| `dispatch_ex` | 1 | 1 |
| `PipelineInput` | 1 | 1 |
| `OperandSpec` | 1 | 1 |
| `DType` | 1 | 1 |
| `shape`（欄） | 1 | 1 |
| `stride`（欄） | 1 | 1 |
| `dtype` | 1 | 2 |

### (3) `[names]` の中身と欄の名前

`sed -n '/^\[names\]/,/^\[/p' public-name-check.conf` の出力:

```text
[names]
tensor
broadcast
tiebreak
scene
renderer
tenant

[words]
```

`[names]` は 6 語で、`shape`・`stride`・`dtype` を含まない。`OperandSpec` の欄 `shape`・`stride`・`dtype` と `DType` は (2) の数え方で JSON に入っており、(1) の `make public-name-check` は `name_hits=0`・`name_hits_total=0` で終了コード 0。

### (4) wgpu 系の型の参照

(1) の `make public-api-check` の `record:` の行は `wgpu_type_refs_total=0`・`wgpu_other_refs_total=0`。

### (5) 名前の検査の合格

(1) の `make public-name-check` は `name_hits_total=0`・`source_hits_total=0`・`field_hits_total=0` で、`ok:` の行を出して終了コード 0。

### (6) CI で流れる場所

`grep -n 'public-api-check\|public-name-check\|public-api-sdk-check\|public-api-webgpu-check' .github/workflows/ci.yml` の出力のうち、ステップの行は `deny` ジョブ内の次のとおり。

| 行 | ステップ |
| -- | -------- |
| 306・307 | `make public-api-check` |
| 309・310 | `make test-public-api-check` |
| 321・322 | `make public-name-check` |
| 324・325 | `make test-public-name-check` |
| 327・328 | `make public-api-sdk-check` |
| 330・331 | `make test-public-api-sdk-check` |
| 339・340 | `make public-api-webgpu-check` |
| 342・343 | `make test-public-api-webgpu-check` |

### (7) 検査のスクリプト・設定を変えていないこと

この頁を足した変更の差分（`origin/main` との `--stat`）は、`docs/compute-public-api-check.md`・`CLAUDE.md`・`README.md` の 3 ファイルだけ。`scripts/`・`public-name-check.conf`・`Makefile`・`.github/workflows/`・`crates/` は変えていない。

## 段の表

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | この頁（所在・走査の範囲・観測の記録） | #5611 |

## 確かめ方

```sh
make public-api-check
make public-api-sdk-check
make public-name-check
make public-api-webgpu-check
```

(2) の数え方は上の節のコマンドで再現する。

## 関連

- [public-api-wgpu-check](public-api-wgpu-check.md)
- [public-api-sdk-check](public-api-sdk-check.md)
- [public-name-check](public-name-check.md)
- [public-api-webgpu-check](public-api-webgpu-check.md)
- [public-api-contract-check](public-api-contract-check.md)
- [contract-compute-types](contract-compute-types.md)
