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

## 完了の条件の確かめ（3/3）

1/3・2/3 の節を入れた後の作業ツリーで観測した結果。
origin/main のコミット `31a50d5`、`docs/spec` の gitlink は `c7887a1` の時点。コマンドはリポジトリの根で流した。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-262。ポインタのみ）。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | `unsafe` の件数と forbid の宣言 | `make upper-layer-check \| grep '^record:'` と `grep -n 'forbid(unsafe_code)' crates/upper/src/lib.rs` | 終了コード 0、`ok:` の行あり、`forbid=yes`・`unsafe_total=0`。`crates/upper/src/lib.rs` の 9 行目に `#![forbid(unsafe_code)]` |
| 2 | チップの crate への依存とソース中の名前 | `make upper-layer-check \| grep '^record:'` | `chip_deps=0`・`chip_paths=0` |
| 3 | 直接依存 | `make upper-layer-check \| grep '^record:'` と `cargo tree --locked --offline --target all -p fandhe-silicon-upper --depth 1 -e normal,build,dev` | `direct_deps=3`・`other_deps=0`。通常の依存に `fandhe-silicon-core`・`fandhe-silicon-exec`、`[dev-dependencies]` に `fandhe-silicon-core` |
| 4 | 上の層へ向く依存 | `cargo tree --locked --offline --target all --workspace -i fandhe-silicon-upper -e normal,build,dev \| wc -l` と `grep -c 'name = "fandhe-silicon-upper"' measure/Cargo.lock wasm/Cargo.lock baseline-wgpu/Cargo.lock` | 前者は 1。後者は 3 つとも 0 |
| 5 | 単独のビルド | `cargo build --locked --offline -p fandhe-silicon-upper` と `cargo build --locked --offline -p fandhe-silicon-upper --features native-blocking-wait`。`cargo tree --locked --offline --target all -p fandhe-silicon-upper --all-features -e normal,build,dev --prefix none \| grep -c fandhe-silicon-contract`。`git grep -nE '#\[path\|include!\|include_str!\|include_bytes!' -- crates/upper \| wc -l` | ビルドはどちらも終了コード 0。契約の crate の行は 0。ファイルの指定の行は 0 |
| 6 | feature の木の `test-support`・`fault-injection` | `cargo tree --locked --offline --target all -p fandhe-silicon-upper -e features,normal,build \| grep -cE 'test-support\|fault-injection'`（`--all-features` 付きも）、同じコマンドの `-e features,normal,build,dev` 版（`--all-features` 付きも）、`measure/` は `--manifest-path measure/Cargo.toml --workspace --all-features` を付けた同形の 2 つ、`grep -c 'name = "fandhe-silicon-contract"' measure/Cargo.lock` | 通常・build の辺は指定なし・`--all-features` とも 0。dev の辺を含めると 1（`[dev-dependencies]` の下の `fandhe-silicon-core feature "test-support"`）。`measure/` は通常・build の辺が 0、dev の辺を含めると 1（`measure-perf` の `[dev-dependencies]` の下の `fandhe-silicon-core feature "test-support"`）。`measure/Cargo.lock` の契約の crate は 0。`record:` は `measure_pkgs=2`・`measure_contract_pkgs=0`・`measure_forbidden=0` |
| 7 | 検査が CI で流れる場所 | `grep -n 'run: make upper-layer-check\|run: make test-upper-layer-check' .github/workflows/ci.yml` と `make test-upper-layer-check` | `.github/workflows/ci.yml` の 268 行目が `run: make upper-layer-check`、338 行目が `run: make test-upper-layer-check`。`make test-upper-layer-check` の最後の行は `passed=746 failed=0` |

今の値の補い（base 8c52335）: 上の表の 7 の `ci.yml` の行番号は、今の `.github/workflows/ci.yml` では `run: make upper-layer-check` が 269 行目、`run: make test-upper-layer-check` が 343 行目（`grep -n 'run: make upper-layer-check\|run: make test-upper-layer-check' .github/workflows/ci.yml` の出力）。

### 成果物と節の対応

| 成果物 | 節 |
| ------ | -- |
| 件数の記録 | 「unsafe と直接依存の記録（1/3）」・「単独のビルドと feature の木の記録（2/3）」 |
| 確かめ | 「完了の条件の確かめ（3/3）」 |

### 再実行の契機

次の出来事のとき、上の表のコマンドを流し直し、記録時点の表は書き換えずに表の後に補いを置く。

- `crates/upper` の `Cargo.toml`・`src/`・`tests/` が変わったとき
- `measure/` に member か依存が足されたとき
- `scripts/upper-layer-check.sh` の数え方の定義が変わったとき

確かめた base のコミットは `31a50d5c4fbe5913a439ec228779b9ad14b4ff80`、`docs/spec` の gitlink は `c7887a1a7f75903d4016ba42e0595ae6cbb2cd0b`。
完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-262）。

## 段の表

| 段 | イシュー・PR | 節 |
| -- | ------------ | -- |
| 1/3 | #4006（PR #7885） | unsafe と直接依存の記録（1/3） |
| 2/3 | #4007（PR #7887） | 単独のビルドと feature の木の記録（2/3） |
| 3/3 | #7877 | 完了の条件の確かめ（3/3） |

## 本リポでの反映の方針

本書には、同じ PR の作業ツリーで流したコマンドとその出力だけを置く。節を足すときは前の節を書き換えず、新しい節を足す。

## 関連

- [upper-layer-check](upper-layer-check.md)
- [upper-compute-path](upper-compute-path.md)
- [unsafe-policy](unsafe-policy.md)
- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)

## 今の値の補い（base 6a62700）

上の記録の節は記録時点の値で、書き換えない。今の値は次のとおり。
GitHub の表の表示では、セルの中の `\|` が `|` に変わる。ソースの字句のまま流すと `grep -E` の選択が効かず、表示の字句のまま流すと `grep` の基本形の選択が効かない。そこで、表の行のコマンドを表の外に、貼って流せる形で置き直す。

### 表の行のコマンドの置き直し

結果は base `6a627007b3e46763f9a7711b44404251ab76dad9` の作業ツリーで流した値。上の表の値と同じ。

```text
# 23 行目: git 管理下のファイル数
git ls-files crates/upper | wc -l
# 62 行目: 上の層へ向く依存（行数）
cargo tree --locked --offline --target all --workspace -i fandhe-silicon-upper -e normal,build,dev | wc -l
# 78 行目・117 行目: 契約の crate の行
cargo tree --locked --offline --target all -p fandhe-silicon-upper --all-features -e normal,build,dev --prefix none | grep -c fandhe-silicon-contract
# 79 行目・117 行目: ファイルの指定
git grep -nE '#\[path|include!|include_str!|include_bytes!' -- crates/upper | wc -l
# 85 行目・118 行目（通常・build、指定なし）
cargo tree --locked --offline --target all -p fandhe-silicon-upper -e features,normal,build | grep -cE 'test-support|fault-injection'
# 86 行目（通常・build、--all-features）
cargo tree --locked --offline --target all -p fandhe-silicon-upper --all-features -e features,normal,build | grep -cE 'test-support|fault-injection'
# 87 行目（通常・build・dev、指定なし）
cargo tree --locked --offline --target all -p fandhe-silicon-upper -e features,normal,build,dev | grep -cE 'test-support|fault-injection'
# 88 行目（通常・build・dev、--all-features）
cargo tree --locked --offline --target all -p fandhe-silicon-upper --all-features -e features,normal,build,dev | grep -cE 'test-support|fault-injection'
# 96 行目（measure、通常・build）
cargo tree --locked --offline --target all --manifest-path measure/Cargo.toml --workspace --all-features -e features,normal,build | grep -cE 'test-support|fault-injection'
# 97 行目（measure、通常・build・dev）
cargo tree --locked --offline --target all --manifest-path measure/Cargo.toml --workspace --all-features -e features,normal,build,dev | grep -cE 'test-support|fault-injection'
# 119 行目: CI が呼ぶ場所
grep -nE 'run: make upper-layer-check|run: make test-upper-layer-check' .github/workflows/ci.yml
```

| 上の表の行 | 置き直したコマンド | 結果 |
| ---------- | ------------------ | ---- |
| :23 | `git ls-files crates/upper \| wc -l` の形 | 9 |
| :62 | 同上の形 | 1 |
| :78・:117 | `grep -c fandhe-silicon-contract` | 0 |
| :79・:117 | `git grep -nE '#\[path|include!|…'` | 0 |
| :85 | 通常・build、指定なし | 0 |
| :86 | 通常・build、`--all-features` | 0 |
| :87 | 通常・build・dev、指定なし | 1 |
| :88 | 通常・build・dev、`--all-features` | 1 |
| :96 | measure、通常・build | 0 |
| :97 | measure、通常・build・dev | 1 |
| :118 | :85〜:88・:96・:97 と同じ | 上の行と同じ |
| :119 | `grep -nE 'run: …|run: …' .github/workflows/ci.yml` | 268 行目と 338 行目の 2 行 |

今の値の補い（base 8c52335）: 上の表の :119 の行の行番号は、今の `ci.yml` では 269 行目と 343 行目（`grep -nE 'run: make upper-layer-check|run: make test-upper-layer-check' .github/workflows/ci.yml` の出力）。

- 表の表示の字句（`grep -n 'a|b'`）のまま流すと、:119 のコマンドは 0 行を返した
- ソースの字句のまま `grep -cE 'a\|b'` を流すと、:87・:88・:97 の dev の行は 0 を返した（`\|` は拡張正規表現では `|` の文字そのものを指す）
- 上の表の `grep -n 'a\|b'`（基本形）をソースの字句のまま流すと 2 行を返す

### 他の補い

- 段の表の 3/3 は #7877（PR #7890）
- 「本書の前の言及」の `git grep -n TASK-262 -- . ':!docs/spec'` は、この節を足す前の base で 7 行を返す（`CLAUDE.md` 1 行、`README.md` 1 行、本書 5 行〔1・4・27・109・137 行目〕）
- 対象の crate のファイル 9 個は `Cargo.toml` の 1 個と `.rs` の 8 個。`record:` の `rs_files=8` は `.rs` だけを数える
- `make upper-layer-check` の終了コードは 0、`record:` は `rs_files=8 direct_deps=3 forbid=yes unsafe_total=0 chip_deps=0 chip_paths=0 other_deps=0 measure_pkgs=2 measure_contract_pkgs=0 measure_forbidden=0`。`make test-upper-layer-check` の最後の行は `passed=746 failed=0`

## 今の値の補い（base 2f36d3d）

上の節の表と文は記録時点の値で、書き換えていない。今の値は次のとおり（実行環境は GPU なしの Linux）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 所在の「対象の crate」（23 行目） | git 管理下のファイルは 9 個 → 10 個 | `git ls-files crates/upper \| wc -l` |
| `record:` の欄の表（38 行目）の `rs_files` | 8 → 9。`crates/upper/tests/cross_layer_types.rs` が加わった | `make upper-layer-check \| grep '^record:'`、`find crates/upper -name '*.rs'` |
| 「他の補い」の 3 つ目（219 行目） | ファイル 9 個は `Cargo.toml` の 1 個と `.rs` の 8 個。`rs_files=8` は `.rs` だけを数える → ファイル 10 個は `Cargo.toml` の 1 個と `.rs` の 9 個（`src/` に 5 個、`tests/` に 4 個〔`tests/common/mod.rs` を含む〕）。`rs_files=9` は `.rs` だけを数える | `git ls-files crates/upper`、`find crates/upper -name '*.rs'` |
| 「他の補い」の 4 つ目（220 行目） | `record:` は `rs_files=8 direct_deps=3 forbid=yes unsafe_total=0 chip_deps=0 chip_paths=0 other_deps=0 measure_pkgs=2 measure_contract_pkgs=0 measure_forbidden=0` → `rs_files=9` のほかは同じ値。`make test-upper-layer-check` の最後の行は `passed=746 failed=0` のまま | `make upper-layer-check \| grep '^record:'`、`make test-upper-layer-check \| tail -1` |
| 「crate ディレクトリの外から読むファイルの指定」の行 | 0 → 0 | `git grep -nE '#\[path\|include!\|include_str!\|include_bytes!' -- crates/upper \| wc -l` |
