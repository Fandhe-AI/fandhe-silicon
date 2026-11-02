# wasm32 のコンパイル試験の検査（TASK-239）の所在と本リポの現状

wasm32 向けの feature の組で、呼べない呼び出しがコンパイルで落ち、呼べる呼び出しが通ることを確かめる検査の所在と、
本リポの現状の事実をまとめる。

## 記録の所在

記録の正は private spec 側（`docs/spec/05-tasks.md` の TASK-239。ポインタのみ）。本文は転記しない
（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## wasm32 のコンパイル試験の検査と回帰テスト（1/4）

### 所在

| 物 | 場所 |
| -- | ---- |
| 検査の本体 | `scripts/wasm-compile-probe-check.sh`（`make wasm-compile-probe-check`） |
| 回帰テスト | `scripts/test-wasm-compile-probe-check.sh`（`make test-wasm-compile-probe-check`） |
| cargo の出力先 | `target/wasm-compile-probe`（リポジトリのファイルは変更しない） |

### wasm32 の組

`scripts/build-profiles.sh` の注記 `# spec BUILD-19` で終わる行の引用符の中身を組とする。本体は `build-profiles.sh` を読むだけで、
実行も source もしない。確かめ方: `grep -n '# spec BUILD-19' scripts/build-profiles.sh`。

### 試験の行

試験の表は本体の `probe_table` の 1 か所で、snippet は `snippet_<鍵>` の関数に 1 つずつ置く。行を足せる形になっている。

| 行 | wasm32 の組に足す feature | 期待 | 確かめ方 |
| -- | ------------------------ | ---- | -------- |
| `download_blocking` | なし | fail | `MemoryOps::download_blocking` を呼ぶ snippet が `error[E0599]` と識別子 `download_blocking` で落ちる |
| `wait_blocking` | なし | fail | `CommandQueue::wait_blocking` を呼ぶ snippet が `error[E0599]` と識別子 `wait_blocking` で落ちる |
| `nonblocking_readback` | なし | pass | `download_async` と `on_complete` を呼ぶ snippet が通る |
| `download_blocking_gate_open` | `native-blocking-wait` | pass | 1 行目と同じ snippet が通る（陽性対照） |
| `wait_blocking_gate_open` | `native-blocking-wait` | pass | 2 行目と同じ snippet が通る（陽性対照） |

### 一時の crate と流し方

行ごとに `mktemp -d` の下へ一時の crate を作る。依存は `fandhe-silicon-contract`（`default-features = false` と組の feature）と
`fandhe-silicon-core`（feature なし）を path で指し、空の `[workspace]` を持つ。流すコマンドは
`cargo check --offline --manifest-path <一時の Cargo.toml> --target wasm32-unknown-unknown --target-dir target/wasm-compile-probe`。
`RUSTFLAGS` 系・`CARGO_TARGET_DIR`・`CARGO_BUILD_TARGET` は外して呼ぶ。確かめ方: `grep -n 'cargo check' scripts/wasm-compile-probe-check.sh`。

### 判定

- pass の行: cargo の終了コードが 0 なら ok。
- fail の行: 終了コードが 0 でなく、かつ出力に `error[<コード>]:` で始まり `` `<識別子>` `` を含む行があれば ok。終了コード 0・別のコード・別の識別子は ng。
- 出力に `may not be installed` があれば行を判定せず終了コード 2 で止める。

### 終了コードと出力

| 終了コード | 意味 |
| ---------- | ---- |
| 0 | 全行 ok |
| 1 | どれかの行が ng |
| 2 | 誤用・前提の不足（cargo が無い・wasm32 の target が無い）・読めない入力（注記の行が 0 行か 2 行以上・引用符が無い・組の文字が不正） |

標準出力は行ごとに `record: probe=<名前> target=wasm32-unknown-unknown features=<組> expect=<pass|fail> code=<コード|-> result=<ok|ng>`、
全行 ok なら最後に `wasm-compile-probe-check: all probes passed`。ng のときは標準エラーに cargo の `error` で始まる行と
`wasm-compile-probe-check: <n> probe(s) failed` を出す。

### native の側の対応

native の側では `crates/contract/src/cfg_gate_doctest.rs` の `DownloadBlockingGate`・`WaitBlockingGate`・`NonBlockingReadbackGate`
（TASK-233）が同じ 3 つの snippet を doctest として持つ。詳細は [feature-gate-compile-check](feature-gate-compile-check.md)。

## wasm-verify からの呼び出し（2/4）

`scripts/wasm-verify.sh` が `scripts/wasm-compile-probe-check.sh` を引数なしで呼ぶ。呼ぶ位置は、contract の試験の wasm32 の型検査の後・`wasm/` の `cargo test` の前。
`wasm-verify.sh` は `set -euo pipefail` なので、本体が失敗すると `make wasm-verify` も失敗する。`ci.yml` の step は増やしていない。

| ジョブ（check 名） | runner | step | 本体が流れる位置 |
| ------------------ | ------ | ---- | ---------------- |
| `deny`（`make deny (cargo-deny)`） | `ubuntu-latest` | `make wasm-verify` | `wasm-verify.sh` の中 |
| `wasm-macos`（`wasm32 build + size + node (macos-latest)`） | `macos-latest`（arm64） | `make wasm-verify` | `wasm-verify.sh` の中 |

確かめ方: `grep -n 'wasm-compile-probe-check' scripts/wasm-verify.sh`

手元の実行（`origin/main` の `76e52b6` の上の作業ツリー）の `make wasm-verify`（終了コード 0）から抜いた行:

```text
record: probe=download_blocking target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops expect=fail code=E0599 result=ok
record: probe=wait_blocking target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops expect=fail code=E0599 result=ok
record: probe=nonblocking_readback target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops expect=pass code=- result=ok
record: probe=download_blocking_gate_open target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops,native-blocking-wait expect=pass code=- result=ok
record: probe=wait_blocking_gate_open target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops,native-blocking-wait expect=pass code=- result=ok
wasm-compile-probe-check: all probes passed
```

## 対応するテスト

`scripts/test-wasm-compile-probe-check.sh` は合成の `build-profiles.sh` と PATH の先頭に置く偽の cargo で判定を確かめる。
実物の cargo は流さず、リポジトリのファイルを変更しない。

| 場面 | 期待の終了コード | 見る出力 |
| ---- | ---------------- | -------- |
| `good` | 0 | `record:` の 5 行と `all probes passed` |
| `good-args` / `good-manifest` | - | cargo の引数（`--offline`・`--target`・`--manifest-path`・`--target-dir`、`--locked` なし）と一時の Cargo.toml の依存 |
| `env-unset` | 0 | `RUSTFLAGS`・`CARGO_TARGET_DIR` が cargo に渡らない |
| `fail-row-passes` / `fail-row-other-code` / `fail-row-other-ident` | 1 | fail の行が通る・別のコード・別の識別子で `result=ng` |
| `pass-row-fails` / `control-row-fails` | 1 | pass の行と陽性対照の行が落ちると `result=ng` |
| `no-target` | 2 | target が無い旨 |
| `note-zero` / `note-two` / `no-quote` / `bad-char` | 2 | 注記の行・引用符・文字の誤り（cargo を呼ばない） |
| `unknown-arg` / `missing-value` / `missing-path` | 2 | usage と読めない入力 |
| `no-cargo` | 2 | cargo が無い旨 |

## 確かめ方

```sh
make wasm-compile-probe-check
make test-wasm-compile-probe-check
```

記録時点のコミットは `473b99b` の上の作業ツリー。`make wasm-compile-probe-check` の出力:

```text
record: probe=download_blocking target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops expect=fail code=E0599 result=ok
record: probe=wait_blocking target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops expect=fail code=E0599 result=ok
record: probe=nonblocking_readback target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops expect=pass code=- result=ok
record: probe=download_blocking_gate_open target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops,native-blocking-wait expect=pass code=- result=ok
record: probe=wait_blocking_gate_open target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops,native-blocking-wait expect=pass code=- result=ok
wasm-compile-probe-check: all probes passed
```

`make test-wasm-compile-probe-check` の最終行:

```text
test-wasm-compile-probe-check: 23 passed, 0 failed
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | wasm32 のコンパイル試験の検査と回帰テスト | #3944 |
| 2/4 | wasm-verify からの呼び出し | #3945 |
| 3/4 | 回帰テストの CI への組み込み | #3946 |
| 4/4 | 完了の条件の確かめ | #3947 |

親 Issue は #463。
