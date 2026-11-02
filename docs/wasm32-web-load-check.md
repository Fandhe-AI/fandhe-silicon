# web の出力の Node での読み込みの検査（TASK-248）の所在と本リポの現状

wasm-bindgen の `--target web` の出力を Node の ESM で読み込み、決まった 7 名の export の有無と
`detect_isa_bits` の戻り値を行で出す検査の所在と、本リポの現状の事実をまとめる。

## 記録の所在

記録の正は private spec 側（`docs/spec/05-tasks.md` の TASK-248。ポインタのみ）。本文は転記しない
（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## web の出力の読み込みの検査

### 読み込みの枠と判定（1/4）

#### 所在

| 物 | 場所 |
| -- | ---- |
| 読み込みと観測（判定なし） | `wasm/node/load-web.mjs` |
| 検査の本体（判定） | `scripts/wasm-web-load-check.sh` |
| 回帰テスト | `scripts/test-wasm-web-load-check.sh` |

本体は `--from-dir <dir>` を付けると生成済みの出力だけを読む（省いた経路は「ビルドの経路（2/4）」）。
node は `v24.13.0` と厳密に一致しなければ、組を読む前に終了コード 2 になる。

#### 組

組の綴りと入力の置き方は `scripts/wasm-size.sh` の `--from-dir` に揃えている（TASK-247 のポインタ）。
読むのは `<dir>/<組>/web/wasm_glue.js` と `wasm_glue_bg.wasm` の 2 名だけで、`--from-dir` 直下の 4 組以外の項目や
`web` 以外の項目は見ない。

| 組 | 入力の置き方 | `detect-isa-bits` の期待の値 |
| -- | ------------ | ---------------------------- |
| `release-scalar` | `<dir>/release-scalar/web/` | 0 |
| `release-simd128` | `<dir>/release-simd128/web/` | 1 |
| `wasm-size-scalar` | `<dir>/wasm-size-scalar/web/` | 0 |
| `wasm-size-simd128` | `<dir>/wasm-size-simd128/web/` | 1 |

#### 11 項目

| 項目 | export | いまの判定の規則 |
| ---- | ------ | ---------------- |
| `gemm-scalar-1`・`gemm-scalar-2` | `gemm_scalar` | export が無ければ `not-run`（`missing-export`）、在っても呼ばず `not-run`（`no-call-rule`） |
| `gemm-simd-1`・`gemm-simd-2` | `gemm_simd` | 同上 |
| `topk-scalar` | `topk_scalar` | 同上 |
| `topk-simd` | `topk_simd` | 同上 |
| `elementwise-scalar-relu`・`elementwise-scalar-add` | `elementwise_scalar` | 同上 |
| `elementwise-simd-relu`・`elementwise-simd-add` | `elementwise_simd` | 同上 |
| `detect-isa-bits` | `detect_isa_bits` | export が無ければ `not-run`（`missing-export`）。在れば引数なしで 1 回呼び、値が組の期待と同じなら `ok`、違えば `ng`、呼び出しが失敗すれば `ng`（`call-failed`） |

`ok` は呼んで確かめた項目にだけ付く。`detect_isa_bits` 以外の 6 名は呼ばない。

#### `load-web.mjs` の結果

結果は 1 行の JSON で、形は次の 3 つだけ。例外の文・パスは書き出さない。

| 形 | 条件 |
| -- | ---- |
| `{"init":"failed"}` | 読み込み・`initSync` の欠け・`initSync` の例外のどれか |
| `{"init":"ok","exports":[...]}` | 初期化でき、`detect_isa_bits` が無い |
| `{"init":"ok","exports":[...,"detect_isa_bits"],"detect_isa_bits":<0〜4294967295 の整数 or "call-failed">}` | 初期化でき、`detect_isa_bits` が在る（戻りが number でなければ `call-failed`） |

引数が 2 つでなければ終了コード 2、それ以外は 0。

#### 行の形

| 行 | 形 |
| -- | -- |
| 組 | `init: combo=<組> target=web result=ok\|ng\|not-run[ reason=<理由>]` |
| 項目 | `item: combo=<組> item=<名前> export=<名前> label=ok\|ng\|not-run[ reason=<理由>][ value=<値>]` |
| 締め | `summary: combos=4 items=44 ok=<n> ng=<n> not-run=<n>` |

| 理由 | 出る場面 |
| ---- | -------- |
| `missing-directory` | 組または `web` のディレクトリが無い |
| `missing-file` | `wasm_glue.js` か `wasm_glue_bg.wasm` が無い |
| `empty-file` | 2 名のどちらかが 0 バイト |
| `init-failed` | `{"init":"failed"}`（組の行は `ng`、11 項目は `not-run`） |
| `missing-export` | 初期化できた組で、項目の export が無い |
| `no-call-rule` | 計算の 10 項目で export が在る |
| `call-failed` | `detect_isa_bits` の呼び出しが失敗した |

#### 終了コード

| コード | 意味 |
| ------ | ---- |
| 0 | 4 組とも `init` が `ok` で 44 項目とも `ok` |
| 1 | `ng` か `not-run` が 1 つ以上（標準エラーに `FAILED:`） |
| 2 | 誤用・前提の不足（node の版違いを含む）・読めない入力（シンボリックリンクなど）・不正な結果（標準エラーに `error:`。このとき `init:`・`item:`・`summary:` は出さない） |

#### glue の export

glue の側の `wasm_bindgen` の付いた関数は、次の確かめのとおり 1 件（`roundtrip_f32`）で、7 名の export は無い。

```text
$ grep -n wasm_bindgen wasm/crates/glue/src/lib.rs
18:#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
```

#### 確かめ方

```text
$ bash scripts/test-wasm-web-load-check.sh
...
test-wasm-web-load-check: 67 passed, 0 failed
```

記録時点のコミットは `d4ed11c`。偽の node による判定の全経路と、本物の node で `load-web.mjs` を偽の glue に直接当てる試験を含む。

### ビルドの経路（2/4）

`--from-dir` を省くと、`wasm/` の glue を 4 組でビルドし、wasm-bindgen の CLI で `--target web` の出力を一時ディレクトリの
`<組>/web/` に作ってから、1/4 と同じ判定にかける。`--from-dir` の経路の振る舞いは変わらない（cargo も CLI も呼ばない）。

#### 前提

外れたら cargo を呼ぶ前に終了コード 2 になる。CLI の入手は試みない。

| 前提 | 確かめ方 |
| ---- | -------- |
| python3 3.8 以上 | 本体の python3 の版の確認 |
| node `v24.13.0` | `node --version` の厳密一致（`--from-dir` の経路と共通） |
| cargo・rustc・`wasm-bindgen` が PATH にある | 無ければ `required command not found on PATH` |
| `wasm-bindgen --version` が `wasm/crates/glue/Cargo.toml` の版と一致 | 1 行目が `wasm-bindgen <版>` と厳密一致 |
| `wasm32-unknown-unknown` の target | `rustc --print sysroot` の `lib/rustlib/wasm32-unknown-unknown` |
| 依存が手元にある | `cargo metadata --locked --offline` が成功する（取得は `make wasm-verify`） |

#### 組ごとのビルド

成果物は cargo の `target_directory` の下の専用のディレクトリ `glue-web-load` に出す（`wasm-size.sh` の `glue-size` と
製品のビルドとは共有しない）。開始時にこのディレクトリだけを作り直す。`RUSTFLAGS` は子に渡さない。

| 組 | `--profile` | `CARGO_ENCODED_RUSTFLAGS` | wasm-bindgen |
| -- | ----------- | ------------------------- | ------------ |
| `release-scalar` | `release` | 空 | `wasm-bindgen --target web --out-dir <一時>/out/release-scalar/web <wasm>` |
| `release-simd128` | `release` | `-C\x1ftarget-feature=+simd128` | 同上（組の名前だけ違う） |
| `wasm-size-scalar` | `wasm-size` | 空 | 同上 |
| `wasm-size-simd128` | `wasm-size` | `-C\x1ftarget-feature=+simd128` | 同上 |

cargo は共通で `build --manifest-path wasm/Cargo.toml -p wasm-glue --target wasm32-unknown-unknown --locked --offline`。

#### `not-run` の理由（ビルドの経路）

組の `init:` 行と 11 項目の `item:` 行が同じ理由の `not-run` になる（終了コードは 1。2 にはしない）。その組の node は流さない。

| 理由 | 出る場面 |
| ---- | -------- |
| `build-failed` | cargo の失敗、または成果物 `wasm_glue.wasm` が無い |
| `bindgen-failed` | wasm-bindgen の失敗 |

#### 手元で流す方法

`make wasm-verify` の後に `scripts/wasm-web-load-check.sh` を流す（Node v24.13.0 と wasm-bindgen の CLI が要る）。

#### 確かめ方

```text
$ bash scripts/test-wasm-web-load-check.sh
...
test-wasm-web-load-check: 122 passed, 0 failed
```

記録時点のコミットは `6f71964`。偽の cargo・rustc・wasm-bindgen・node によるビルドの経路の場面（組ごとの引数、
`RUSTFLAGS` が渡らないこと、ビルドと生成の失敗、前提の不足）を含む。

### make ターゲットと CI（3/4）

| ターゲット | スクリプト | CI で呼ぶジョブ |
| ---------- | ---------- | --------------- |
| `make wasm-web-load-check` | `scripts/wasm-web-load-check.sh` | 呼ばない（手元の実行） |
| `make test-wasm-web-load-check` | `scripts/test-wasm-web-load-check.sh` | `deny` |

`make -n` の出力。

```text
$ make -n wasm-web-load-check
scripts/wasm-web-load-check.sh
$ make -n test-wasm-web-load-check
scripts/test-wasm-web-load-check.sh
```

`make ci-frame-check` と `make test-wasm-web-load-check` の最終行。

```text
ok: ci workflow frame (8 jobs, rules F1-F6)
test-wasm-web-load-check: 122 passed, 0 failed
```

記録時点のコミットは `573a2e5`。CI の step は `.github/workflows/ci.yml` の `deny` ジョブにあり、
`grep -n "test-wasm-web-load-check" .github/workflows/ci.yml` で確かめる。

### 完了の条件の確かめ（4/4）

1/4〜3/4 の検査・ビルドの経路・回帰テスト・make ターゲットと CI への組み込みを、手元の実行と `main` の push の CI の出力で確かめた。結果の欄は出力をそのまま写している。

手元の実行は `origin/main` の `708a34a` の上の作業ツリー。CI は run `41548609028`（headSha `b83c546ca9a637a6c675b5e19bc3f08cb499925c`、conclusion `success`）のジョブ `make deny (cargo-deny)`（job `125630247457`）。

| 確かめ | コマンド | 結果 |
| ------ | -------- | ---- |
| (1) 4 組の綴りと web の出力の置き方 | `grep -n '^COMBOS = \|^TARGET_DIR = \|^JS_FILE = \|^WASM_FILE = ' scripts/wasm-web-load-check.sh` | 下の (1) のブロック |
| (2) 11 項目と export の対応 | `grep -n -A12 '^ITEMS = (' scripts/wasm-web-load-check.sh` | 下の (2) のブロック |
| (3) `detect-isa-bits` の組ごとの期待の値 | `grep -n -A1 'def expected_isa' scripts/wasm-web-load-check.sh` | 下の (3) のブロック |
| (4) glue の export の現状 | `grep -n wasm_bindgen wasm/crates/glue/src/lib.rs` | 下の (4) のブロック |
| (5) 回帰テスト（手元と CI） | `make test-wasm-web-load-check` と、`gh run view --job 125630247457 -R Fandhe-AI/fandhe-silicon --log` を `test-wasm-web-load-check: ` で絞る | 下の (5) のブロック |
| (6) export が無いときの出力の形の試験 | `grep -n '1 no exports\|1 44 missing-export' scripts/test-wasm-web-load-check.sh` | 下の (6) のブロック |
| (7) 1/4〜3/4 の各コミットでの `crates`・`wasm/crates`・依存の変更 | コミットごとに `git diff --stat <c>^..<c> -- crates wasm/crates wasm/Cargo.toml wasm/Cargo.lock wasm/deps-approved.toml Cargo.lock deps-approved.toml`（`c01f972`・`573a2e5`・`b83c546`） | 下の (7) のブロック（diff の出力なし） |

(1)

```text
72:COMBOS = ("release-scalar", "release-simd128", "wasm-size-scalar", "wasm-size-simd128")
73:TARGET_DIR = "web"
86:JS_FILE = "wasm_glue.js"
87:WASM_FILE = "wasm_glue_bg.wasm"
```

(2)

```text
99:ITEMS = (
100-    ("gemm-scalar-1", "gemm_scalar"),
101-    ("gemm-scalar-2", "gemm_scalar"),
102-    ("gemm-simd-1", "gemm_simd"),
103-    ("gemm-simd-2", "gemm_simd"),
104-    ("topk-scalar", "topk_scalar"),
105-    ("topk-simd", "topk_simd"),
106-    ("elementwise-scalar-relu", "elementwise_scalar"),
107-    ("elementwise-scalar-add", "elementwise_scalar"),
108-    ("elementwise-simd-relu", "elementwise_simd"),
109-    ("elementwise-simd-add", "elementwise_simd"),
110-    ("detect-isa-bits", "detect_isa_bits"),
111-)
```

(3)

```text
140:def expected_isa(combo):
141-    return 0 if combo.endswith("-scalar") else 1
```

(4)

```text
18:#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
```

(5)

手元の最終行。

```text
test-wasm-web-load-check: 122 passed, 0 failed
```

CI のログ。

```text
make deny (cargo-deny)	make test-wasm-web-load-check	2026-11-02T23:54:15.9395478Z test-wasm-web-load-check: 122 passed, 0 failed
```

(6)

```text
163:expect "1 no exports" 1 "summary: combos=4 items=44 ok=0 ng=0 not-run=44" "reason=missing-export" "FAILED:"
164:check "1 44 missing-export rows" "$([ "$(grep -c 'reason=missing-export' "${OUT_F}")" = 44 ] && echo 0 || echo 1)"
```

(7)

```text
== c01f972
== 573a2e5
== b83c546
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | web の出力を Node で読み込む枠と回帰テスト | #3970 |
| 2/4 | web の 4 組をビルドする経路 | #3971 |
| 3/4 | make ターゲットと回帰テストの CI への組み込み | #3972 |
| 4/4 | 完了の条件の確かめ | #3973 |

親 Issue は #468。
