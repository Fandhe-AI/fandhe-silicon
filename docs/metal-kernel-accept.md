# 実行時に受けるカーネルの受け付けの道具（TASK-485）の所在と本リポの現状

TASK-485 の作業のうち、実行時に渡されるカーネルの一覧のファイルの読み取りと形の検査、入口名の抜き出しの所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-485。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 置き場 | `measure/crates/kernel-accept`（package `measure-kernel-accept`） | `measure/Cargo.toml` の `members` |
| 一覧の読み取りと検査 | `measure/crates/kernel-accept/src/manifest.rs`（`parse`・`load`・`Manifest`・`ManifestError`。純粋で、device に触れず、`unsafe` を書かない） | `pub mod manifest;`（`src/lib.rs`） |
| 入口名の抜き出し | `measure/crates/kernel-accept/src/scan.rs`（`scan_entries`） | `pub mod scan;`（`src/lib.rs`） |
| 試験 | `measure/crates/kernel-accept/tests/manifest.rs`・`tests/scan.rs`（どの OS でも走る。`#[ignore]` なし） | ファイルの有無 |
| 確かめ方 | `make measure-verify` | fmt / clippy / test / 依存の検査 |

## 一覧のファイルの形（1/6）

### 行の形

- UTF-8 のテキストで、1 行が 1 入口。改行は LF。最後の LF は付けても付けなくてもよい（あれば 1 つだけ）。
- 1 行は半角の空白 1 つで区切った 5 欄 `<rev> <path> <sha256> <mode> <entry>`。前後の空白・空白の重なりは欄の数が合わず拒否する。
- 行の番号は 1 始まり。空行・CR・タブなどの制御文字を含む行は拒否する。

### 欄ごとの検査

| 欄 | 受けるもの | 拒否するもの |
| -- | ---------- | ------------ |
| rev | 40 桁の小文字の 16 進 | それ以外 |
| path | 素直な相対パス（`metal-gemm-arm` の `check_source_path` と同じ） | 空・`-` か `/` で始まる・`..` 成分・`:`・NUL・改行などを含むもの |
| sha256 | 64 桁の小文字の 16 進 | それ以外 |
| mode | `msl`・`specialized` | それ以外 |
| entry | `[A-Za-z_][A-Za-z0-9_]{0,63}` | それ以外 |

### 行をまたぐ検査

- 全行の rev が 1 行目と同じ。
- 同じパスの行は連続し、sha256 が同じ。
- 違うパスの最後の成分（ファイル名）が重ならない。
- 入口名が一覧全体で重ならない。
- 最初に違反した行の番号で失敗を返す。

### 上限と失敗の文言

- 大きさ `MAX_MANIFEST_BYTES`（64 KiB）と行数 `MAX_MANIFEST_LINES`（1024）を超える一覧は、行ごとの検査より前に拒否する。`load` は通常のファイルだけを受け、読む前と開いた後に確かめ、上限つきで読む。
- 空の一覧は拒否する。
- 失敗の文言は固定の英語と行の番号だけで、入力の値（rev・パス・sha256・入口名）を含めない。

### 入口名の抜き出し

`scan_entries` は、正規表現 `kernel\s+void\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(` と同じ規則で、先頭から重ならずに入口名を出現の順に拾う。コメントは除かない。
