# 公開 API の名前の検査（TASK-14・TASK-113）の決定の記録の所在と本リポの現状

TASK-14・TASK-113 の決定の記録の所在と、本リポのコードで確かめられる現状を案内する。
spec の本文・決定の内容は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

- 決定の記録の正は private spec 側にある（ポインタ: `docs/spec/05-tasks.md` の TASK-14・TASK-113 と、そこから辿る決定の記録。関連: TASK-10・TASK-112）
- 本リポには決定の記録・内容を置かない（[契約の文書の分担](contract/README.md)）

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| スクリプト | `scripts/public-name-check.sh` が `scripts/` にある。JSON を作る段は `scripts/public-api-check.sh --emit-json` を呼んで共有する | `ls scripts` |
| 設定ファイル・Make ターゲット | 語の一覧の設定ファイル `public-name-check.conf` がリポジトリ直下にあり、`make public-name-check` が入口になる | `ls public-name-check.conf`・`make help` |
| 回帰テスト | `scripts/test-public-name-check.sh` が、合成 JSON・合成ソースと一時 workspace の実物の rustdoc の JSON で、終了コード 0・1・2 を確かめる | `make test-public-name-check` |
| CI | CI の `deny` ジョブが `make public-name-check` と `make test-public-name-check` を呼ぶ | `grep -n public-name .github/workflows/ci.yml` がステップを返す |
| 公開面 | `crates/contract/src/lib.rs` の `pub mod` は `backend_metal`・`backend_vulkan`・`backend_cuda` の feature ゲート付き 3 つ。`crates/core/src/lib.rs` は条件なしの `pub mod hash`・`pub mod execution` と、`test-support` の裏の `pub mod parity`・`pub mod test_frame`・`pub mod rerun` のみ。`crates/exec`・`crates/upper` の `lib.rs` に `pub mod`・`pub use` は無い | `grep -n "pub mod\|pub use" crates/*/src/lib.rs` |

## 決定表の語彙の検査（TASK-142）

`make public-name-check` は、既存の `--source` と新しい `--exact-fields` を決定表に当てて渡す（専用のスクリプトは持たない。語の一覧は `public-name-check.conf` のまま）。

| 検査 | 対象 | 読み方 |
| ---- | ---- | ------ |
| `diagnostics(` の呼び出しが 0 件 | ソース `crates/core/src/decision.rs` | `scanned-source:` の `call_hits=0`。当たると `violation: source=… word=diagnostics(` |
| ベンダー名 14 語・API 名 6 語の単語単位の一致が 0 件 | 同じソース（コメント・文字列も数える） | `scanned-source:` の `word_hits=0`。当たると `violation: source=… line=… word=<語>` |
| 製品固有の語 6 語が公開アイテムの名前に 0 件 | rustdoc の JSON の全公開アイテム（`decision` モジュールを含む上位集合） | `scanned:` の `name_hits=0`。当たると `violation: crate=… item=… word=…` |
| `DecisionInput` の公開フィールドが 4 つだけ | rustdoc の JSON の `decision::DecisionInput`（crate 内の経路で特定） | `scanned-fields:` の `public_fields=capability,scale,residency,scalar`。過不足は `violation: fields … problem=unexpected-field:… / missing-field:… / field-count:…`。`dtype` は集合に無いので足すと落ちる |

- 対象のソースは実装ファイル `decision.rs` だけで、`decision/tests.rs` は含めない。`decision/` に `tests.rs` 以外のファイルが増えると回帰テストが落ちるので、そのファイルを `Makefile` の `--source` に足す
- 単語の境界は `[A-Za-z0-9_]` なので、複合識別子（例 `MetalDevice`・`metal_device`）の中の語は数えない（スクリプトのヘッダの「検査しないもの」と同じ）
- フィールドの集合は、ソースの grep でなく rustdoc の JSON から読む（`pub` の有無・`#[non_exhaustive]`・フィールドの順に影響されない。非公開フィールドは数えない）。指定の struct が渡した JSON のどれにも無ければ終了コード 2
- `--exact-fields` を渡したときだけ、`record:` の末尾に `field_specs=<n> field_hits_total=<n>` が付く。語の一覧の変更は TASK-14 の流儀に従い、今回は変えていない
- 回帰テスト（`make test-public-name-check`）は、合成 JSON のフィールドの各経路に加えて、実物の `decision.rs` の一時コピーへ禁止の語・`diagnostics(` を 1 つ足すと落ちること、実物の rustdoc の JSON の一時コピーへフィールド `dtype` や名前 `tensor_shape` を足すと落ちることを確かめる（陽性対照）
- CI は `deny` ジョブの既存のステップ `make public-name-check` で流れる（ジョブ・ステップの追加なし）

## 本リポでの反映の方針

- 検査の内容（規則・語・失敗の条件）の説明は、それを受け持つタスクの PR で、公開テスト（回帰テスト）とともに入れる（ポインタ: TASK-113。関連: TASK-10・TASK-112）
- 検査の内容は、その PR のコードとテストが示す
- 進め方の追跡は親 Issue #394 に従う

## 関連

- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)
- [implementation-scope](implementation-scope.md)
