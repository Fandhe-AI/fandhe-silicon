# doctest のエラーコードの照合

doctest の `compile_fail,E0xxx` が、期待したエラーコードでコンパイルに落ちていることを確かめる検査の所在・振る舞い・回帰テスト・確かめ方。

## 背景

- stable の rustdoc は `compile_fail,E…` のエラーコードを照合しない（unstable の扱い）。期待と別のエラーで落ちても、通常の `cargo test` では doctest が合格する
- `RUSTC_BOOTSTRAP=1` を付けて `cargo test --doc` を流すと照合が効く。誤ったコード（例: E0999）の doctest は、stable では通り、`RUSTC_BOOTSTRAP=1` では落ちる
- `compile_fail,E…` を持つ doctest は `crates/core`（`binding.rs`・`device.rs`・`execution/compute.rs`・`execution/submit.rs`・`graphics.rs`・`graphics/draw_range.rs`・`graphics/pass_state.rs`・`graphics/surface.rs`・`interop.rs`・`memory/slice.rs`・`render_target.rs`・`sampler.rs`・`spirv.rs`・`texture.rs`・`texture/copy_spec.rs`・`texture/ops.rs`・`texture/view.rs`）、`crates/contract/src/memory_free_doctest.rs`、`crates/upper`（`buffer.rs`・`device.rs`・`encoder.rs`・`pass.rs`）にある
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

## 今の値の補い（base 801a89d）

上の節の表と文は記録時点の値で、書き換えていない。今の値は次のとおり（実行環境は GPU なしの Linux）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 背景の「`compile_fail,E…` を持つ doctest」の一覧 | `crates/core` の 17 ファイル、`crates/contract/src/memory_free_doctest.rs`、`crates/upper` の 4 ファイル → `crates/core/src` にさらに `memory/mapped_range.rs`（`E0499`）。`crates/contract/src` にさらに `cfg_gate_doctest.rs`（feature `cpu-ops` が有効なときの 1 件が `E0004`。そのほかの `compile_fail` は上の説明どおりコードなし）。`crates/core/tests/graphics_ops_doctest_shape.rs` は `compile_fail,E0046` の文字列を読む統合テストで、doctest ではない | `grep -rl 'compile_fail,E' crates`、`grep -n 'compile_fail' crates/contract/src/cfg_gate_doctest.rs` |
| 同・「エラーコードを指定しない `compile_fail` の doctest」の一覧に挙げた `cfg_gate_doctest.rs` | コードなしのみ → 上のとおり `cpu-ops` が有効な組では `E0004` の 1 件を持つ。`--workspace --all-features` の組で照合が効く | 同上 |
| 確かめ方の末尾の文（期待のコードと一致） | 全部の組で一致 → 同じ。`make doctest-error-code-check` は終了コード 0（`doctest error codes ok`） | `make doctest-error-code-check` |
