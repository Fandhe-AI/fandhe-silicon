# DX12 の compute に関する点検の記録（TASK-236）の所在・方針・形（案）

TASK-236（親 Issue #461）の点検の記録について、置き場・書き手・本リポへの映し方の方針と、本リポ側の文書の形を、本リポで公開済みの事実とポインタだけで記録する。
本書は案で、オーナーの確認は済んでいない。
spec の本文・点検の対象の名前・候補・結果・件数は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
着手時点の現状と前提は「着手時点の本リポの事実」「関連タスクの状態」の節に記録する（1/8、Issue #3932）。

## 記録の所在

- 点検の記録（表・結果）の正は private spec 側にある（ポインタ: `docs/spec/05-tasks.md` の TASK-236。対象の項目は `docs/spec/04-behavior/build-profiles.md` の BUILD-18・BUILD-64）
- 本リポには表・結果・件数を置かない（[契約の文書の分担](contract/README.md)）
- 前提タスクの有無と完了の条件は TASK-236 を参照する（ポインタのみ）
- 完了の条件との対応づけと進み具合の記録の正も private spec 側（TASK-236 の状態。ポインタのみ）

## 方針（2/8）が前提にする本リポの事実

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| DX12 の置き場 | `backend-dx12-compute` は中身の無い仮の feature。受け持つタスクのポインタ付きのコメントがある | `crates/contract/Cargo.toml` |
| DX12 のコード | `crates/` の `*.rs` に DX12 の記述は無い。背後実装のモジュールは Metal・Vulkan・CUDA の 3 つ | `grep -rni dx12 crates --include='*.rs'`、`ls crates/contract/src` |
| 契約の文書 | `docs/contract/` は書式・手順書だけで、領域ごとの文書は無い | `ls docs/contract` |
| spec の取り込み | `docs/spec` は gitlink（`update = none`）。CI は取得しない | `.gitmodules`、`git ls-tree HEAD docs/spec` |

- 確かめた日付: 2026-10-12（base: `3e78751`、gitlink: `c1cb4c9`）
- 3/8 の形の設計の確かめた日付: 2026-10-12（base: `460ef21`、gitlink: `c1cb4c9`）。形の設計は既存の規約と文書だけから導いており、`docs/spec` は読んでいない

## 着手時点の本リポの事実

確かめた日付: 2026-10-12（base: `460ef21`、gitlink: `c1cb4c9`）。上の「方針が前提にする本リポの事実」を補う。本範囲では点検そのものを行わない。上記のポインタが gitlink の指す commit に見出しとして在るかは確かめていない（未検証）。

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| DX12 の compute の feature | `fandhe-silicon-contract` に `backend-dx12-compute` があり、`contract-core` を有効にするだけ。`default` には入っていない。コメントに受け持つタスクのポインタ（TASK-222）がある | `crates/contract/Cargo.toml` の `[features]` |
| DX12 のモジュール | `crates/contract/src/lib.rs` に DX12 のモジュール宣言は無く、`crates/contract/src/` に DX12 のファイルも無い | `crates/contract/src/lib.rs`、`ls crates/contract/src` |
| build-profiles の組 | `backend-dx12-compute` を含む組は `NATIVE_SETS`・`WASM_SETS` に無い。`make verify` は `--all-features` なので feature 自体は有効になる | `scripts/build-profiles.sh`、`scripts/verify.sh` |
| 製品の公開面 | 製品 crate の公開面はほぼ空で、点検の対象になる操作・型は本リポに無い | 各 crate の `src/lib.rs` |
| 領域ごとの契約の文書 | `docs/contract/` には書式・手順書だけがあり、領域ごとの文書は無い | `ls docs/contract` |
| `docs/spec` | gitlink で `update = none`。既定の clone では空で、`ci.yml` も取得しない | `.gitmodules`、`git ls-tree HEAD docs/spec`、`.github/workflows/ci.yml` |
| 外部依存 | 0 件。追加は承認制 | `cargo metadata --format-version 1 --all-features --locked`、[dependency-policy](../.claude/rules/dependency-policy.md) |
| 外部の一次資料の調べ方 | Direct3D の調査は reference-researcher が `windows-graphics-media` スキルで受け持つ | `.claude/agents/research/reference-researcher.md`、`skills-lock.json` |
| 持ち込みと spec の扱い | PoC の記録は本リポへ写さない。`docs/spec` 配下は書き換えない | [poc-carry-over](../.claude/rules/poc-carry-over.md)・[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md) |

## 関連タスクの状態

`crates/contract/Cargo.toml` のコメントが指すタスクに限る。spec 内の依存の関係は書き起こさない。

| 対象 | Issue | 状態（2026-10-12 時点） |
| ---- | ----- | ------------------------ |
| TASK-236（親） | #461 | OPEN |
| TASK-222 | #1145 | OPEN |
| TASK-234 | #1154 | OPEN |

## 本リポへの反映の方針（2/8）

- 点検の記録は private spec 側に置き、本リポには置かない。根拠は、記録の内容が本リポのコード・公開テストから説明できない spec 由来の情報であること（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)、[契約の文書の分担](contract/README.md)）
- PoC の記録は書き換えない・写さない（[poc-carry-over](../.claude/rules/poc-carry-over.md)）
- spec 側の置き場・書式は spec リポ側の作業。本リポから出すのは提案までで、`docs/spec` 配下と gitlink は変えない（[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md)）
- 本作業で feature・モジュール・テスト・領域ごとの契約の文書を足さない。`backend-dx12-compute` の中身は `crates/contract/Cargo.toml` のコメントが指すタスクの受け持ち
- 点検の入力は gitlink の指す commit の spec とし、確かめの記録には base と gitlink の短い sha を残す
- 外部仕様の確認は reference-researcher に委譲し、`windows-graphics-media` スキルを先に当たる。調べた結果の本体も spec 側に置く
- 見直しが要る点が見つかった場合は、文書・テストで黙って寄せない。contract-shaping-escalation の差し戻しの手順に従い、本リポには ID のポインタと「spec 側の判断待ち」の事実だけを書く
- 後続の確かめの段（4/8・5/8・8/8）の本リポ側の作業は、本リポで観測できる事実による確かめの記録とする（[implementation-scope](implementation-scope.md) の「完了の条件の確かめ」の流儀）

## 2/8 と 3/8 の分け方

| 段 | 受け持ち |
| -- | -------- |
| 2/8（Issue #3933） | 方針: 記録の置き場・書き手・本リポへの映し方・伺う経路・後続への引き継ぎ |
| 3/8（Issue #3934） | 形: 記録の行の単位・列・結果の書き方・ファイルの構成（本体は spec 側の提案。本リポには節の構成とポインタだけ） |

## 形の設計（3/8）

本リポ側の文書の形だけを決める。spec 側の記録の本体の形は書かない。既存の決定から一意に導ける点を、根拠のポインタ付きで記録する。

| # | 点 | 形 | 根拠 |
| - | -- | -- | ---- |
| F1 | spec 側の記録の本体の形 | 行の単位・列・結果の書き方・ファイルの構成は spec 側で決める。本書には書かない | [spec-confidentiality](../.claude/rules/spec-confidentiality.md)、[契約の文書の分担](contract/README.md)、本書「記録の所在」 |
| F2 | PoC の記録との関係 | 既存の PoC の記録は書き換えず・写さない。置き場は spec 側が決める | [poc-carry-over](../.claude/rules/poc-carry-over.md) |
| F3 | 本リポの文書の構成 | 本書 1 ファイルに閉じる。新しい文書・領域ごとの契約の文書・feature・モジュール・テストは足さない | [契約の文書の分担](contract/README.md)、[implementation-scope](implementation-scope.md) |
| F4 | 節の順 | 記録の所在 → 方針 → 形の設計 → 確かめの記録（段ごとの小見出し）→ オーナー判断待ち → 後続の分担 → 関連。各段は自分の節だけを足し、他の節の文言を動かさない。空の節・空の表は先に作らない | [implementation-scope](implementation-scope.md)、[parity-tool](parity-tool.md) の節の並び |
| F5 | 確かめの記録の表 | 列は「確かめる項目 / 方法 / 現時点の結果」。方法は再実行できるコマンドか参照先のパスで書く | [implementation-scope](implementation-scope.md) の「完了の条件の確かめ」 |
| F6 | 結果の語彙 | 「成り立つ / 成り立たない」「在る / 無い」「通過 / 失敗」「未検証（理由）」「本リポからは確かめない」に限る。未実施は理由付きで書き、済と書かない。spec 側の点検の対象の名前・判定・件数・理由は書かない | [spec-confidentiality](../.claude/rules/spec-confidentiality.md)、[coding-rust](../.claude/rules/coding-rust.md) のテスト節 |
| F7 | 版の記録 | 段ごとに、確かめた日付・base の短い sha・gitlink の短い sha を 1 行で残す | 本書「本リポへの反映の方針（2/8）」 |
| F8 | 外部仕様の確かめ | 本リポには reference-researcher へ委譲した事実と、結果の所在が spec 側であることだけを書く | 本書「本リポへの反映の方針（2/8）」 |
| F9 | 見直しの提案 | 本リポには ID のポインタ・「spec 側の判断待ち」・「保留」だけを書く | [contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md)、[out-of-scope-tracking](../.claude/rules/out-of-scope-tracking.md) |
| F10 | 検査 | 文書だけなので `make check` で足りる。make ターゲット・CI ジョブ・スクリプトは足さない | [makefile](../.claude/rules/makefile.md)、[github-actions](../.claude/rules/github-actions.md) |

## 確かめの記録

本リポで観測できる事実だけを記録する（F5〜F7）。点検の記録（表・結果）の正は private spec 側で、本書には置かない（ポインタ: `docs/spec/05-tasks.md` の TASK-236）。

### 4/8（Issue #3935）

範囲は TASK-236 の内容の (1)（対象の項目: `docs/spec/04-behavior/build-profiles.md` の BUILD-18）。(2)（BUILD-64）は 5/8（Issue #3936）の受け持ち。

| # | 確かめる項目 | 方法 | 現時点の結果 |
| - | ------------ | ---- | ------------ |
| 1 | 前段（1/8〜3/8）が済んでいる | Issue #3932・#3933・#3934 の状態 | 成り立つ（いずれも CLOSED） |
| 2 | TASK-236 が gitlink の commit の `docs/spec/05-tasks.md` に見出しとして在る | gitlink の commit の当該ファイルを `gh api` で読み取り、見出しを検索する（取得物は残さない） | 在る |
| 3 | BUILD-18 が gitlink の commit の `docs/spec/04-behavior/build-profiles.md` に在る | 同上 | 在る |
| 4 | 「方針（2/8）が前提にする本リポの事実」の表がいまも成り立つ | `crates/contract/Cargo.toml`、`grep -rni dx12 crates --include='*.rs'`、`ls crates/contract/src`、`ls docs/contract` | 成り立つ |
| 5 | 製品 crate の公開面に DX12 の compute に関する操作・型が在る | `grep -rnE 'pub (fn\|struct\|enum\|trait\|type\|const\|static\|mod\|use)' crates/*/src`、`crates/contract/src/lib.rs` | 無い（`contract` の公開面は Metal・Vulkan・CUDA のモジュール宣言だけ。`core` の公開項目は `test-support` の裏の比較の道具 `parity` で、DX12 に関する記述は無い。`exec`・`upper` に公開項目は無い。本リポのコードを対象にした点検は行わない） |
| 6 | 範囲 (1) の点検の記録（表）そのもの | 本リポからは確かめない | 本リポからは確かめない（正は private spec 側。書き手は論点 1 の判断待ち） |
| 7 | 外部仕様の確かめ（F8） | 本段では reference-researcher へ委譲していない | 未検証（理由: 結果の置き場が spec 側で、書き手が論点 1 の判断待ち） |
| 8 | 見直しの提案（F9）で本リポから挙げたもの | 本書の「オーナー判断待ち」 | 無い（新しい論点は足していない） |
| 9 | 本作業で feature・モジュール・テスト・領域ごとの契約の文書・make ターゲット・CI ジョブを足していない | `git diff --stat origin/main...HEAD`、`ls docs/contract` | 成り立つ |
| 10 | 品質ゲート | `make check` | 通過（`make verify`・`make deny` は未実行。コード・設定の変更が無いため） |

- 判定: 本リポで確かめられる項目に問題は見つからなかった。範囲 (1) の記録の正は private spec 側（ポインタのみ）
- 再実行の契機: gitlink の更新（`update-external.yml` の PR）、`crates/contract/Cargo.toml` のコメントのポインタの変更、製品 crate の公開面の追加
- 版の記録: 2026-10-12（base: `d8e06a1`、gitlink: `c1cb4c9`）

### 5/8（Issue #3936）

範囲のポインタ: TASK-236 の内容の (2)、BUILD-64（`docs/spec/05-tasks.md`、`docs/spec/04-behavior/build-profiles.md`）。本リポで観測できる事実だけを記録する。点検の本体・結果・件数の正は private spec 側で、本書には置かない。

| # | 確かめる項目 | 方法 | 現時点の結果 |
| - | ------------ | ---- | ------------ |
| 1 | 範囲のポインタが gitlink の指す commit に在る | 当該 2 ファイルを一時領域へ読み取りで取得し、ID の見出し・出現を数える（取得物は削除） | 在る |
| 2 | 範囲 (2) の点検の対象になる操作・型が、製品 crate の公開面に在る | 下の「公開面の一覧のコマンド」 | 無い（`pub mod` の 4 件だけ） |
| 3 | `backend-dx12-compute` が中身の無い仮の置き場のまま | `crates/contract/Cargo.toml` の `[features]`、`ls crates/contract/src`、`grep -rn -i dx12 crates scripts .github` | 成り立つ |
| 4 | 「着手時点の本リポの事実」の表がいまも成り立つ | 同表の「確かめ方」の列 | 成り立つ（本範囲で再確認した行は 2・3 に挙げた公開面と DX12 の記述、`docs/contract` の中身） |
| 5 | 範囲 (2) の点検の記録の本体（spec 側） | 本リポからは確かめない | 本リポからは確かめない（書き手と経路は「オーナー判断待ち」の論点 1） |
| 6 | 外部仕様（Direct3D 12）との突き合わせ | 本リポからは確かめない | 未検証（理由: 結果の置き場が spec 側で、書き手が論点 1 の判断待ち。本範囲では reference-researcher へ委譲していない） |
| 7 | 見直しの提案 | 本リポからは確かめない | 本リポ側から新たに挙げるものは無い（点検の本体が未実施のため） |
| 8 | 事前に空の feature・モジュール・領域ごとの文書を足していない | `crates/contract/Cargo.toml`、`ls docs/contract` | 成り立つ |
| 9 | 品質ゲート | `make check` | 通過（`make verify`・`make deny` は未実行。コード・設定の変更なし） |

公開面の一覧のコマンド（`|` を含むので表の外に置く）:

```bash
git grep -n -E '^[[:space:]]*pub(\([a-z]+\))? (fn|struct|enum|trait|type|const|static|mod|use)' HEAD -- 'crates/*/src/*.rs' ':(exclude)crates/core/src/parity*'
```

確かめた日付: 2026-10-12（base: `d8e06a1`、gitlink: `c1cb4c9`）

### 6/8（Issue #3937）

範囲は本書「後続の分担」の 6/8（オーナーへの確認）に従う。本リポで観測できる事実だけを記録する。伺いの本体は下の「オーナー判断待ち」の論点 1・2。

| # | 確かめる項目 | 方法 | 現時点の結果 |
| - | ------------ | ---- | ------------ |
| 1 | 前段（1/8〜5/8）の状態 | Issue #3932〜#3936 の状態 | 1/8〜5/8 はいずれも CLOSED |
| 2 | 範囲のポインタ（TASK-236）が gitlink の commit の `docs/spec/05-tasks.md` に在る | gitlink の commit の当該ファイルを `gh api` で読み取り、ID を検索する（取得物は残さない） | 在る |
| 3 | Issue の範囲の指し方が一意に決まる | Issue #3937 の本文と gitlink の commit の当該タスク | 成り立たない（範囲は本書「後続の分担」に従う） |
| 4 | 論点 1・2 が伺いの 5 項目（要約・選択肢と影響・推奨・止まるもの・回答の形）の形で在る | 本書「オーナー判断待ち」、[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md) | 成り立つ |
| 5 | 論点 1・2 への回答の記録 | spec リポ側を ID で検索（gitlink の commit と `main` の先端。取得物は残さない）、Issue #461・#3937 のコメント | 無い（回答待ち） |
| 6 | 伺いの経路 | contract-shaping-escalation の「経路」 | 自動運転のため直接は伺っていない。本書と PR 本文の「オーナー判断待ち」に残した |
| 7 | 見直しの提案（F9）で本リポから挙げたもの | 4/8 の記録、本書の「オーナー判断待ち」 | 無い（新しい論点は足していない） |
| 8 | 本段で新しい論点を足していない・推奨案を確定していない | `git diff origin/main...HEAD` | 成り立つ |
| 9 | 品質ゲート | `make check` | 通過（`make verify`・`make deny` は未実行。コード・設定の変更が無いため） |

- 判定: 論点 1・2 は未回答。本書は案のまま。7/8（#3938）・8/8（#3939）のうち回答に依る部分は「保留」
- 再実行の契機: オーナーの回答、gitlink の更新（`update-external.yml` の PR）
- 版の記録: 2026-10-12（base: `c0d94a2`、gitlink: `c1cb4c9`。spec リポの `main` の先端は 2026-10-12 時点で確かめた）

### 7/8（Issue #3938）

範囲のポインタ: TASK-236 の成果物に対応する決定の記録（`docs/spec/05-tasks.md`、`docs/spec/04-behavior/build-profiles.md` の BUILD-18・BUILD-64）。本リポで観測できる事実だけを記録する。決定の記録と成果物の本体の正は private spec 側で、本書には置かない。

| # | 確かめる項目 | 方法 | 現時点の結果 |
| - | ------------ | ---- | ------------ |
| 1 | 前段（1/8〜6/8）の状態 | Issue #3932〜#3937 の状態 | 1/8〜5/8 は CLOSED。6/8（#3937）は OPEN（PR #7042 も OPEN で base と衝突中） |
| 2 | 範囲のポインタ（TASK-236）が gitlink の指す commit に在る | `gh api` の読み取り（取得物は削除） | 在る |
| 3 | 決定の記録の所在 | 本書「記録の所在」、`docs/contract/procedure.md` の置き場の表 | private spec 側（ポインタのみ） |
| 4 | TASK-236 の状態・決定の記録が、gitlink の指す commit に在る | 同上の取得物を「状態」「決定の記録」で検索 | 無い |
| 5 | 論点 1・2 への回答の記録 | 本リポの文書・Issue #3937 の状態 | 無い（回答待ち。6/8 が OPEN のまま） |
| 6 | 成果物の本体（spec 側） | 本リポからは確かめない | 本リポからは確かめない（書き手と経路は論点 1 の判断待ち） |
| 7 | 本リポ側で決まっている点 | 本書「形の設計（3/8）」 | 在る（既存の決定から一意に導いた点だけ） |
| 8 | 本範囲で新しい決定・新しい論点を足していない。推奨案を確定していない | `git diff origin/main...HEAD` | 成り立つ |
| 9 | feature・モジュール・テスト・領域ごとの契約の文書・make ターゲット・CI ジョブを足していない | `git diff --stat origin/main...HEAD`、`ls docs/contract crates/contract/src` | 成り立つ |
| 10 | 品質ゲート | `make check` | 通過（`make verify`・`make deny` は未実行。コード・設定の変更なし） |

- 判定: 決定の記録は spec 側にまだ無く、本書は案のまま。回答に依る部分は「保留」で、8/8（#3939）へ引き継ぐ
- 再実行の契機: オーナーの回答、gitlink の更新（`update-external.yml` の PR）
- 版の記録: 2026-10-12（base: `08b9c22`、gitlink: `c1cb4c9`）

### 8/8（Issue #3939）

範囲のポインタ: TASK-236 の完了の条件（`docs/spec/05-tasks.md`。対象の項目: `docs/spec/04-behavior/build-profiles.md` の BUILD-18・BUILD-64）。本リポで観測できる事実だけを記録する。完了の条件との対応づけ・点検の本体・結果・件数の正は private spec 側で、本書には置かない。

| # | 確かめる項目 | 方法 | 現時点の結果 |
| - | ------------ | ---- | ------------ |
| 1 | 前段（1/8〜7/8）の状態 | Issue #3932〜#3938 と PR #7042 の状態 | 6/8（#3937）だけ OPEN（PR #7042 も OPEN）。ほかは CLOSED |
| 2 | 範囲のポインタ（TASK-236）が gitlink の指す commit と spec の `main` の先端に在る | `gh api` の読み取り（取得物は削除） | 在る（どちらにも見出しが 1 件） |
| 3 | BUILD-18・BUILD-64 が `docs/spec/04-behavior/build-profiles.md` に在る | 同上 | 在る（どちらの ref にも） |
| 4 | TASK-236 の進み具合・決定の記録が spec 側に在る | 同上の取得物の TASK-236 の節を「状態」「決定」で検索 | 無い（どちらの ref にも） |
| 5 | 論点 1・2 への回答の記録 | Issue #461・#3937 のコメント、spec 側 | 無い（回答待ち） |
| 6 | 点検の記録の本体（spec 側）と、完了の条件との対応づけ | 本リポからは確かめない | 本リポからは確かめない（書き手と経路は論点 1 の判断待ち） |
| 7 | 外部仕様（Direct3D 12）との突き合わせ（F8） | 本段では reference-researcher へ委譲していない | 未検証（理由: 結果の置き場が spec 側で、書き手が論点 1 の判断待ち） |
| 8 | 前提にした本リポの事実の 2 つの表がいまも成り立つ | `ls docs/contract crates/contract/src`、`grep -rn -i dx12 crates scripts .github` | 成り立つ（再確認したのは DX12 の記述、背後実装のモジュール、`docs/contract` の中身） |
| 9 | 点検の対象になる操作・型が製品 crate の公開面に在る | 5/8 の「公開面の一覧のコマンド」 | 無い（`pub mod` の 4 件だけ） |
| 10 | `backend-dx12-compute` が中身の無い仮の置き場のまま | `crates/contract/Cargo.toml` | 成り立つ |
| 11 | 関連タスクの状態 | Issue #461・#1145・#1154 | いずれも OPEN |
| 12 | 本段で新しい論点を足していない・推奨案を確定していない | `git diff origin/main...HEAD` | 成り立つ |
| 13 | feature・モジュール・テスト・領域ごとの契約の文書・make ターゲット・CI ジョブを足していない | `git diff --stat origin/main...HEAD`、`ls docs/contract crates/contract/src` | 成り立つ |
| 14 | 品質ゲート | `make check` | 通過（`make verify`・`make deny` は未実行。コード・設定の変更なし） |

- 判定: 本リポで確かめられる項目に問題は無い。ただし **TASK-236 の完了は本リポからは確かめられず、宣言しない**（spec 側に進み具合・決定の記録が無く、論点 1・2 が未回答で、6/8 も OPEN のため）。回答に依る部分は「保留」
- 再実行の契機: オーナーの回答、gitlink の更新（`update-external.yml` の PR）、6/8 の小節のマージ
- 版の記録: 2026-10-12（base: `508080b`、gitlink: `c1cb4c9`、spec の `main` の先端 `fef7100` も読み取りで確認）
- 引き継ぎ（親 #461 へ）: 残るのは、論点 1・2 への回答 → spec 側の記録 → 本表の再実行 → spec 側の進み具合の記録 → 親のクローズの判断、の順。いずれも本リポの自動運転の対象外

## オーナー判断待ち

論点 1: spec 側の点検の記録を、誰がどの経路で書くか（ポインタ: TASK-236、#461）

1. 要約: 記録の正は spec 側だが、自動運転の実装エージェントは spec リポへ書けない
2. 選択肢と影響
    - A: main セッションまたはオーナーが spec リポ側で書く（進み具合の記録と同じ経路）。本リポの sub-issue は観測できる事実の確かめだけを残す
    - B: 公開してよい範囲をオーナーが決め、その範囲だけ本リポに置く。spec 漏えいの検査の負担が増える
    - C: 記録の作成を自動運転の対象から外し、後続の sub-issue を止める
3. 推奨: A（既存の分担と同じ。公開側に新しい内容を出さない）
4. 止まるもの: 記録を埋める作業、#461 のクローズ、TASK-236 を前提に引くタスク（`docs/spec/05-tasks.md` を参照。ポインタのみ）
5. 回答の形: 「推奨で」の一言で A

論点 2: spec 側の点検の記録の形を、公開側にどこまで映すか（ポインタ: TASK-236、#461）

1. 要約: 形の本体は spec 側で決める。本リポに雛形などを置くかの範囲を伺う
2. 選択肢と影響
    - A: 映さない。本リポは上の形の設計（本リポ側の文書の形）だけを持つ
    - B: 実在しないダミー値だけの雛形を本リポに置く（先例: `docs/contract/TEMPLATE.md`）。公開してよい範囲をオーナーが決める必要があり、spec 漏えいの検査の負担が増える
3. 推奨: A（論点 1 の A と同じ経路。公開側に新しい内容を出さない）
4. 止まるもの: spec 側の記録の本体を埋める作業。本リポ側の確かめの記録は上の形の設計だけで進められる
5. 回答の形: 「推奨で」の一言で A

推奨案は先行確定しない。判断が出るまで本書は案のままとする。

## 後続の分担

| Issue | 段 | 本リポ側で行うこと |
| ----- | -- | ------------------ |
| #3932 | 1/8 | 現状と前提の節の追記 |
| #3934 | 3/8 | 形の設計の節（本書。節の構成・確かめの記録の表の形・結果の語彙・版の記録） |
| #3935〜#3939 | 4/8〜8/8 | 本リポで観測できる事実による確かめの記録。6/8 はオーナーへの確認（経路は上の論点に従う）、8/8 は完了の条件の確かめ |

- spec 側への記録は spec リポ側の作業で、本リポからは書き換えない

## 関連

- [contract/README.md](contract/README.md)・[contract/procedure.md](contract/procedure.md)・[implementation-scope.md](implementation-scope.md)・[workspace-boundary.md](workspace-boundary.md)
- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md)・[poc-carry-over](../.claude/rules/poc-carry-over.md)
