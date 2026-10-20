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
| 契約の crate の CPU 向けの feature | まだコードを持たない | `crates/contract/src` |
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

## CI への組み込み（4/4）

既存のジョブへステップとして足した。新しいジョブ・runner・ツールは無く、check 名と `ci-complete` の `needs` は変わらない。

| ジョブ | ステップ | 流すもの |
| ------ | -------- | -------- |
| `verify` | `make build-profiles` | `scripts/build-profiles.sh` の末尾で `crates/wasm-abi` の scalar 版・simd128 版を release でビルドし、成果物の在りかと wasm の先頭 4 バイトを確かめる（BUILD-19・BUILD-20 のポインタ）。利用者の rustflags 系の環境変数は外し、simd128 版にだけ `+simd128` を渡す。target が無ければ失敗する |
| `deny` | `make test-wasm-build-check` | `scripts/wasm-build-check.sh` の回帰テスト（偽の cargo と一時 workspace のダミーの crate。ネットワーク不要） |

`make wasm-build-check` の本体は CI から呼ばない。本リポの現状では `type:` に渡せる公開の型の名前が `crates/` に無く、`cfg:` の対象の `simd128` の出現も 0 件で、どちらも not-run になり終了コード 1 だから。
ダミーの型や `cfg` を足す、not-run を許す口を足すといった合格に見せる変更はしない。

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
