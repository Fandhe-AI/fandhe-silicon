# feature ゲートのコンパイル失敗の試験（TASK-233）の所在と本リポの現状

TASK-233（親 Issue #460）の試験の所在と、本リポのコードで確かめられる現状を案内する。
spec の本文・内部判断は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

- 正は private spec 側にある（ポインタ: `docs/spec/05-tasks.md` の TASK-233。関連: `docs/spec/04-behavior/build-profiles.md` の BUILD-11・BUILD-12・BUILD-13）
- 進み具合・前提の扱いは private spec 側の記録に従う（ポインタのみ）

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| feature の宣言 | `native-blocking-wait`・`ext-interop` は `crates/contract/Cargo.toml` に宣言済み。`default` には入らず、他の feature の有効化リストにも現れない | `cargo test -p fandhe-silicon-contract --test feature_gate_scope` |
| コンパイル失敗の仕組み | 外部 crate を足さず、rustdoc の doctest の fence を `cfg_attr` で切り替える（feature なしは `compile_fail`、ありは通る）。置き場は `crates/contract/src/cfg_gate_doctest.rs`（doctest の収集のため `pub mod` にし、`#[doc(hidden)]` で文書の表示だけを隠している。外部 crate から参照はできる） | `cargo test -p fandhe-silicon-contract --doc` と `--features backend-metal` の両方 |
| 仕組みの対照 | 実在の feature ゲート `backend-metal` の `backend_metal` モジュールの対と、`native-blocking-wait`・`ext-interop` それぞれの門の目印（値を持たない型）の対。目印は枠の確かめにとどまり、対象のアイテム自体の確かめではない | 同上と `--features native-blocking-wait`・`--features ext-interop` |
| 有効側の試験 | feature を有効にした構成で目印が見えること。置き場は `crates/contract/tests/feature_gate_enabled.rs` | `cargo test -p fandhe-silicon-contract --all-features --test feature_gate_enabled` |
| 流れる場所 | feature なし側は `scripts/build-profiles.sh` の組、ありの側は `make verify`（`--all-features`）と、`scripts/build-profiles.sh` の `native-blocking-wait` 単独・`ext-interop` 単独の組（build / clippy / test）。どちらも `verify` ジョブが呼ぶ | `make build-profiles`、`make verify` |
| wasm32 の組 | `cargo check` のみで doctest は流れない | `scripts/build-profiles.sh` |

## 段

| 段 | Issue | 内容 |
| -- | ----- | ---- |
| 1/4 | #3928 | 試験の枠（本書の仕組みと feature の範囲の試験） |
| 2/4 | #3929 | 有効側の試験 |
| 3/4 | #3930 | CI への組み込み |
| 4/4 | #3931 | 完了の条件の確かめ |

## CI への組み込み（3/4）

- `scripts/build-profiles.sh` の組に `native-blocking-wait` 単独と `ext-interop` 単独を足し、feature ごとに build / clippy（`-D warnings`）/ test（doctest を含む）を流す。新しいジョブ・ターゲットは足さず、`verify` ジョブの `make build-profiles` が呼ぶ
- 単独の組の出力の見え方: `native-blocking-wait` 単独では `native_blocking_wait_gate_is_visible` と `NativeBlockingWaitGate` の doctest が通り、他の 2 つの doctest は `compile fail ... ok`。`ext-interop` 単独では `ext_interop_gate_is_visible` と `ExtInteropGate` が通り、他の 2 つが `compile fail ... ok`
- 手元の確かめ: `cargo test -p fandhe-silicon-contract --no-default-features --features native-blocking-wait`（`ext-interop` も同様）、全体は `make build-profiles`

## 本リポでの反映の方針

- 対象の機能を使うコードの snippet は、そのコードを持つ PR で `cfg_gate_doctest.rs` に同じ形で足す（コードより先に具体を書かない）
- 手順は [contract/procedure](contract/procedure.md) に従う

## 関連

- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)
