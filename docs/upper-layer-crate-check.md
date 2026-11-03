# 上の層の crate の実物に当てた検査（TASK-262）の所在と現状の事実

本リポのスクリプトとコマンドの出力から説明できる事実を記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-262。ポインタのみ）。本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

| 前提タスク | 本リポの物 |
| ---------- | ---------- |
| TASK-9 | `docs/unsafe-policy.md`（`#![forbid(unsafe_code)]` を宣言する crate の一覧） |
| TASK-15 | `scripts/upper-layer-check.sh`（[upper-layer-check](upper-layer-check.md)） |
| TASK-2386 | `crates/upper`（[upper-compute-path](upper-compute-path.md)） |
| TASK-27 | `measure/`（[measurement-tool](measurement-tool.md)） |

`docs/spec` の submodule の版は `git ls-tree origin/main docs/spec` で確かめる。

## 本リポの現状

既存の検査は作り直さず、上の層の crate の実物にそのまま当てる。

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 対象の crate | `fandhe-silicon-upper`（`crates/upper`）。git 管理下のファイルは 9 個 | `ls crates/upper` と `git ls-files crates/upper \| wc -l` |
| 測定の workspace | `measure/` の member は 2 つ（`crates/detect`・`crates/perf`） | `grep -n members measure/Cargo.toml` |
| 検査 | `scripts/upper-layer-check.sh` をそのまま呼ぶ | `make upper-layer-check` |
| 走る場所 | CI の `deny` ジョブが `make upper-layer-check` を呼ぶ | `grep -n 'run: make upper-layer-check' .github/workflows/ci.yml` |
| 本書の前の言及 | 本書の前に、公開側に TASK-262 への言及は 0 件 | `git grep -n TASK-262 -- . ':!docs/spec'` |

## unsafe と直接依存の記録（1/3）

origin/main のコミット `422787d`、`docs/spec` の gitlink は `c7887a1` の時点で、作業ツリーが clean の状態から流した結果。
`make upper-layer-check` の終了コードは 0 で、`ok:` の行が出た。

### `record:` の欄

| 欄 | 値 | 確かめ方 |
| -- | -- | -------- |
| `rs_files` | 8 | `make upper-layer-check \| grep '^record:'` |
| `forbid` | yes | 同上 |
| `unsafe_total` | 0 | 同上 |
| `direct_deps` | 3 | 同上 |
| `chip_deps` | 0 | 同上 |
| `chip_paths` | 0 | 同上 |
| `other_deps` | 0 | 同上 |

- `#![forbid(unsafe_code)]` は `crates/upper/src/lib.rs` の 9 行目にある（`grep -n 'forbid(unsafe_code)' crates/upper/src/lib.rs`）

### 直接依存の一覧

`cargo tree --locked --offline --target all -p fandhe-silicon-upper --depth 1 -e normal,build,dev` の出力から、絶対パスを外して引いた。

| 依存の種類 | package |
| ---------- | ------- |
| 通常の依存 | `fandhe-silicon-core` |
| 通常の依存 | `fandhe-silicon-exec` |
| `[dev-dependencies]` | `fandhe-silicon-core` |

### 上の層へ向く依存の件数

| 対象 | 件数 | 確かめ方 |
| ---- | ---- | -------- |
| 製品の workspace の member | 1（出力の行数。上の層自身の 1 行だけで、他の member からの辺は 0） | `cargo tree --locked --offline --target all --workspace -i fandhe-silicon-upper -e normal,build,dev \| wc -l` |
| `measure/Cargo.lock` | 0 | `grep -c 'name = "fandhe-silicon-upper"' measure/Cargo.lock` |
| `wasm/Cargo.lock` | 0 | `grep -c 'name = "fandhe-silicon-upper"' wasm/Cargo.lock` |
| `baseline-wgpu/Cargo.lock` | 0 | `grep -c 'name = "fandhe-silicon-upper"' baseline-wgpu/Cargo.lock` |

## 単独のビルドと feature の木の記録（2/3）

origin/main のコミット `567e7cd`、`docs/spec` の gitlink は `c7887a1` の時点で、作業ツリーから流した結果。
`make upper-layer-check` の終了コードは 0 で、`ok:` の行が出た。

### 単独のビルド

| 対象 | 結果 | 確かめ方 |
| ---- | ---- | -------- |
| feature なしの単独のビルド | 終了コード 0 | `cargo build --locked --offline -p fandhe-silicon-upper` |
| `native-blocking-wait` を付けた単独のビルド | 終了コード 0 | `cargo build --locked --offline -p fandhe-silicon-upper --features native-blocking-wait` |
| 上の層の依存の木に現れる契約の crate の行 | 0 | `cargo tree --locked --offline --target all -p fandhe-silicon-upper --all-features -e normal,build,dev --prefix none \| grep -c fandhe-silicon-contract` |
| crate ディレクトリの外から読むファイルの指定（`#[path]`・`include!` の類） | 0 | `git grep -nE '#\[path\|include!\|include_str!\|include_bytes!' -- crates/upper \| wc -l` |

### 上の層の feature の木の `test-support`・`fault-injection`

| 辺 | feature | 行数 | 確かめ方 |
| -- | ------- | ---- | -------- |
| 通常・build | 指定なし | 0 | `cargo tree --locked --offline --target all -p fandhe-silicon-upper -e features,normal,build \| grep -cE 'test-support\|fault-injection'` |
| 通常・build | `--all-features` | 0 | `cargo tree --locked --offline --target all -p fandhe-silicon-upper --all-features -e features,normal,build \| grep -cE 'test-support\|fault-injection'` |
| 通常・build・dev | 指定なし | 1 | `cargo tree --locked --offline --target all -p fandhe-silicon-upper -e features,normal,build,dev \| grep -cE 'test-support\|fault-injection'` |
| 通常・build・dev | `--all-features` | 1 | `cargo tree --locked --offline --target all -p fandhe-silicon-upper --all-features -e features,normal,build,dev \| grep -cE 'test-support\|fault-injection'` |

- dev の辺を含めた木で当たる 1 行は、`[dev-dependencies]` の下の `fandhe-silicon-core feature "test-support"`

### 測定の workspace の feature の木

| 対象 | 値 | 確かめ方 |
| ---- | -- | -------- |
| 通常・build の辺の `test-support`・`fault-injection` の行 | 0 | `cargo tree --locked --offline --target all --manifest-path measure/Cargo.toml --workspace --all-features -e features,normal,build \| grep -cE 'test-support\|fault-injection'` |
| 通常・build・dev の辺の `test-support`・`fault-injection` の行 | 1 | `cargo tree --locked --offline --target all --manifest-path measure/Cargo.toml --workspace --all-features -e features,normal,build,dev \| grep -cE 'test-support\|fault-injection'` |
| `measure/Cargo.lock` の `fandhe-silicon-contract` の package | 0 | `grep -c 'name = "fandhe-silicon-contract"' measure/Cargo.lock` |
| `record:` の `measure_pkgs` | 2 | `make upper-layer-check \| grep '^record:'` |
| `record:` の `measure_contract_pkgs` | 0 | 同上 |
| `record:` の `measure_forbidden` | 0 | 同上 |

- dev の辺を含めた木で当たる 1 行は、`measure-perf` の `[dev-dependencies]` の下の `fandhe-silicon-core feature "test-support"`

## 本リポでの反映の方針

本書には、同じ PR の作業ツリーで流したコマンドとその出力だけを置く。節を足すときは前の節を書き換えず、新しい節を足す。

## 関連

- [upper-layer-check](upper-layer-check.md)
- [upper-compute-path](upper-compute-path.md)
- [unsafe-policy](unsafe-policy.md)
- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)
