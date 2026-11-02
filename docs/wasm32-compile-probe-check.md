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

`scripts/build-profiles.sh` の `WASM_SETS` のうち、背後実装の feature を含まない行（行末の注記で 1 行に決まる。下の確かめ方の `grep` で出る行）の引用符の中身を組とする。本体は `build-profiles.sh` を読むだけで、
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

手元の実行の出力は「確かめ方」の節（記録時点 `473b99b`）にある。

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

PR の CI の run `41412398011`（`8e4f92d`）の `make deny (cargo-deny)` と `wasm32 build + size + node (macos-latest)` の `make wasm-verify` の step のログにも、上と同じ `record:` の 5 行と `wasm-compile-probe-check: all probes passed` が出ている。

## CI への組み込み（3/4）

足したのは既存の `deny` ジョブの step 1 つで、ジョブ・make ターゲット・スクリプトは足していない（`ci-complete` の `needs` と check 名は変わらない）。step に `continue-on-error`・`|| true`・`if:` は付けていないので、回帰テストの失敗はそのまま `deny` ジョブの失敗になる。`wasm-macos` には足していない。本体は 2/4 のとおり `make wasm-verify` の中で流れる。

| ジョブ（check 名） | runner | step（make ターゲット） | 流すもの |
| ------------------ | ------ | ----------------------- | -------- |
| `deny`（`make deny (cargo-deny)`） | `ubuntu-latest` | `make test-wasm-compile-probe-check` | 回帰テスト（合成の `build-profiles.sh` と偽の cargo。実物の cargo とネットワークは使わない） |

確かめ方: `grep -n 'test-wasm-compile-probe-check' .github/workflows/ci.yml` と `make ci-frame-check`

手元の実行（`origin/main` の `8ec33a8` の上の作業ツリー）の `make test-wasm-compile-probe-check` の最終行:

```text
test-wasm-compile-probe-check: 23 passed, 0 failed
```

PR の CI の run `41414895973`（`fac357a`）の `make deny (cargo-deny)` の `make test-wasm-compile-probe-check` の step のログにも、上と同じ最終行が出ている。

## 完了の条件の確かめ（4/4）

1/4〜3/4 の検査・回帰テスト・`wasm-verify` からの呼び出し・CI への組み込みを、手元の実行と `main` の push の CI の出力で確かめた。結果の欄は出力をそのまま写している。

手元の実行は `origin/main` の `7129b17` の上の作業ツリー。CI は run `41416189346`（headSha `7129b1783631959aaf0c03ef0b35097b6a8c62d5`、conclusion `success`）のジョブ `make deny (cargo-deny)`（job `125180496225`）と `wasm32 build + size + node (macos-latest)`（job `125180496408`）。

| 確かめ | コマンド | 結果 |
| ------ | -------- | ---- |
| (1) 5 つの試験が記録の行と最終行を出す | `make wasm-compile-probe-check` | 下の (1) のブロック |
| (2) ゲートを開けた 2 つの試験が通る | 同上 | 下の (2) のブロック |
| (3) 検査が使う feature の組と、wasm 側に `native-blocking-wait` の記述が無いこと | `grep -n '# spec BUILD-19' scripts/build-profiles.sh` と `git grep -c native-blocking-wait -- crates/wasm-abi wasm; echo "rc=$?"` | 下の (3) のブロック |
| (4) 回帰テスト | `make test-wasm-compile-probe-check` | 下の (4) のブロック |
| (5) main の push の CI | `gh run view --job <job> -R Fandhe-AI/fandhe-silicon --log` を `record: probe=`・`wasm-compile-probe-check: `・`test-wasm-compile-probe-check: ` で絞る | 下の (5) のブロック |
| (6) ゲートの型の所在 | `grep -n 'DownloadBlockingGate\|WaitBlockingGate\|NonBlockingReadbackGate' crates/contract/src/cfg_gate_doctest.rs` | 下の (6) のブロック（TASK-233 の試験。[feature-gate-compile-check.md](feature-gate-compile-check.md)） |
| (7) `crates`・`scripts/build-profiles.sh`・`wasm` の変更 | `git diff --stat 473b99b..HEAD -- crates scripts/build-profiles.sh wasm` | 出力なし |
| (8) 依存の変更 | `git diff --stat 473b99b..HEAD -- Cargo.lock deps-approved.toml wasm/Cargo.lock` | 出力なし |

(1)

```text
[wasm-compile-probe-check.sh] cargo check --target wasm32-unknown-unknown (probe=download_blocking features=contract-core,cpu-isa,cpu-ops)
record: probe=download_blocking target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops expect=fail code=E0599 result=ok
[wasm-compile-probe-check.sh] cargo check --target wasm32-unknown-unknown (probe=wait_blocking features=contract-core,cpu-isa,cpu-ops)
record: probe=wait_blocking target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops expect=fail code=E0599 result=ok
[wasm-compile-probe-check.sh] cargo check --target wasm32-unknown-unknown (probe=nonblocking_readback features=contract-core,cpu-isa,cpu-ops)
record: probe=nonblocking_readback target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops expect=pass code=- result=ok
[wasm-compile-probe-check.sh] cargo check --target wasm32-unknown-unknown (probe=download_blocking_gate_open features=contract-core,cpu-isa,cpu-ops,native-blocking-wait)
record: probe=download_blocking_gate_open target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops,native-blocking-wait expect=pass code=- result=ok
[wasm-compile-probe-check.sh] cargo check --target wasm32-unknown-unknown (probe=wait_blocking_gate_open features=contract-core,cpu-isa,cpu-ops,native-blocking-wait)
record: probe=wait_blocking_gate_open target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops,native-blocking-wait expect=pass code=- result=ok
wasm-compile-probe-check: all probes passed
```

(2)

```text
record: probe=download_blocking_gate_open target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops,native-blocking-wait expect=pass code=- result=ok
record: probe=wait_blocking_gate_open target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops,native-blocking-wait expect=pass code=- result=ok
```

(3)

```text
52:  "contract-core,cpu-isa,cpu-ops"                 # spec BUILD-19
rc=1
```

`git grep -c` の出力は無く、終了コードは 1。

(4)

```text
test-wasm-compile-probe-check: 23 passed, 0 failed
```

(5)

```text
== make deny (cargo-deny)（job 125180496225）
make deny (cargo-deny)	make wasm-verify	2026-11-02T08:53:07.0477322Z record: probe=download_blocking target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops expect=fail code=E0599 result=ok
make deny (cargo-deny)	make wasm-verify	2026-11-02T08:53:07.1203021Z record: probe=wait_blocking target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops expect=fail code=E0599 result=ok
make deny (cargo-deny)	make wasm-verify	2026-11-02T08:53:07.1846391Z record: probe=nonblocking_readback target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops expect=pass code=- result=ok
make deny (cargo-deny)	make wasm-verify	2026-11-02T08:53:07.4578256Z record: probe=download_blocking_gate_open target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops,native-blocking-wait expect=pass code=- result=ok
make deny (cargo-deny)	make wasm-verify	2026-11-02T08:53:07.5220618Z record: probe=wait_blocking_gate_open target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops,native-blocking-wait expect=pass code=- result=ok
make deny (cargo-deny)	make wasm-verify	2026-11-02T08:53:07.5221196Z wasm-compile-probe-check: all probes passed
make deny (cargo-deny)	make test-wasm-compile-probe-check	2026-11-02T08:57:57.7323891Z test-wasm-compile-probe-check: 23 passed, 0 failed
== wasm32 build + size + node (macos-latest)（job 125180496408）
wasm32 build + size + node (macos-latest)	make wasm-verify	2026-11-02T08:53:08.4983230Z record: probe=download_blocking target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops expect=fail code=E0599 result=ok
wasm32 build + size + node (macos-latest)	make wasm-verify	2026-11-02T08:53:08.6480760Z record: probe=wait_blocking target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops expect=fail code=E0599 result=ok
wasm32 build + size + node (macos-latest)	make wasm-verify	2026-11-02T08:53:08.7862860Z record: probe=nonblocking_readback target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops expect=pass code=- result=ok
wasm32 build + size + node (macos-latest)	make wasm-verify	2026-11-02T08:53:09.3300730Z record: probe=download_blocking_gate_open target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops,native-blocking-wait expect=pass code=- result=ok
wasm32 build + size + node (macos-latest)	make wasm-verify	2026-11-02T08:53:09.4873260Z record: probe=wait_blocking_gate_open target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops,native-blocking-wait expect=pass code=- result=ok
wasm32 build + size + node (macos-latest)	make wasm-verify	2026-11-02T08:53:09.4876360Z wasm-compile-probe-check: all probes passed
```

(6)

```text
37:pub struct DownloadBlockingGate;
49:pub struct WaitBlockingGate;
64:pub struct NonBlockingReadbackGate;
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
| 3/4 | CI への組み込み | #3946 |
| 4/4 | 完了の条件の確かめ | #3947 |

親 Issue は #463。

## wasm32 の組の `Send`・`Sync` の行（TASK-553）

### 記録の所在

記録の正は private spec 側（`docs/spec/05-tasks.md` の TASK-553。ポインタのみ）。

### 試験の行と回帰テスト（1/2）

`scripts/wasm-compile-probe-check.sh` の試験の表に 6 行を足した。足す feature は 6 行とも無し（wasm32 の組そのまま）。

| 行 | wasm32 の組に足す feature | 期待 | 確かめ方 |
| -- | ------------------------- | ---- | -------- |
| `native_handle_send_sync` | なし | pass | `NativeDeviceHandle`・`DeviceOwnership`・`DeviceRequest` が `Send + Sync` の境界を満たし、`Arc` の値を包める snippet が通る |
| `native_handle_rejects_not_send` | なし | fail（`E0277`・`Rc<u8>`） | `NativeDeviceHandle::new` に `Rc<u8>` を渡す snippet が `error[E0277]` と `` `Rc<u8>` `` で落ちる |
| `encoder_closer_send_sync` | なし | pass | `__BackendEncoderCloser`・`__BackendSharedEncoderCloser` を実装した型で `CommandEncoder` を作る snippet が通る |
| `encoder_closer_rejects_not_send` | なし | fail（`E0277`・`Rc<()>`） | `Rc<()>` を持つ型に `__BackendEncoderCloser` を実装する snippet が `error[E0277]` と `` `Rc<()>` `` で落ちる |
| `shared_closer_rejects_not_sync` | なし | fail（`E0277`・`Cell<u32>`） | `Cell<u32>` を持つ型に `__BackendSharedEncoderCloser` を実装する snippet が `error[E0277]` と `` `Cell<u32>` `` で落ちる |
| `command_encoder_send_sync` | なし | pass | `CommandEncoder` が `Send + Sync` の境界を満たす snippet が通る |

試験の表は 11 行になった。TASK-239 の節の 5 行の表と `record:` の 5 行は記録時点の値で、補足はこの節に置く。native の側の所在は `crates/core/tests/encoder_send_sync.rs`（TASK-590）。

回帰テスト（`scripts/test-wasm-compile-probe-check.sh`）で足した場面:

| 場面 | 期待の終了コード | 見る出力 |
| ---- | ---------------- | -------- |
| `bound-other-type` | 1 | `shared_closer_rejects_not_sync` の行が `result=ng`、`1 probe(s) failed` |
| `bounds-relaxed` | 1 | `native_handle_send_sync`・`command_encoder_send_sync` の行、`5 probe(s) failed`、`all probes passed` が出ない |

手元の実行（ベース `b6c80bb` の上の作業ツリー）の `make wasm-compile-probe-check` の出力:

```text
record: probe=download_blocking target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops expect=fail code=E0599 result=ok
record: probe=wait_blocking target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops expect=fail code=E0599 result=ok
record: probe=nonblocking_readback target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops expect=pass code=- result=ok
record: probe=download_blocking_gate_open target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops,native-blocking-wait expect=pass code=- result=ok
record: probe=wait_blocking_gate_open target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops,native-blocking-wait expect=pass code=- result=ok
record: probe=native_handle_send_sync target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops expect=pass code=- result=ok
record: probe=native_handle_rejects_not_send target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops expect=fail code=E0277 result=ok
record: probe=encoder_closer_send_sync target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops expect=pass code=- result=ok
record: probe=encoder_closer_rejects_not_send target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops expect=fail code=E0277 result=ok
record: probe=shared_closer_rejects_not_sync target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops expect=fail code=E0277 result=ok
record: probe=command_encoder_send_sync target=wasm32-unknown-unknown features=contract-core,cpu-isa,cpu-ops expect=pass code=- result=ok
wasm-compile-probe-check: all probes passed
```

`make test-wasm-compile-probe-check` の最終行:

```text
test-wasm-compile-probe-check: 25 passed, 0 failed
```

### 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | wasm32 のコンパイル試験に `Send`・`Sync` の行を足す | #7736 |
