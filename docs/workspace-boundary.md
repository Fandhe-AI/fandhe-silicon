# 製品 workspace の境界

製品 workspace に何を入れるかの境界を、本リポで公開済みの事実とポインタで示す。
持ち込む・除くものの一覧と理由の正は private spec 側にあり、本リポには転記しない。

## 製品 workspace の構成

- `members` は `crates/core`・`crates/contract`・`crates/exec`・`crates/upper` の 4 つ（正はルート `Cargo.toml`）
- member の増減と、測定専用の crate・ハーネスの置き場は、private spec の TASK-1・TASK-6 の決定の記録に従う（`docs/spec/05-tasks.md`。本リポには内容を転記しない）
- 測定専用の別 workspace `measure/` はルート `Cargo.toml` の `exclude` にあり、製品の member・依存グラフに入らない（TASK-6・TASK-27 のポインタ。所在と現状の事実は [measurement-tool](measurement-tool.md)）
- wasm32 向けの別 workspace `wasm/` もルート `Cargo.toml` の `exclude` にあり、製品の member・依存グラフに入らない。依存の向きは `wasm/` → 製品（`crates/contract` への path）だけで、製品の crate が `wasm/` を参照すると `make deny` の path の検査が落ちる（TASK-22 のポインタ。所在と現状の事実は [wasm32-node-foundation](wasm32-node-foundation.md)）
- これ以外の workspace のコードを、member や path 依存として加えない

## 依存グラフから外すもの

- `wgpu`・`wgpu-core`・`wgpu-hal`・`wgpu-types`・`naga` は `deny.toml` の `[bans] deny` で拒否する（D-18、AC-2 のポインタ）
- 検出は `make deny` と CI の `deny` ジョブで行う（別 workspace の `baseline-wgpu/` は wgpu を持つため、禁止を外した専用の設定で監査する）
- 比較基準の wgpu は基盤の外の別 workspace に置く（TASK-7 のポインタ。所在と現状の事実は [baseline-wgpu](baseline-wgpu.md)）
- 比較用の run に残す wgpu の版の記録の書式は [wgpu-baseline-record](wgpu-baseline-record.md) に示す

## 外部の workspace からコードを持ち込むとき

- 持ち込む・持ち込まないの一覧とその理由の正は、private spec の TASK-6（`docs/spec/05-tasks.md`）の決定の記録である。本リポには一覧を転記しない
- 決定の記録に無いものは持ち込まない
- 持ち込む PR では、PR 本文に TASK-6 のポインタを書く
- 依存の追加を伴う場合は [dependency-policy](../.claude/rules/dependency-policy.md) の承認フローに従う

## 関連

- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)
- [dependency-policy](../.claude/rules/dependency-policy.md)
- [docs/contract/procedure.md](contract/procedure.md)
