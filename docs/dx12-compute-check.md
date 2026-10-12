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
