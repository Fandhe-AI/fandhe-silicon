# 契約の crate の実物に当てた公開 API の検査（TASK-253）の所在と現状の事実

本リポのスクリプトと CI のログから説明できる事実を記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-253。関連: BUILD-15・BUILD-47・BUILD-51。ポインタのみ）。本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

| 前提タスク | 本リポの物 |
| ---------- | ---------- |
| TASK-10 | `scripts/public-api-check.sh`（[public-api-wgpu-check](public-api-wgpu-check.md)） |
| TASK-12 | `scripts/baseline-positive-control.sh`（[baseline-positive-control](baseline-positive-control.md)） |
| TASK-13 | `scripts/public-api-sdk-check.sh`（[public-api-sdk-check](public-api-sdk-check.md)） |

`docs/spec` の submodule の版は `git ls-tree origin/main docs/spec` で確かめる。

## 本リポの現状

既存の検査は作り直さず、契約の crate の実物にそのまま当てる。

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 対象の crate | `fandhe-silicon-contract`（`crates/contract`） | `ls crates/contract` |
| 本番の組の渡し方 | contract だけ `--no-default-features --features` に `CONTRACT_PRODUCTION` を渡す（列は本書に列挙しない） | `grep -n -A3 'CONTRACT_PRODUCTION =' scripts/public-api-check.sh` |
| 本番の組から除く feature | `backend-dx12-compute`・`test-support`・`fault-injection`・`artifact-inspection`。`CONTRACT_PRODUCTION` の名前はすべて `[features]` にある | 上の grep と `sed -n '/\[features\]/,/^$/p' crates/contract/Cargo.toml` を見比べる |
| 全 feature の組 | `--all-features` | `grep -n 'all-features' scripts/public-api-check.sh` |
| backend のモジュールの条件 | feature で切り替わる。Metal に target の条件は無く、Vulkan・CUDA は wasm32 を外すだけ。Linux の CI でも本番の組で backend のモジュールが公開 API に入る | `grep -n cfg crates/contract/src/lib.rs` |
| 走る場所 | CI の `deny` ジョブ（ubuntu）が `make public-api-check` を呼ぶ | `grep -n 'public-api-check' .github/workflows/ci.yml` |
| 本書の前の言及 | 本書の前に、公開側に TASK-253 への言及は無かった | `git grep -n TASK-253 -- . ':!docs/spec'` |

## 本番の feature の組と --all-features での wgpu 系の参照（1/4）

`make public-api-check` を、コミット `992386b` で作業ツリーが clean の状態から流した結果。終了コードは 0 で、`ok:` の行が出た。

| 組 | `wgpu_type_refs` | `wgpu_other_refs` | 確かめ方 |
| -- | ---------------- | ----------------- | -------- |
| 本番（production） | 0 | 0 | `make public-api-check \| grep 'scanned: crate=fandhe_silicon_contract'` |
| 全 feature（all-features） | 0 | 0 | 同上 |

- `record:` の行の `wgpu_type_refs_total`・`wgpu_other_refs_total` も 0 だった（`commit=` に `-dirty` は付かなかった）
- `public_items` の値は時点（コミット）で変わるので、この節には書かない。時点を添えた値は 3/4 の節に置く。1 以上で検査が成立している（終了コードが 2 ではない）ことを見る
- 同じ行は、本 PR の CI の `deny` ジョブのログでも確かめる（`gh run view <id> --log | grep 'scanned: crate=fandhe_silicon_contract'`）

## 外部 crate の一覧とチップの SDK の型（2/4）

`make public-api-sdk-check` を、コミット `03c6623` で作業ツリーが clean の状態から流した結果。終了コードは 0 で、`ok:` の行が出た。対象は契約の crate と共通の crate を別々の行で扱う。

| crate | 組 | `sdk_type_refs` | `sdk_other_refs` | `external_crates` | 確かめ方 |
| ----- | -- | --------------- | ---------------- | ----------------- | -------- |
| contract | production | 0 | 0 | `-` | `make public-api-sdk-check \| grep 'scanned: crate=fandhe_silicon_contract'` |
| contract | all-features | 0 | 0 | `alloc,core` | 同上 |
| core | production | 0 | 0 | `alloc,core` | `make public-api-sdk-check \| grep 'scanned: crate=fandhe_silicon_core'` |
| core | all-features | 0 | 0 | `alloc,core,std` | 同上 |

- `external_crates` は公開 API から実際に解決された外部 crate 名（自 crate を除く）の昇順で、1 つも無ければ `-`。一覧は時点（コミット）で変わるので、記録には版を添える
- 依存側の `scanned: check=common-deps` の `chip_deps` は 0。`record:` の `sdk_type_refs_total`・`sdk_other_refs_total`・`common_chip_deps` も 0（`commit=` に `-dirty` は付かなかった）
- 契約の crate の `[dependencies]` は共通の crate への path 依存だけ（確かめ方: `sed -n '/^\[dependencies\]/,/^$/p' crates/contract/Cargo.toml`）
- 同じ出力には exec・upper の行も出るが、本節の対象は contract と core だけ
- 同じ行は、本 PR の CI の `deny` ジョブのログでも確かめる（`gh run view <id> --log | grep 'scanned: crate=fandhe_silicon_\(contract\|core\)'`）

## 公開 API の検査の実行の記録（3/4）

`make public-api-check` と `make public-api-sdk-check` を、コミット `fe8f397` で作業ツリーが clean の状態から流した結果。どちらも終了コードは 0 で、`ok:` の行が出た。

### 時点

- 時点はコミット `fe8f3973f73875bba56a138f0d0d71bbd3b70182`。2 つのスクリプトの `record:` の行の `commit=` が一致し、`-dirty` は付かなかった
- 4 crate の `version` はどれも `0.0.0` なので、時点は版ではなくコミットで決まる（確かめ方: `git grep -n '^version' -- crates/*/Cargo.toml`）
- 時点を添える仕組みは既存のスクリプトの `record:` の行で、本段で作ったものではない（確かめ方: `grep -n 'record:' scripts/public-api-check.sh scripts/public-api-sdk-check.sh`）

### 走査の記録

| crate | 組 | `public_items` | wgpu 系（`wgpu_type_refs` / `wgpu_other_refs`） | SDK（`sdk_type_refs` / `sdk_other_refs`） |
| ----- | -- | -------------- | ------------------------------------------------ | ------------------------------------------ |
| contract | production | 4 | 0 / 0 | 0 / 0 |
| contract | all-features | 198 | 0 / 0 | 0 / 0 |
| core | production | 256 | 0 / 0 | 0 / 0 |
| core | all-features | 2921 | 0 / 0 | 0 / 0 |

- 確かめ方: `make public-api-check | grep 'scanned: crate=fandhe_silicon_\(contract\|core\)'` と `make public-api-sdk-check | grep 'scanned: crate=fandhe_silicon_\(contract\|core\)'`（2 つの検査の `public_items` は同じ値だった）
- 共通の crate の依存の行（`scanned: check=common-deps`）は `declared_deps=0`・`chip_deps=0`

### 締めの行（`record:`）

exec・upper も含む合計で、本節の対象の crate だけの値ではない。

| 検査 | `documents` | `public_items_total` | 参照の合計 | 依存 |
| ---- | ----------- | -------------------- | ---------- | ---- |
| `public-api-check` | 8 | 3383 | `wgpu_type_refs_total=0`・`wgpu_other_refs_total=0` | - |
| `public-api-sdk-check` | 8 | 3383 | `sdk_type_refs_total=0`・`sdk_other_refs_total=0` | `common_declared_deps=0`・`common_chip_deps=0` |

### 注意

- `public_items` は時点で変わる。走査の数を前の記録と比べることは TASK-254 のポインタに留め、本書では比べない
- CI の `deny` ジョブのログの行は、CI が走ったコミットの時点のもの（`gh run view <id> --log | grep 'scanned: crate=fandhe_silicon_\(contract\|core\)'`）。記録したコミットとは違うことがある
- 外部 crate の一覧のまとめは「外部 crate の一覧（4/4）」、完了の条件の確かめは「完了の条件の確かめ（4/4）」の節

## 外部 crate の一覧（4/4）

`make public-api-sdk-check` を、コミット `6f5a955` で作業ツリーが clean の状態から流した結果。終了コードは 0 で、`ok:` の行が出た。

| crate | 組 | `external_crates` |
| ----- | -- | ----------------- |
| contract | production | `-` |
| contract | all-features | `alloc,core` |
| core | production | `alloc,core` |
| core | all-features | `alloc,core,std` |

- 確かめ方: `make public-api-sdk-check | grep 'scanned: crate=fandhe_silicon_\(contract\|core\)'`
- 一覧に `objc2-metal`・`ash`・`cudarc` は 1 つも無く、4 行とも `sdk_type_refs`・`sdk_other_refs` は 0
- `-` は外部 crate が 1 つも無いこと。`alloc`・`core`・`std` は標準の crate
- 一覧は時点（コミット）で変わるので、時点はコミットで添える。4 crate とも `version` が `0.0.0` なのは 3/4 の節と同じ（確かめ方は既存の grep）
- `public_items` は 3/4 の節が持つので、この節には書かない

## 完了の条件の確かめ（4/4）

4/4（#3981）で記録した。本リポで観測できる事実だけを書く（ポインタ: `docs/spec/05-tasks.md` の TASK-253）。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| A1 | 契約の crate・本番の組で wgpu 系の参照が 0 | `make public-api-check \| grep 'scanned: crate=fandhe_silicon_contract'` | `wgpu_type_refs=0`・`wgpu_other_refs=0` |
| A2 | 契約の crate・全 feature の組で同じく 0 | 同上 | 0 / 0 |
| A3 | チップの SDK の型の参照が contract・core とも両方の組で 0 | `make public-api-sdk-check` | `sdk_type_refs=0`・`sdk_other_refs=0`。`common-deps` は `chip_deps=0` |
| B1 | 外部 crate の一覧を contract と core で別々に記録した | 本書の「外部 crate の一覧（4/4）」・2/4 の節 | 記録済み |
| B2 | 記録に時点が添えてある | 2 つのスクリプトの `record:` の `commit=` | 一致し `-dirty` なし。1/4〜4/4 の節すべてにコミットがある |
| C1 | 検出の仕組みの陽性対照 | `make baseline-positive-control` | 終了コード 0（`ok: positive control detected wgpu in baseline-wgpu`） |
| C2 | 検査の仕組みを作り直していない | `git diff --stat origin/main -- scripts/` | 変更 0 件 |
| D | 品質ゲート | `make check` | 終了コード 0。文書だけの変更なので `verify`・`deny` は PR の CI のジョブで確かめる |

- CI の `deny` ジョブのログの行は、CI が走ったコミットの時点のもの。記録したコミットとは違うことがある

### 成果物と節の対応

| 成果物 | 節 |
| ------ | -- |
| 公開 API の検査の実行の記録 | 1/4・2/4・3/4 の節 |
| 外部 crate の一覧 | 2/4 の節・4/4 の節 |

### 再実行の契機

- `scripts/public-api-check.sh`・`scripts/public-api-sdk-check.sh`・`scripts/baseline-positive-control.sh` を変えたとき
- `crates/contract/Cargo.toml`・`crates/core/Cargo.toml` の `[features]`・`[dependencies]` を変えたとき
- `crates/contract/src/lib.rs` の `cfg` を変えたとき
- `docs/spec` の gitlink を進めたとき

確かめた base は `6f5a9556d99ac275ea5222988da6eabcf9b544ba`、`docs/spec` の gitlink は `8664148`。完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-253）。

## 段の表

| 段 | イシュー | 内容 |
| -- | -------- | ---- |
| 1/4 | #3978 | 本番の feature の組と --all-features での wgpu 系の参照 |
| 2/4 | #3979 | 外部 crate の一覧とチップの SDK の型 |
| 3/4 | #3980 | 公開 API の検査の実行の記録（契約の crate・共通の crate、時点つき） |
| 4/4 | #3981 | 外部 crate の一覧のまとめと完了の条件の確かめ |

## 本リポでの反映の方針

本書には事実だけを置き、後の段はその PR で節を足す。守る規約は [spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[dependency-policy](../.claude/rules/dependency-policy.md)。

## 関連

- [public-api-wgpu-check](public-api-wgpu-check.md)・[public-api-sdk-check](public-api-sdk-check.md)・[baseline-positive-control](baseline-positive-control.md)・[public-name-check](public-name-check.md)
- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)
