# 比較基準の wgpu の版の記録の書式

比較用の run の記録に、比較基準の wgpu の **どの版** を使ったかを残すための書式。
ポインタ: TASK-7（`docs/spec/05-tasks.md`）・M-6・AC-2。

- 本書式は案である。TASK-28（記録の全体の形式と置き場）でオーナーの確認を経るまで、確定した方針として扱わない。
- 書式を選んだ理由・経緯・判断は private spec 側の TASK-7 の決定の記録に置く。本リポにはポインタだけを書く。
- 例示値は実在しないダミー。実際の版番号・タグ名・commit SHA は本書に書かない。

## 適用範囲

- 対象: 比較基準の別 workspace（基盤の外。TASK-7 の 2/4 で置く）から動かす、比較用のすべての run
- 基盤の workspace（`crates/` の 4 crate）は wgpu に依存しない。根拠は `deny.toml` と [workspace-boundary](workspace-boundary.md)

## 版の記録の欄

値は run のたびに別 workspace の実物から取る。spec から値を写さない。

| 欄 | 必須 | 内容 | 値の取り方 |
| -- | ---- | ---- | ---------- |
| `wgpu_baseline.version` | 必須 | 解決された `wgpu` の版（`x.y.z` 形式） | 別 workspace の `Cargo.lock` から取る |
| `wgpu_baseline.source` | 必須 | 取得元の種類（`crates-io` または `git`） | `Cargo.lock` の `source` から判別する |
| `wgpu_baseline.git_repository` | `git` のとき必須 | upstream のリポジトリ URL | `Cargo.lock` の `source` から取る |
| `wgpu_baseline.git_tag` | `git` のとき必須 | 固定したタグ名 | 別 workspace の `Cargo.toml` の `tag` から取る |
| `wgpu_baseline.git_commit` | `git` のとき必須 | commit SHA（40 桁の小文字 16 進） | `Cargo.lock` の `#<sha>` から取る |
| `wgpu_baseline.lockfile_sha256` | 必須 | 別 workspace の `Cargo.lock` の SHA-256 | run の時点のファイルから計算する |
| `wgpu_baseline.basis` | 必須 | どの比較基準の版で測ったかの区分。ID のポインタで示す（M-6・AP-3 のポインタ） | run の設定から取る |

- 区分の名前・数・意味は spec 側を正とし、本書には列挙しない。`basis` の値の語彙は未決（オーナー判断待ち）
- 2 つの版が同じ値であっても `basis` は省略しない

## 記録の規則

- 欄が欠けている記録、または形式が違う記録（`version` が `x.y.z` でない、`git_commit` が 40 桁の 16 進でない など）は、比較の根拠に使わない（fail-closed）
- 版は `=x.y.z`（完全固定）の宣言から解決されたものに限る。別 workspace の依存の追加・更新にも [dependency-policy](../.claude/rules/dependency-policy.md) の承認が要る
- 記録に書かないもの（M-17 のポインタ）: 実ホスト名・非公開の接続先・資格情報・トークン・UUID の値・ローカルの絶対パス・ユーザー名

## 記入例

実在しないダミー値だけを使う。

```json
{
  "wgpu_baseline": {
    "version": "0.0.0",
    "source": "git",
    "git_repository": "https://example.invalid/wgpu.git",
    "git_tag": "example-tag",
    "git_commit": "0000000000000000000000000000000000000000",
    "lockfile_sha256": "0000000000000000000000000000000000000000000000000000000000000000",
    "basis": "BASIS-0"
  }
}
```

## 関連

- [workspace-boundary](workspace-boundary.md)
- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)
- [dependency-policy](../.claude/rules/dependency-policy.md)
- [poc-carry-over](../.claude/rules/poc-carry-over.md)
- `deny.toml`
