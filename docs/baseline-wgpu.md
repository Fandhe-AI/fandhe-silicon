# 比較基準の wgpu の置き場（現状と前提）

比較基準の wgpu を基盤の外の別 workspace に置く作業（TASK-7、関連: AC-2・BUILD-50・D-18）について、
現状と前提を、本リポで公開済みの事実とポインタだけで記録する。
spec の本文・値は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
版と実測の値は、本リポの `baseline-wgpu/` の manifest・`Cargo.lock`・実行結果から取ったものだけを書く。

## 前提タスクの状態

| 前提 | 状態 | 根拠 |
| ---- | ---- | ---- |
| TASK-4（workspace と crate の骨格） | 済 | `cargo metadata --format-version 1 --all-features` で member は 4 crate、外部パッケージは 0 件。`deny.toml` の `[bans] deny` で wgpu 系を拒否済み |
| TASK-6（PoC から持ち込むもの・除くものの決定） | 済 | 対応する Issue #35 はクローズ済み。決定の記録の正は private spec（[workspace-boundary](workspace-boundary.md)） |

- TASK-7 の完了の条件との対応づけの記録の正は private spec 側（TASK-7 の状態。ポインタのみ）
- 製品 workspace に wgpu を入れない向きは、TASK-6 の決定に左右されない

## 製品 workspace を守る仕組み

- `deny.toml` の `[bans] deny` が wgpu 系を拒否する。検出は `make deny` と CI の `deny` ジョブ
- 境界の案内は [workspace-boundary](workspace-boundary.md) を参照

## 別 workspace の配置と骨格（TASK-7 の 2/4）

- 置き場は本リポ内の非 member ディレクトリ `baseline-wgpu/`。ルートの `Cargo.toml` の `exclude` で切り離し、自身の `[workspace]` と `Cargo.lock` を持つ
- smoke crate が 1 つ。外部依存は wgpu のみ（下の「wgpu の依存追加と版の記録」）。詳細は [baseline-wgpu/README.md](../baseline-wgpu/README.md) を参照
- `make deny`・`make verify` はルートの workspace が対象で、本 workspace は範囲外のまま。別 workspace を検査する仕組み（検査と CI ジョブ）は未整備

## wgpu の依存追加と版の記録（TASK-7）

`baseline-wgpu/` に `wgpu = "=30.0.1"` を追加した（feature は既定のまま）。承認の記録は `baseline-wgpu/deps-approved.toml`
（`approval_ref = "#40"`）に置く。ルートの `deps-approved.toml` に入れないのは、ルートの workspace では使われない記録が違反になるため。
本 workspace の検査は手動: `scripts/deps-check.sh baseline-wgpu/Cargo.toml`（`make deps-check` と CI はルートのみ。CI には組み込まない）。

版の記録（[wgpu-baseline-record](wgpu-baseline-record.md) の書式に沿った、本リポの実物から取った値。spec 由来の値ではない。`basis` は run の設定から取る欄で、版の置き場の記録には run が無いため空欄）:

| 欄 | 値 |
| -- | -- |
| `wgpu_baseline.version` | `30.0.1` |
| `wgpu_baseline.source` | `crates-io` |
| `wgpu_baseline.lockfile_sha256` | `966b73974affc4ed4f3a215e941f7e1d0674b8f900dfc2c3afb3919e3a519d44`（`baseline-wgpu/Cargo.lock`） |
| `wgpu_baseline.basis` | 未記入（run の設定から取る欄） |

補足（公開情報）:

- crates.io の公開日: 2026-08-22。ライセンス: `MIT OR Apache-2.0`。MSRV: 1.87
- `source` が `crates-io` のため、`git_repository`・`git_tag`・`git_commit` の欄は使わない

### ローカル実測（2026-10-11。CI では実行していない）

| 項目 | 結果 |
| ---- | ---- |
| `cargo build`・`cargo clippy --all-targets -- -D warnings`・`cargo fmt --check` | いずれも通過 |
| 間接依存の数（重複を除く crate 数、wgpu 自身と smoke を含む） | 実行ホストの target では 64、`--target all` では 117、`Cargo.lock` の package は 125 |
| ライセンス | `cargo deny`（ルートの `deny.toml` から wgpu 系の禁止だけ外した一時設定）の licenses・sources・bans・advisories がすべて ok。MIT / Apache-2.0 と互換でないもの・ライセンス欄が空のものは 0 件 |
| ビルドに要ったシステムライブラリ | 追加で要ったものは無い（Linux・Rust stable でビルドできた） |

`cargo run` の出力（GPU を持たない Linux ホスト。ソフトウェア実装のドライバだけがある。ホスト名などは含まない）:

```text
wgpu adapters: 2 adapter(s)
[0] name="llvmpipe (LLVM 21.1.8, 256 bits)" backend=Vulkan device_type=Cpu
[1] name="llvmpipe (LLVM 21.1.8, 256 bits)" backend=Gl device_type=Cpu
```

物理 GPU が無い環境でもアダプタが 0 件になるとは限らない。0 件のときは `0 adapter(s)` を出して正常終了する作りで、
この 0 件の経路は実機では未確認。

## 残っている論点

完了の条件との対応づけの記録の正は private spec 側（TASK-7 の状態。ポインタのみ）。本節には、それ以外に残っている論点と事実を書く。

1. 置き場の最終確認: 本リポ内の `baseline-wgpu/` を置き場として smoke crate を置いた。別リポジトリや PoC の隔離 workspace への移行をオーナーが望む場合は再判断する
    - 別 workspace を `make deny`・`make verify` の範囲に含める検査と CI ジョブは未決
2. wgpu の依存追加は、オーナーの明示承認（2026-10-11、#40。追加先は `baseline-wgpu/` に限る）を得て `=30.0.1` で追加済み（[dependency-policy](../.claude/rules/dependency-policy.md)）。以後の追加・更新は改めて承認を得る

## 後続の分担

| 作業 | 担当 | 状態 |
| ---- | ---- | ---- |
| 別 workspace の作成 | TASK-7 の 2/4 | 済 |
| 版の記録の書式 | TASK-7 の 3/4 | 済（[wgpu-baseline-record](wgpu-baseline-record.md)） |
| wgpu の依存追加と版の記録 | TASK-7（本書の該当節） | 済 |
| 完了の条件の確かめ | TASK-7 の 4/4 | 下の節に記録（対応づけの記録の正は private spec 側。TASK-7 の状態） |

## 完了の条件の確かめ

本リポで観測できる事実による確かめの記録（ポインタ: TASK-7・AC-2・BUILD-50・D-18）。
spec の完了の条件との対応づけの記録の正は private spec 側（TASK-7 の状態。ポインタのみ）。条件の本文は転記しない。

| 確かめる項目 | 方法 | 現時点の結果 |
| ------------ | ---- | ------------ |
| 製品 workspace の依存グラフに wgpu 系・naga が無い | `make deny`、`cargo metadata --format-version 1 --all-features --locked` | 通過。member 4・パッケージ 4・該当 0 件 |
| 品質ゲート | `make verify`、`make check` | 通過 |
| 別 workspace が製品 workspace の外にあり、member・path 依存になっていない | `make deny` の path 検査、`cargo metadata` | 通過（`make deny` は crates/ の外を指す path 依存を検出しない。wgpu 追加後もパッケージ 4・wgpu 系 0 件） |
| 比較基準の版の記録が決めた書式で残っている | 本書の「wgpu の依存追加と版の記録」 | 記録済み（`basis` は run の設定から取る欄のため未記入） |
| 別 workspace から wgpu を使う bin がビルド・実行できる | `baseline-wgpu/` で `cargo build`・`cargo run` | 通過（ローカル実行。CI では未実行。出力は「ローカル実測」の節） |
| 前提の TASK-6 が確定している | 対応する Issue の状態 | Issue #35 はクローズ済み |

- 判定: 本リポで確かめられる項目は通過。完了の条件との対応づけの記録の正は private spec 側（TASK-7 の状態。ポインタのみ）
- 再実行の契機: 別 workspace の依存の追加・更新、置き場の変更のあと、同じ手順で表を更新する
- 確かめた日付: 2026-10-11（base: 6d105dc）
