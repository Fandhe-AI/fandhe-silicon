# doctest のエラーコードの照合

doctest の `compile_fail,E0xxx` が、期待したエラーコードでコンパイルに落ちていることを確かめる検査の所在・振る舞い・回帰テスト・確かめ方。

## 背景

- stable の rustdoc は `compile_fail,E…` のエラーコードを照合しない（unstable の扱い）。期待と別のエラーで落ちても、通常の `cargo test` では doctest が合格する
- `RUSTC_BOOTSTRAP=1` を付けて `cargo test --doc` を流すと照合が効く。誤ったコード（例: E0999）の doctest は、stable では通り、`RUSTC_BOOTSTRAP=1` では落ちる
- `compile_fail,E…` を持つ doctest は `crates/core`（`binding.rs`・`device.rs`・`execution/compute.rs`・`execution/submit.rs`・`graphics.rs`・`interop.rs`・`memory/slice.rs`・`render_target.rs`・`sampler.rs`・`spirv.rs`・`texture.rs`・`texture/copy_spec.rs`・`texture/ops.rs`・`texture/view.rs`）、`crates/contract/src/memory_free_doctest.rs`、`crates/upper`（`buffer.rs`・`device.rs`・`encoder.rs`・`pass.rs`）にある
- エラーコードを指定しない `compile_fail` の doctest（`crates/core` の `device_lost.rs`・`memory/ops.rs`・`parity/pixel.rs`・`parity/tolerance.rs`、`crates/contract` の `callback_send_doctest.rs`・`cfg_gate_doctest.rs`）は、`RUSTC_BOOTSTRAP=1` でも失敗の理由を照合しない。コンパイルが通らないことだけを確かめる

## 所在

| 物 | 場所 |
| -- | ---- |
| 検査本体 | `scripts/doctest-error-code-check.sh`（`make doctest-error-code-check`） |
| 回帰テスト | `scripts/test-doctest-error-code-check.sh`（`make test-doctest-error-code-check`） |
| CI | `ci.yml` の `deny` ジョブの 1 ステップずつ（ジョブは増やしていない） |

## 振る舞い

- `RUSTC_BOOTSTRAP=1` は、スクリプトの中で doctest を流す `cargo test --doc` にだけ付ける。呼び出し元の環境・ほかのビルドや試験には付けない
- 流す組: `fandhe-silicon-core` を feature なし・`test-support`・`graphics`・`test-support,graphics` で、続けて `--workspace --all-features`（`crates/contract` の `memory_free_doctest` は `test-support` の裏にあり、この組で流れる）
- `--manifest-path <Cargo.toml>` を渡すと、その crate だけを `--all-features` で 1 回流す（回帰テスト用）
- 終了コード: 0 全部通った / 2 誤用または cargo が無い / それ以外の非 0 は `cargo test` の終了コード（doctest が落ちた。コードの食い違いを含む）
- ソースは変更しない。ネットワークは要らない

## 回帰テストの内容

実行時に作る依存の無い一時 crate（値の移動後の使用 E0382 を起こす doctest）で確かめる。

| 確かめること | 期待 |
| ------------ | ---- |
| 正しいコード（E0382） | 通る |
| 誤ったコード（E0999） | 落ち、出力に E0999 が出る |
| 対照: 素の stable の `cargo test --doc` で誤ったコード | 通る（検査が要る理由） |
| 未知の引数 | 終了コード 2 |

## 確かめ方

```bash
make doctest-error-code-check
make test-doctest-error-code-check
```

本リポの doctest の `compile_fail,E…` は、上の組のすべてで期待のコードと一致している。
