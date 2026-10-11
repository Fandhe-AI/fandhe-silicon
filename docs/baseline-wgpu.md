# 比較基準の wgpu の置き場（現状と前提）

比較基準の wgpu を基盤の外の別 workspace に置く作業（TASK-7、関連: AC-2・BUILD-50・D-18）について、
着手時点の現状と前提を、本リポで公開済みの事実とポインタだけで記録する。
spec の本文・比較基準の版・計測値は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 前提タスクの状態

| 前提 | 状態 | 根拠 |
| ---- | ---- | ---- |
| TASK-4（workspace と crate の骨格） | 済 | `cargo metadata --format-version 1 --all-features` で member は 4 crate、外部パッケージは 0 件。`deny.toml` の `[bans] deny` で wgpu 系を拒否済み |
| TASK-6（PoC から持ち込むもの・除くものの決定） | 未完了 | 親 Issue 配下の一部の sub-issue が open。決定の記録の確定はオーナー判断待ち |

- TASK-6 が確定するまで、TASK-7 の完了は宣言しない。TASK-6 を満たされたものとして扱わない
- 製品 workspace に wgpu を入れない向きは、TASK-6 の未決の論点に左右されない

## 製品 workspace を守る仕組み

- `deny.toml` の `[bans] deny` が wgpu 系を拒否する。検出は `make deny` と CI の `deny` ジョブ
- 境界の案内は [workspace-boundary](workspace-boundary.md) を参照

## 別 workspace の配置と骨格（TASK-7 の 2/4）

- 置き場は本リポ内の非 member ディレクトリ `baseline-wgpu/`。ルートの `Cargo.toml` の `exclude` で切り離し、自身の `[workspace]` と `Cargo.lock` を持つ
- 骨格は外部依存 0 の smoke crate のみ。比較基準は稼働していない。詳細は [baseline-wgpu/README.md](../baseline-wgpu/README.md) を参照
- `make deny`・`make verify` はルートの workspace が対象で、本 workspace は範囲外のまま。別 workspace を検査する仕組み（検査と CI ジョブ）は未整備

## オーナー判断待ちの論点

推奨案は確定しない。決まるまで後続の作業は仮置きで進めない。

1. 置き場の最終確認: 本リポ内の `baseline-wgpu/` を置き場として骨格を作成済み。別リポジトリや PoC の隔離 workspace への移行をオーナーが望む場合は再判断する
    - 別 workspace を `make deny`・`make verify` の範囲に含める検査と CI ジョブは未決
    - 止まるもの: 版の記録の書式、完了の条件の確かめ
2. wgpu の依存追加の承認（`=x.y.z` 完全固定。[dependency-policy](../.claude/rules/dependency-policy.md)）は未承認。承認まで依存は追加しない
3. TASK-6 の決定の記録の確定は未完了のまま。TASK-7 の完了は宣言しない

## 後続の分担

| 作業 | 担当 |
| ---- | ---- |
| 別 workspace の作成 | TASK-7 の 2/4 |
| 版の記録の書式 | TASK-7 の 3/4 |
| 完了の条件の確かめ | TASK-7 の 4/4 |

2/4・3/4 は本文書の該当節へ追記する。
