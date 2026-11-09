# wasm32 向けの cdylib のビルド

wasm32 向けの cdylib（製品の workspace の `crates/wasm-abi`）の所在と、scalar 版のビルドについて、本リポのコードから説明できる現状を記録する。
記録の所在は private spec の TASK-237 とその前提（`docs/spec/05-tasks.md`。ポインタのみ。内容は転記しない）。
親 Issue は #462、この文書は 1/4（#3940）・2/4（#3941）・3/4（#3942）・4/4（#3943）の範囲。

## 所在

| 項目 | 現状 | 確かめ方 |
| ---- | ---- | -------- |
| crate | `crates/wasm-abi/`（package `fandhe-silicon-wasm-abi`。名前は仮。`publish = false`） | `ls crates/wasm-abi` |
| 依存 | 契約の crate（`fandhe-silicon-contract`、`default-features = false`）だけ。外部依存は 0 | `cargo tree -p fandhe-silicon-wasm-abi --no-default-features` |
| feature | `contract-core`・`cpu-isa`・`cpu-ops` を契約の crate へそのまま転送する（`default` はこの 3 つ） | `crates/wasm-abi/Cargo.toml` |
| `crate-type` | `["cdylib", "lib"]`。`lib` は公開 API の検査が lib target を要るために入れている | `cargo metadata --format-version 1 --no-deps` の kind |
| export・公開アイテム | 無い（入口の `//!` と `forbid(unsafe_code)` のみ） | `crates/wasm-abi/src/lib.rs` |
| 契約の crate の CPU 向けの feature | `cpu-ops` が `cpu_ops` を公開している（[cpu-ops](cpu-ops.md)） | `grep -n -B1 'pub mod cpu_ops' crates/contract/src/lib.rs` |
| 別 workspace `wasm/` の glue | 別の物。本 crate は `wasm/` を参照しない | [workspace-boundary](workspace-boundary.md) |

## scalar 版のビルド（BUILD-19 のポインタ）

`RUSTFLAGS` を付けずに、ルートで次を流す。

```sh
env -u RUSTFLAGS -u CARGO_ENCODED_RUSTFLAGS cargo build -p fandhe-silicon-wasm-abi \
  --no-default-features --features contract-core,cpu-isa,cpu-ops \
  --target wasm32-unknown-unknown --release --locked
```

| 項目 | 現状 | 確かめ方 |
| ---- | ---- | -------- |
| 成果物 | `target/wasm32-unknown-unknown/release/fandhe_silicon_wasm_abi.wasm` が実在し非空 | `ls -l` |
| 形式 | 先頭 4 バイトが `00 61 73 6d` | `od -An -tx1 -N4 <成果物>` |
| rustflags | リポジトリに `.cargo/config.toml` は無く、scalar 版は追加の rustflags を使わない | `ls .cargo`（無いこと） |
| rustflags（simd128 版） | `RUSTFLAGS` に `-C target-feature=+simd128` を付ける（下の節） | 下の節 |

## simd128 版のビルド（BUILD-20 のポインタ）

同じ feature の組で、`RUSTFLAGS` に `-C target-feature=+simd128` を付け、`--target-dir` を分けて、ルートで次を流す。

```sh
env -u CARGO_ENCODED_RUSTFLAGS RUSTFLAGS="-C target-feature=+simd128" \
  cargo build -p fandhe-silicon-wasm-abi \
  --no-default-features --features contract-core,cpu-isa,cpu-ops \
  --target wasm32-unknown-unknown --release --locked \
  --target-dir target/wasm32-simd128
```

`CARGO_ENCODED_RUSTFLAGS` が環境にあると、cargo は `RUSTFLAGS` より先にそちらを使う。`+simd128` が付かないまま scalar 版と同じものができるので、外してから流す。

| 項目 | 現状 | 確かめ方 |
| ---- | ---- | -------- |
| 成果物 | `target/wasm32-simd128/wasm32-unknown-unknown/release/fandhe_silicon_wasm_abi.wasm` が実在し非空 | `ls -l` |
| 形式 | 先頭 4 バイトが `00 61 73 6d` | `od -An -tx1 -N4 <成果物>` |
| `--target-dir` を分ける理由 | 2 版の成果物は target-dir から下が同じ相対パスで、分けないと後から作った方が前の方を上書きする | 2 つのパスが別であること |
| 置き場 | `target/` の下なので `.gitignore` の `/target/` に入る | `git check-ignore -v target/wasm32-simd128/x` |

## 2 版の判別・型の存在・cfg の確かめ（BUILD-21・BUILD-25 のポインタ。3/4）

検査は `scripts/wasm-build-check.sh`（`make wasm-build-check`）、回帰テストは `scripts/test-wasm-build-check.sh`（`make test-wasm-build-check`）。
検査の本体は手元で流す道具で CI からは呼ばない。回帰テストは CI の `deny` ジョブが呼ぶ（[CI への組み込み](#ci-への組み込み44)）。bash + python3（3.8 以上・標準ライブラリのみ）+ cargo + `wasm32-unknown-unknown` の target が要り、無ければ終了コード 2（skip しない）。

```sh
scripts/wasm-build-check.sh [--require-type <path>]... [--source-root <dir>]
```

既定の経路は 2 版を専用の target-dir（cargo の target_directory 配下の `wasm-build-check/`）にビルドして調べる。`RUSTFLAGS` 系の環境変数は子プロセスへ渡さず、simd128 版にだけ `CARGO_ENCODED_RUSTFLAGS` で `+simd128` を渡す。
型を確かめる名前は検査に持たせず `--require-type` で受ける（crate 名の有無はどちらでもよい）。

| 行 | 確かめること | 判定 |
| -- | ------------ | ---- |
| `artifact:`（版ごと） | 先頭が wasm の header で、custom section `target_features` に `+simd128` が simd128 版にだけ在り、2 つが別のバイト列であること | match / mismatch（`reason=identical-artifacts`・`missing-target-features`） |
| `type:`（名前ごと） | `--require-type` の各 path が、wasm32 向けの rustdoc JSON の公開アイテムに在ること | match / mismatch。0 個なら not-run（`reason=no-required-types`） |
| `cfg:` | `target_feature = "simd128"` の出現（コメントを除く）がすべて `cfg(all(target_arch = "wasm32", target_feature = "simd128"))` か、その `not(...)` の中に在り、実行時の検出が無いこと | match / mismatch（`violation:` の行に file・line・reason）。出現が 0 件なら not-run（`reason=no-simd128-sites`） |

| 項目 | 現状 |
| ---- | ---- |
| 終了コード | 0 全行 match / 1 mismatch か not-run が 1 行以上 / 2 誤用・前提の不足・読めない入力（壊れた wasm・LEB128・重複した section・読めない JSON を含む） |
| 締めの行 | 終了コード 0 と 1 のときだけ `record:`（commit・`artifact=<n>/2`・`types=<m>/<n>`・`cfg_sites`・`not_run`）。合格時は `ok:` |
| 出力に出さないもの | ホストの絶対パス・環境変数の値 |
| 合格に見せる口 | 無い（除外・閾値を緩める引数・skip は持たない。not-run は一致に数えない） |

### 回帰テストとの対応

| 観点 | ケース |
| ---- | ------ |
| 成果物の判別 | 正しい組・2 版が同一・simd128 版に `+simd128` が無い・scalar 版に有る・section が無い・同じパス・ファイルが無い・空・magic の違い・切れた LEB128・切れた section・重複した section |
| 型 | 在る（短い path と crate 名付きの path）・無い・非公開・外部 crate・`--require-type` が 0 個・JSON の形が違う・JSON でない・JSON が無い |
| cfg | 正しい形（複数行を含む）・`not(all(...))`・`target_feature` だけ・`any(...)`・実行時の検出・コメントだけ・出現が 0 件・起点が無い・本リポの `crates/` の `//!` にある語を数えないこと |
| 偽の cargo | scalar 版に rustflags 系が渡らない・simd128 版に `+simd128` だけが渡る・2 版の target-dir が別・型が無ければ rustdoc を呼ばない・rustdoc にだけ `RUSTC_BOOTSTRAP`・ビルドの失敗は終了コード 2 |
| 実物 | 一時 workspace（本体と `lib.sh` を写したもの）で実物の cargo による 2 版のビルドと rustdoc を通し、全行 match と、型が無いときの mismatch |
| 誤用 | 未知の引数・`--from-dir` の引数の不足・`--require-type` の値なし |

### 本リポの現状での実行結果

`make wasm-build-check`（`--require-type` なし）を本リポの `main` で流した結果は次のとおり。終了コード 1 は想定どおりで、結果を合格に見せる設定は入れていない。

| 行 | 結果 | 理由 |
| -- | ---- | ---- |
| `artifact:`（scalar・simd128） | 2 行とも match（300 バイトと 309 バイト。simd128 版にだけ `+simd128`） | 空の crate でも `target_features` で判別できる |
| `type:` | not-run | `--require-type` を渡していない |
| `cfg:` | not-run | コメントを除くと `simd128` の出現が `crates/` に 0 件 |

補い: 契約の crate に `cpu_ops`（[cpu-ops](cpu-ops.md)）が入った後の `abd6c95` で `make wasm-build-check` を流すと、`cfg:` の行は `sites=5 label=match`、`type:` の行は `label=not-run reason=no-required-types`、終了コードは 2 だった。

## 現状の値の補い

上の「本リポの現状での実行結果」の表と、その後の補い（`abd6c95`）は記録した時点の値で、書き換えていない。`d909cc7` で `make wasm-build-check` を流した結果は次のとおり。

| 行 | 結果 |
| -- | ---- |
| `artifact:`（scalar） | `label=match simd128=absent bytes=300` |
| `artifact:`（simd128） | `label=match simd128=present bytes=309` |
| `type:` | `label=not-run reason=no-required-types` |
| `cfg:` | `sites=5 label=match`（`simd128` の出現は 0 件ではない） |
| `record:` | `artifact=2/2 types=0/0 cfg_sites=5 not_run=1` |

終了コードは、`scripts/wasm-build-check.sh` を直接流すと 1、`make wasm-build-check` は make が包むので 2（make の出力に `Error 1` が出る）。`make` 経由の 2 は make の値で、スクリプトそのものの値ではない。

## CI への組み込み（4/4）

既存のジョブへステップとして足した。新しいジョブ・runner・ツールは無く、check 名と `ci-complete` の `needs` は変わらない。

| ジョブ | ステップ | 流すもの |
| ------ | -------- | -------- |
| `verify` | `make build-profiles` | `scripts/build-profiles.sh` の末尾で `crates/wasm-abi` の scalar 版・simd128 版を release でビルドし、成果物の在りかと wasm の先頭 4 バイトを確かめる（BUILD-19・BUILD-20 のポインタ）。利用者の rustflags 系の環境変数は外し、simd128 版にだけ `+simd128` を渡す。target が無ければ失敗する |
| `deny` | `make test-wasm-build-check` | `scripts/wasm-build-check.sh` の回帰テスト（偽の cargo と一時 workspace のダミーの crate。ネットワーク不要） |

`make wasm-build-check` の本体は CI から呼ばない。本リポの現状では `type:` に渡せる公開の型の名前が `crates/` に無く、`cfg:` の対象の `simd128` の出現も 0 件で、どちらも not-run になり終了コード 1 だから。
ダミーの型や `cfg` を足す、not-run を許す口を足すといった合格に見せる変更はしない。

今の値の補い（base 09a69f3）: 上の「`make wasm-build-check` の本体は CI から呼ばない」の段落と、確かめ方の表の最終行（`grep -n "run: make wasm-build-check" .github/workflows/ci.yml` が 0 件）は記録時点の書き方のまま。今は `deny` ジョブが本体を呼ぶ（後ろの「CI の deny ジョブでの本体の実行（2/3）」の節）。

組み込みに要る条件は次の 3 つ。そろったら Makefile のターゲットに `--require-type` を渡したうえで `deny` ジョブへ足す。

- `--require-type` に渡す公開の型が `crates/` にある
- `cfg:` の出現が 1 件以上ある
- 手元の `make wasm-build-check` が終了コード 0 になる

確かめ方:

| 確かめること | コマンド |
| ------------ | -------- |
| 2 版のビルドが `verify` から流れる | `make build-profiles`（ログに `wasm-abi scalar` と `wasm-abi simd128` の `cargo build` が出る） |
| 回帰テストが通る | `make test-wasm-build-check` |
| `deny` ジョブに足したステップ | `grep -n "wasm-build-check" .github/workflows/ci.yml` |
| 本体が CI から呼ばれていない | `grep -n "run: make wasm-build-check" .github/workflows/ci.yml`（0 件） |

## simd128 の cfg の検査の CI での実行（TASK-2954）

記録の正は private spec 側（`docs/spec/05-tasks.md` の TASK-2954 のポインタ）。

### cfg の行だけを流す入口と wasm-verify からの呼び出し（1/2）

`scripts/wasm-build-check.sh --cfg-only` は、`crates/` の走査（`cfg:` の行）だけを流す。cargo を呼ばず、wasm も rustdoc も作らない。前提は python3 3.8 以上だけ。全行の経路（`--cfg-only` なし）の判定・出力・終了コードは上の節のまま。

| 入力 | 出す行 | 終了コード |
| ---- | ------ | ---------- |
| 出現がすべて正しい形で、実行時の検出が無い | `cfg: sites=N label=match`・`record: commit=... mode=cfg-only cfg_sites=N not_run=0`・`ok: all rows match` | 0 |
| 正しくない形か実行時の検出がある | `cfg: ... label=mismatch reason=...`・`violation:` の行・`record:`・標準エラーに `FAILED:` | 1 |
| 出現が 0 件 | `cfg: sites=0 label=not-run reason=no-simd128-sites`・`not_run=1`（合格に数えない） | 1 |
| `--require-type`・`--from-dir`・`--rustdoc-json` のどれかとの併用（`--source-root` は併用してよい） | 標準エラーに `usage:` | 2 |
| 起点のディレクトリが無い | 標準エラーに `error: source root is not a directory` | 2 |

呼ぶ場所:

| ジョブ | runner | 呼ぶステップ | 呼ぶ場所 |
| ------ | ------ | ------------ | -------- |
| `deny` | `ubuntu-latest` | `make wasm-verify` の中 | `scripts/wasm-verify.sh` が `wasm-compile-probe-check.sh` の後で `wasm-build-check.sh --cfg-only` を呼ぶ |
| `wasm-macos` | `macos-latest` | `make wasm-verify` の中 | 同上 |

`ci.yml` のステップは増えていない。

回帰テスト（`scripts/test-wasm-build-check.sh`）に足したケース:

| ケース | 入力 | 期待の終了コード | 見る出力 |
| ------ | ---- | ---------------- | -------- |
| `cfg-only-good`・`cfg-only-not-form` | 正しい形の合成ソース | 0 | `label=match`・`mode=cfg-only`・`ok:`（`artifact:`・`type:` は出ない） |
| `cfg-only-runtime`・`cfg-only-only-feature` | 実行時の検出・`all(...)` でない形 | 1 | `reason=runtime-detection`・`reason=unsupported-cfg-form` |
| `cfg-only-zero-sites` | 出現が 0 件 | 1 | `label=not-run`・`not_run=1`・`ok:` が出ない |
| `cfg-only-source-root-missing` | 起点が無い | 2 | `error: source root is not a directory` |
| `cfg-only-misuse-*` | 3 つの併用 | 2 | `usage:` |
| `cfg-only-fake-*` | 偽の cargo を `PATH` に置いて流す | 0 | `ok:`。偽の cargo の記録に呼び出しが無い |
| `cfg-only-real-crates` | 実物の `crates/` | 0 | `label=match`・`violation:` が出ない（件数は固定しない） |
| `cfg-only-positive-control-*` | 実物の `crates/` の一時の写しに、実行時の検出を 1 行足す／`mod simd128;` の属性を `cfg(target_feature = "simd128")` だけにする | 1 | `reason=runtime-detection`・`reason=unsupported-cfg-form`、`violation: file=contract/src/cpu_ops.rs` |

確かめ方:

| 確かめること | コマンド |
| ------------ | -------- |
| 入口が `wasm-verify.sh` から呼ばれる | `grep -n 'cfg-only' scripts/wasm-verify.sh` |
| 実物の `crates/` が通る | `scripts/wasm-build-check.sh --cfg-only`（終了コード 0） |
| 併用は誤用 | `scripts/wasm-build-check.sh --cfg-only --require-type X`（終了コード 2） |
| 回帰テストが通る | `make test-wasm-build-check` |

重なる既存の試験の所在: `crates/contract/tests/cpu_ops_build_selection.rs`（TASK-1351。`make verify` が流す）。

手元の出力（`bash scripts/wasm-build-check.sh --cfg-only`、終了コード 0。`commit=` の値は流した時点の HEAD で変わる）:

```text
cfg: sites=5 label=match
record: commit=1376f6a mode=cfg-only cfg_sites=5 not_run=0
ok: all rows match
```

### 完了の条件の確かめ（2/2）

1/2 の入口と回帰テストを、手元の実行と main の push の CI の出力で確かめた。結果の欄は出力をそのまま写している。

- 手元: `origin/main` の `08b18aa` の上の作業ツリー
- CI: run `41561344417`（headSha `08b18aa2e33f4df718ab495988c4e82e116ca92b`・conclusion `success`）。ジョブ `make deny (cargo-deny)`（job `125672426920`）・`wasm32 build + size + node (macos-latest)`（job `125672426938`）

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 実物の `crates/` が通る | `scripts/wasm-build-check.sh --cfg-only` | 下の (1) のブロック |
| 2 | CI の 2 ジョブで `make wasm-verify` の中で流れる | 各ジョブのログを `cfg: sites=` / `mode=cfg-only` / `ok: all rows` で絞る | 下の (2) のブロック |
| 3 | 回帰テストと陽性対照 | `make test-wasm-build-check`、`crates/` の写しに実行時検出の呼び出しを足して `--source-root` で渡す | 下の (3) のブロック |
| 4 | `cfg` の出現の数 | `grep -n` で `crates/contract/src/cpu_ops.rs` の simd128 の `cfg` を引く | 下の (4) のブロック |
| 5 | `crates/*/src` に実行時検出が無い | `grep -rn '_feature_detected' crates/*/src` | 下の (5) のブロック |
| 6 | 重なる既存の試験が流れる | `cargo test -p fandhe-silicon-contract --test cpu_ops_build_selection` | `test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| 7 | 引数なしの全行の経路 | `scripts/wasm-build-check.sh` | 下の (7) のブロック |
| 8 | 対象外のファイルを変えていない | `git diff --stat 08b18aa^ 08b18aa -- crates .github wasm Cargo.toml Cargo.lock deps-approved.toml scripts/build-profiles.sh` | 出力なし |

(1)

```text
cfg: sites=5 label=match
record: commit=08b18aa mode=cfg-only cfg_sites=5 not_run=0
ok: all rows match
rc=0
```

(2)

```text
make deny (cargo-deny)	make wasm-verify	2026-11-03T01:42:30.2372975Z cfg: sites=5 label=match
make deny (cargo-deny)	make wasm-verify	2026-11-03T01:42:30.2373736Z record: commit=08b18aa mode=cfg-only cfg_sites=5 not_run=0
make deny (cargo-deny)	make wasm-verify	2026-11-03T01:42:30.2374469Z ok: all rows match
wasm32 build + size + node (macos-latest)	make wasm-verify	2026-11-03T01:42:44.3781050Z cfg: sites=5 label=match
wasm32 build + size + node (macos-latest)	make wasm-verify	2026-11-03T01:42:44.3781400Z record: commit=08b18aa mode=cfg-only cfg_sites=5 not_run=0
wasm32 build + size + node (macos-latest)	make wasm-verify	2026-11-03T01:42:44.3781620Z ok: all rows match
```

(3)

```text
手元の make test-wasm-build-check の最終行:
test-wasm-build-check: 87 passed, 0 failed
CI の deny ジョブ（make test-wasm-build-check の step）:
test-wasm-build-check: 87 passed, 0 failed
crates/ の写しの cpu_ops.rs の末尾に is_x86_feature_detected!("simd128") を呼ぶ関数を足して実行:
FAILED: at least one row is mismatch or not-run
cfg: sites=5 label=mismatch reason=runtime-detection
violation: file=contract/src/cpu_ops.rs line=482 reason=runtime-detection
record: commit=08b18aa mode=cfg-only cfg_sites=5 not_run=0
rc=1
```

(4)

```text
12:#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
87:#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
100:#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
113:#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
123:#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
```

(5)

```text
rc=1
```

(7)

```text
FAILED: at least one row is mismatch or not-run
artifact: kind=scalar label=match simd128=absent bytes=300 sha256=207eab239a9a837827ce9fc92a715af8bcdd48aa2c729b2315f85af95b43952a
artifact: kind=simd128 label=match simd128=present bytes=309 sha256=5c97560c5151429e5b122307b90d64cd49140e5c5304abb95508e4eb5602dad9
type: label=not-run reason=no-required-types
cfg: sites=5 label=match
record: commit=08b18aa artifact=2/2 types=0/0 cfg_sites=5 not_run=1
rc=1
```

## 今の値の補い（base ed82c18）

記録時点の表と既存の節は書き換えない。以下は base `ed82c18` で観測した今の値（実行環境は GPU なしの Linux x86_64）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| `cfg:` の行を記録した各節 | `cfg: sites=5 label=match` → `cfg: sites=6 label=match`（#8058 が `crates/contract/tests/` に `cfg` を足した）。`record:` の `cfg_sites=5` → `cfg_sites=6` | `scripts/wasm-build-check.sh --cfg-only` |

## 今の値の補い（base 2f35b9e）

記録時点の表と既存の節は書き換えない。以下は base `2f35b9e` で観測した今の値（実行環境は GPU なしの Linux x86_64）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 完了の条件の確かめの 6 の行（`:217`） | `7 passed` → `17 passed`（走査の項目 (i) と陽性対照 4 本が増えた） | `cargo test -p fandhe-silicon-contract --test cpu_ops_build_selection` |

## 依存の crate の型の照合と本体の CI での実行（TASK-237）

記録の正は private spec 側（`docs/spec/05-tasks.md` の TASK-237 のポインタ）。

### 依存の crate の rustdoc JSON での型の照合（1/3）

`scripts/wasm-build-check.sh` の `type:` の行は、`crates/wasm-abi` の依存の crate も含めて作った rustdoc JSON で型を照合する。

`cargo doc` の呼び方（2 版で共通。`--no-deps` は付けない）:

| 項目 | scalar 版 | simd128 版 |
| ---- | --------- | ---------- |
| コマンド | `cargo doc --release --locked -p fandhe-silicon-wasm-abi --no-default-features --features contract-core,cpu-isa,cpu-ops --target wasm32-unknown-unknown` | 同左 |
| rustdoc の指定（`CARGO_ENCODED_RUSTDOCFLAGS`） | `-Zunstable-options --output-format json` | 左に `-C target-feature=+simd128` を足す |
| rustc の指定（`CARGO_ENCODED_RUSTFLAGS`） | 渡さない | `-C target-feature=+simd128` |
| 出力の置き場（`--target-dir`） | `<target_directory>/wasm-build-check/rustdoc-scalar` | `<target_directory>/wasm-build-check/rustdoc-simd128` |
| JSON の置き場 | `<置き場>/wasm32-unknown-unknown/doc/<crate 名>.json` | 同左 |

流す前に JSON の置き場の `*.json` を消す。`fandhe_silicon_wasm_abi.json` が無ければ `error: rustdoc JSON was not produced`（終了コード 2）、`cargo doc` の失敗は `error: cargo doc failed`（2）。

path の照らし先の規則（先頭の `::` までの語だけで決まり、別の crate を探し直さない）:

| 入力 | 照らす JSON | 例 |
| ---- | ----------- | -- |
| 先頭が wasm-abi 以外で、置き場に JSON のある crate 名 | その crate の公開パス（先頭の語を除いた残り） | `fandhe_silicon_core::capability::Capabilities` |
| crate 名なし、または `fandhe_silicon_wasm_abi::` 付き | wasm-abi の公開パス | `dummy_mod::Alpha` |
| 置き場に JSON の無い crate 名 | wasm-abi の公開パス（mismatch） | `other_crate::dummy_mod::Alpha` |
| crate 名だけ | wasm-abi の公開パス（mismatch） | `dep_crate` |

`--rustdoc-dir <dir>`（回帰テスト用）は、`<crate>.json` を並べたディレクトリを cargo を呼ばずに 2 版とも使う。

| 入力 | 出す行 | 終了コード |
| ---- | ------ | ---------- |
| `fandhe_silicon_wasm_abi.json` の無いディレクトリ | `error: cannot read rustdoc JSON` | 2 |
| 読めない JSON を含むディレクトリ | `error: cannot read rustdoc JSON` | 2 |
| 存在しないディレクトリ | `error: cannot read rustdoc JSON` | 2 |
| 値の無い `--rustdoc-dir` | `usage:` | 2 |
| `--rustdoc-json` との併用 | `usage:` | 2 |
| `--cfg-only` との併用 | `usage:` | 2 |

回帰テスト（`scripts/test-wasm-build-check.sh`）との対応:

| 観点 | 行 |
| ---- | -- |
| 依存の crate の型と wasm-abi の型を同じ呼び出しで照らす | `dir-dep-and-root` |
| 依存の crate に無い名前・公開でない名前 | `dir-dep-missing`・`dir-dep-non-public` |
| 置き場に JSON の無い crate 名・crate 名だけ | `dir-crate-not-in-graph`・`dir-crate-name-only` |
| root の JSON の欠落・読めない依存の JSON・存在しないディレクトリ | `dir-without-root-json`・`dir-bad-dep-json`・`dir-missing` |
| 偽の cargo への呼び方（`doc`・feature・`rustdoc` を呼ばない） | `fake-doc-features` |
| 偽の cargo 経由の依存の型・`cargo doc` の失敗 | `fake-dep-type`・`fake-doc-fails` |
| 引数の誤用 | `misuse-rustdoc-dir-no-value`・`misuse-rustdoc-json-and-dir`・`cfg-only-misuse-rustdoc-dir` |
| 実物の一時 workspace の依存のダミー crate（一致・不一致・simd128 だけの型） | `real-dep-type`・`real-dep-type-missing`・`real-dep-cfg-split` |

本リポの現状での実行結果（GPU なしの Linux x86_64）:

```text
$ scripts/wasm-build-check.sh --require-type fandhe_silicon_core::capability::Capabilities --require-type fandhe_silicon_core::capability::CapabilityLevel --require-type fandhe_silicon_core::decision::DecisionInput --require-type fandhe_silicon_core::decision::Decision
type: name=fandhe_silicon_core::capability::Capabilities label=match build=scalar
type: name=fandhe_silicon_core::capability::Capabilities label=match build=simd128
type: name=fandhe_silicon_core::capability::CapabilityLevel label=match build=scalar
type: name=fandhe_silicon_core::capability::CapabilityLevel label=match build=simd128
type: name=fandhe_silicon_core::decision::DecisionInput label=match build=scalar
type: name=fandhe_silicon_core::decision::DecisionInput label=match build=simd128
type: name=fandhe_silicon_core::decision::Decision label=match build=scalar
type: name=fandhe_silicon_core::decision::Decision label=match build=simd128
record: ... artifact=2/2 types=4/4 cfg_sites=6 not_run=0
ok: all rows match
```

終了コードは 0。存在しない名前（`fandhe_silicon_core::capability::Nope`）や crate 名の無い `capability::Capabilities` は `label=mismatch`・`types=0/2` で、終了コードは 1。

確かめ方: `make test-wasm-build-check` の最後の行が `test-wasm-build-check: 104 passed, 0 failed`。

### CI の deny ジョブでの本体の実行（2/3）

`make wasm-build-check` は CI の `deny` ジョブで、`make wasm-verify` の後のステップとして流れる。

| ジョブ | ステップ | 流すもの |
| ------ | -------- | -------- |
| `deny` | `make wasm-build-check` | `scripts/wasm-build-check.sh` に `Makefile` が渡す 4 つの `--require-type`（`fandhe_silicon_core::capability::Capabilities`・`fandhe_silicon_core::capability::CapabilityLevel`・`fandhe_silicon_core::decision::DecisionInput`・`fandhe_silicon_core::decision::Decision`） |

手元の出力（GPU なしの Linux x86_64）:

```text
$ make wasm-build-check
artifact: kind=scalar label=match simd128=absent bytes=300 sha256=207eab239a9a837827ce9fc92a715af8bcdd48aa2c729b2315f85af95b43952a
artifact: kind=simd128 label=match simd128=present bytes=309 sha256=5c97560c5151429e5b122307b90d64cd49140e5c5304abb95508e4eb5602dad9
type: name=fandhe_silicon_core::capability::Capabilities label=match build=scalar
type: name=fandhe_silicon_core::capability::Capabilities label=match build=simd128
type: name=fandhe_silicon_core::capability::CapabilityLevel label=match build=scalar
type: name=fandhe_silicon_core::capability::CapabilityLevel label=match build=simd128
type: name=fandhe_silicon_core::decision::DecisionInput label=match build=scalar
type: name=fandhe_silicon_core::decision::DecisionInput label=match build=simd128
type: name=fandhe_silicon_core::decision::Decision label=match build=scalar
type: name=fandhe_silicon_core::decision::Decision label=match build=simd128
cfg: sites=6 label=match
record: ... artifact=2/2 types=4/4 cfg_sites=6 not_run=0
ok: all rows match
```

終了コードは 0。

確かめ方: CI のワークフローの `deny` ジョブに `run: make wasm-build-check` のステップが 1 件あり、`make ci-frame-check` が `ok: ci workflow frame (8 jobs, rules F1-F6)`。

## 関連

- [wasm32-compile-probe-check](wasm32-compile-probe-check.md)
- 契約の crate の CPU の演算（`cpu_ops`）: [cpu-ops](cpu-ops.md)
