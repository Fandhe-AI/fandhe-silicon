# WebGPU の口に当たる能力の問合せの行の所在と現状の事実（TASK-155）

本リポのコードと公開テストから観測できる事実だけを記録する。記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-155。対象: DEV-35・DEV-36。ポインタのみ）。

## 所在

| 物 | 場所 | 内容 |
| -- | ---- | ---- |
| 鍵 2 件 | `crates/core/src/capability.rs` の `WEBGPU_DEVICE_PORT`・`WEBGPU_HANDLE_PORT` | `FeatureId`。文字列は定数の定義を正とする |
| 返す値 | 同 `WEBGPU_PORT_UNSUPPORTED` | 段階 `Unsupported`・理由 `NoApi`・根拠なし |
| 共通の検査 | `crates/core/src/capability_rule.rs` の `check_webgpu_ports`（feature `test-support`） | 2 件の鍵で `query` と `query_ex` が上の値かを確かめ、違反を全部返す |

上の 3 件は定数で、関数ではない。

## 対応するテスト

`crates/core/tests/capability_webgpu_ports.rs`（`cargo test -p fandhe-silicon-core --features test-support --test capability_webgpu_ports`）。DEV-35 と DEV-36 の名前の付いたテストが、鍵と値、共通の規則（`check_query`）の通過、行の欠落（`S0`）・理由違い・根拠付き・`query` と `query_ex` の食い違い（違反の中身を含む）・`query` だけ `Unsupported` を返す実装の拒否を確かめる。

## 現状

- 2 件の鍵に上の値を求める規則を `check_webgpu_ports` が確かめる。core の試験は表を持つスタブ（`CapabilityTable`）などの実装に当てる
- 公開の名前の検査 `scripts/public-api-webgpu-check.sh` の範囲は [public-api-webgpu-check](public-api-webgpu-check.md) に従う
