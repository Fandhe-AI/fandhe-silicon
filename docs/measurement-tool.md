# 性能を測る共通の道具（TASK-27）の所在と本リポの現状

TASK-27 の決定の記録の所在と、本リポのコードで確かめられる現状を案内する（親 Issue は #121）。
2/8（#123）で「形の設計」の節を足した（決定の記録のポインタ・道具が乗る既存のコードの事実・守る本リポの規約）。
spec の本文・決定の内容は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

- 決定の記録の正は private spec 側にある（ポインタ: `docs/spec/05-tasks.md` の TASK-27）
- 前提のタスクはポインタのみ（TASK-4・TASK-28・TASK-19）。対応する公開側の文書は次のとおり
    - TASK-4: ルートの `Cargo.toml` の `members` と [workspace-boundary](workspace-boundary.md)
    - TASK-28: [measurement-record](measurement-record.md)
    - TASK-19: [metal-run-procedure](metal-run-procedure.md)
- 本リポの `docs/spec` の gitlink は `0696aed`（`update = none`）
    - 確かめ方: `git ls-tree origin/main docs/spec`
    - gitlink の更新は `update-external.yml` の PR 経由で、本作業では進めない

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 製品の workspace | `members` は 4 crate、`exclude` は `baseline-wgpu` と `measure`。外部依存の承認の記録は 0 件 | `grep -n -e members -e exclude Cargo.toml`、`grep -c '^\[\[' deps-approved.toml` |
| 性能を測る道具 | 時間を測る標準ライブラリの型の使用は `measure/` にだけあり、`crates/`・`baseline-wgpu/` には無い | `grep -rn -e Instant -e SystemTime -e 'std::time' crates baseline-wgpu`（0 件）、`grep -rln 'std::time' measure --include=*.rs` |

既にある関連の部品（名前とリンクのみ）:

- `crates/core` の `test_frame` と記録の集計 `scripts/record-summary.sh`: [test-frame](test-frame.md)
- 回ごとの記録の形の検査と手順のスクリプト: [metal-run-procedure](metal-run-procedure.md)
- 比較基準の wgpu の別 workspace: [baseline-wgpu](baseline-wgpu.md)
- 機材の要らない CI の枠: [ci-device-free-jobs](ci-device-free-jobs.md)

## 形の設計（2/8）

本段の範囲は `docs/spec/05-tasks.md` の TASK-27 の内容の前半。
具体（置き場の名前・型・欄・値・統計の定義・出力の先）は 3/8（#124）以降のコードと公開テストが示し、本節には先に書かない。
本節に書くのは、決定の記録のポインタ、道具が乗る既存のコードの事実、どの形でも守る本リポの規約。

### 決定の記録に従う点

| 点 | 扱い |
| -- | ---- |
| 道具の形 | `docs/spec/05-tasks.md` の TASK-27 と、そこから辿る決定の記録に従う。本書には内容を書かない |
| 記録の形と置き場 | TASK-28 のポインタ（[measurement-record](measurement-record.md)） |
| 専有の確かめの手順 | TASK-19 のポインタ（[metal-run-procedure](metal-run-procedure.md)） |
| 測定専用のコードの置き場 | TASK-6 のポインタ（[workspace-boundary](workspace-boundary.md)・[poc-carry-over](../.claude/rules/poc-carry-over.md)） |
| 試験の結果の部品との分担 | TASK-23 のポインタ（[test-frame](test-frame.md)） |
| 範囲の外 | TASK-1354・TASK-1356・TASK-53 のポインタ。本節では扱わない |
| 決定の記録から一意に決まらない点 | [contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md) の経路で伺う。推奨案を既定として確定しない |

### 道具が乗る既存のコード・検査の事実

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 試験の結果の部品 | `crates/core` の `test_frame::record` が `RecordPart` と `BODY_KIND` を持つ（`test-support` の裏）。run の欄を足して 1 件に組むのは受け取る側 | `crates/core/src/test_frame/record.rs` の `//!` |
| 部品の書き出し | `test_frame::export` が `RECORD_DIR_ENV` で指定されたディレクトリへ書く（指定が無ければ書かない） | `crates/core/src/test_frame/export.rs` |
| 回ごとの記録の形の検査 | `scripts/session-record-check.sh`。make ターゲットは無い | ファイル冒頭のコメント |
| run ごとの記録の形の検査 | `scripts/perf-record-check.sh`。make ターゲットは無い（7/8） | ファイル冒頭のコメント |
| 環境の集計値 | `scripts/metal-session.sh probe` が JSON を標準出力へ出す（macOS だけ） | ファイル冒頭のコメント |
| 部品の集計 | `scripts/record-summary.sh` | [test-frame](test-frame.md) |
| path の検査 | `scripts/deny.sh` の `ALLOWED_ROOT` は `crates/`。測定専用の crate の置き場が決まったら見直す、とコメントにある | `scripts/deny.sh` |
| 別 workspace の前例 | `baseline-wgpu/` はルートの `exclude`。9/11 で `make deny` の対象に入った（下の「現状の形（9/11）」） | [baseline-wgpu](baseline-wgpu.md) |
| CI の `deny` ジョブ | `make deny`、`make deps-check`、各回帰テストの順に呼ぶ | `.github/workflows/ci.yml` |
| 外部依存 | 製品の workspace の承認の記録は 0 件 | `deps-approved.toml` |

### どの形でも守る本リポの規約

| # | 点 | 守ること | 根拠 |
| - | -- | -------- | ---- |
| M1 | 製品の workspace | 製品の crate は道具に依存しない。向きは「測定 → 製品の公開 API」の一方向。製品の `members`・依存グラフ・`Cargo.lock` に足場を入れない | [poc-carry-over](../.claude/rules/poc-carry-over.md)、[workspace-boundary](workspace-boundary.md) |
| M2 | 依存 | 標準ライブラリだけで書く。依存が要る場合は承認の経路と `=x.y.z` 固定 | [dependency-policy](../.claude/rules/dependency-policy.md) |
| M3 | unsafe | 道具のコードは `unsafe` を持たない | [coding-rust](../.claude/rules/coding-rust.md)、[unsafe-policy](unsafe-policy.md) |
| M4 | エラー | ライブラリの部分は `Result` を返し panic しない。取得できない値を暗黙に埋めない | [coding-rust](../.claude/rules/coding-rust.md) |
| M5 | 数値 | 既存の規約に従う（具体は実装と公開テストの PR で書く） | [security](../.claude/rules/security.md) |
| M6 | 機械可読の出力 | 既存の先例に従う（具体は実装と公開テストの PR で書く） | [test-frame](test-frame.md) |
| M7 | 出力に含めない情報 | 既存の規約と TASK-28 のポインタに従う（具体は実装と公開テストの PR で書く） | [security](../.claude/rules/security.md)、TASK-28 のポインタ |
| M8 | 信頼しない入力 | 既存の規約と検査に従う（具体は実装と公開テストの PR で書く） | [security](../.claude/rules/security.md)、`scripts/session-record-check.sh` |
| M9 | 検査は fail-closed | 形が合わない・前提が足りないときは成功扱いにしない。終了コードは既存の検査に揃える | 既存の `scripts/*-check.sh` |
| M10 | 純粋な計算と I/O | 計算は純粋な関数に寄せ、書き出しと分ける。テストはプロセスの環境と作業ツリーを書き換えない | `test_frame` の先例 |
| M11 | CI | ジョブと必須チェックを増やさず、既存のジョブに手順を足す。CI 用のツールは版固定・ハッシュで検証する | [github-actions](../.claude/rules/github-actions.md)、[ci-job-split](ci-job-split.md) |
| M12 | 測らなかった結果 | skip・ignore・アサーションの弱体化で CI を通さない。取得できなかった値を成功にも失敗にも読み替えない | [coding-rust](../.claude/rules/coding-rust.md) |

### 3/8（#124）で扱ったこと

- 置き場のディレクトリ名・crate 名は 3/8 がコードで決めた。内容は下の「現状の形（3/8）」

### 3/8（#124）への引き継ぎ（実施済みの点を含む）

- 着手時に、gitlink の版の spec の TASK-27 とそこから辿る決定の記録を読み直す。一意に導けない点は推奨案で確定せず伺う
- 決定の記録のポインタに従い、置き場のディレクトリ名・crate 名などは 3/8 がコードで決め、選んだ形を PR 本文に書く
- 置き場を足す段では、同じ PR で次を揃える
    - ルートの `Cargo.toml` の `exclude`
    - `docs/workspace-boundary.md`・`docs/baseline-wgpu.md` の関係の記述
    - `scripts/deny.sh` の `ALLOWED_ROOT` のコメント
    - `README.md`・`CLAUDE.md` のツリー
    - 本書の「本リポの現状」の表
- 公開アイテム・スクリプトを足したら、本書に「現状の形（N/8）」を足し、具体はその PR のコードとテストに合わせて書く

## 現状の形（3/8）

測定専用の別 workspace `measure/`（ルート `Cargo.toml` の `exclude`。自身の `Cargo.lock` を持つ）に、共有 crate `measure-perf`（`measure/crates/perf/`・lib のみ・外部依存 0 件）を置いた。
製品の crate は本 crate に依存せず、本 crate も製品の crate に依存しない。device には触れない。
記録の書き出しは 4/8 の節に書く。後続の段の範囲（欄の追加・判定など）は本節に書かない。

### 公開アイテムと振る舞い

| モジュール | 公開アイテム | 振る舞い（対応するテスト） |
| ---------- | ------------ | -------------------------- |
| `sample` | `collect`・`sample_once`・`SampleCounts`・`PhaseSamples`・`SubmissionSample`・`CollectError`・`duration_to_ns` | 指定した数だけ cold・warmup・計測の順に閉包を呼ぶ。GPU 時間は閉包が返した `Option<u64>` をそのまま持ち、取得不能を 0 にしない。計測の回数 0 は `Err`。閉包の失敗は `CollectError::Submit` で返す。`Duration` が `u64` の ns に収まらなければ `Err`（`tests/sample.rs`） |
| `stats` | `run_stats`・`RunStats`・`MAX_EXACT_NS` | 入力（ns）を書き換えず昇順にして、中央値（偶数個は中央 2 つの平均）・Q1・Q3・p95（添字 `round(p × (n − 1))`・補間なし・端数 0.5 は 0 から遠い側へ丸める）・最小・最大を出す。空と上限超えは `Err`（`tests/stats.rs`） |
| `across` | `across_runs`・`across_run_stats`・`AcrossRuns` | run ごとの中央値の中央値、最大 − 最小、最大 ÷ 最小を出す。最小が 0 のときの比は `None`（無限大・NaN を返さない）。空・有限でない値・負の値は `Err`（`tests/across.rs`） |
| 共通 | `PerfError` | 失敗の種類を表す。ライブラリのコードは panic せず、`unsafe` を持たない（workspace の lint で forbid） |

### 確かめ方

- `make measure-verify`（`scripts/measure-verify.sh`）: `measure/` の fmt --check・clippy（-D warnings）・test・`deps-check.sh measure/Cargo.toml`。CI の `deny` ジョブも呼ぶ
- 製品の workspace が変わっていないこと: `cargo metadata --format-version 1 --no-deps | jq -r '.workspace_members[]'`（4 crate）

### 段の表

| Issue | 段 |
| ----- | -- |
| #122 | 1/8 現状と前提の確認 |
| #123 | 2/8 形の設計（本節） |
| #124 | 3/8 実装 |
| #125 | 4/8 実装 |
| #126 | 5/8 実装 |
| #127 | 6/8 実装 |
| #128 | 7/8 テスト |
| #129 | 8/8 確かめ（済。本書の「完了の条件の確かめ（8/8）」） |

## 現状の形（4/8）

`measure-perf` に、run ごとの記録を書く側の部品を足した（`record`・`json`。外部依存は増えない）。
封筒の芯と `perf-samples` の本体を検証つきで作り、決定的な JSON（1 行・ASCII だけ）にして、新しいファイルへ書く。
時計は読まない。標本数・run 数・機材ごとの制限は持たず、node の機材の記録も同じ形で書ける。
後続の段の範囲は本節に書かない。

| 公開アイテム | 振る舞い（対応するテスト） |
| ------------ | -------------------------- |
| `UtcTime` | `YYYY-MM-DDTHH:MM:SSZ` の暦として正しい値だけを受ける。Unix 秒からの変換は純粋（`tests/record.rs` の t1） |
| `Id`・`RefId`・`SessionRef`・`NO_SESSION` | ID は白表の形だけを受ける。回なしの予約語は回の ID の形に一致しない（t2・t3） |
| `RunRecord`・`Envelope` | 開始が終了より後なら `Err`、同時刻は受ける（t4） |
| `Machine`・`MachineCombination`・`Label`・`NotRunReason`・`IndeterminateReason` | 綴りは `crates/core` の `test_frame` のソースと突き合わせる。理由は理由を持つラベルのときだけ `label_reason` として出る（t5・t6） |
| `PerfSamplesBody`・`Body` | GPU 時間の取得不能は要素ごとに `null`、計測の区間に 1 つでもあれば GPU の統計は `null`。数の食い違い・空・上限超えは `Err`（t7・t8・t8b・t12） |
| `RunRecord::to_json`・`json` | 欄の順序は固定で同じ入力から同じバイト列。出力は ASCII だけ。中央値は整数か `.5` の 10 進（t9・t10・t11） |
| `write_new` | 既存のファイルは上書きせず `Err`。ディレクトリは作らない（t13）。node の機材も同じ形（t14） |

確かめ方は `make measure-verify`（3/8 と同じ）。

## 現状の形（5/8）

`measure-perf` に、封筒の経路の証拠・環境・比較基準の版の欄と、本体の処理件数・命令の表を足した（`evidence`。外部依存は増えない）。
値は白表の列挙と形を狭く検証した文字列・数だけを受け、綴りと形は `src/evidence.rs` と `tests/record.rs` が示す。
文字列の欄の取得不能は予約語 `UNAVAILABLE`、型を持つ欄は `null`、出力なしは予約語 `NO_OUTPUT`。条件に当たらない欄はキーごと出さない。
後続の段の範囲は本節に書かない。

| 公開アイテム | 振る舞い（対応するテスト） |
| ------------ | -------------------------- |
| `Sha256`・`CommitSha`・`SemVer`・`VersionText`・`Name`・`DeviceName`・`OptionToken`・`GitRepository`・`LoadAvg` | 形に合わない値・制御文字・非 ASCII・長さ超えは `Err`（`tests/record.rs` の t15） |
| `UNAVAILABLE`・`NO_OUTPUT` | どの値型の形にも一致せず、互いに・`NO_SESSION` とも異なる（t16）。欄の型ごとの書き分け（t17） |
| `Environment`・`BackendEnv` | 背後実装ごとの欄だけが出る。組み合わせと食い違えば `Err`（t18） |
| `WgpuBaseline`・`BaselineSourceSpec`・`Basis` | 比較用の run のときだけ出る。取得元ごとの欄は取得元が合うときだけ出る（t19） |
| `Evidence`・`AdapterBackend` | `adapter_backend` は入力のまま出し、組み合わせに寄せない。綴りは `MachineCombination` と core のソースに揃える（t20） |
| `EnvSnapshot`・`PowerSource`・`LoadAvg` | 表のキー順は決定的（t21）。負荷平均は固定桁の 10 進で、電源の綴りは `scripts/metal-session.sh` に揃える（t22） |
| `PerfSamplesBody::with_processed_count`・`with_instructions_found` | 処理件数は値か `null`、命令の表はキー順で出る（t23） |

確かめ方は `make measure-verify`（3/8 と同じ）。

## 現状の形（6/8）

`measure-perf` に、2 つの値の比から「同等」「差がある」の区分を出す部品を足した（`ratio`。外部依存は増えない）。
純粋な関数で、記録（`RunRecord` の JSON）は変えない。綴りと振る舞いは `src/ratio.rs`（単体テスト）と `tests/ratio.rs` が示す。
後続の段の範囲は本節に書かない。

| 公開アイテム | 振る舞い（対応するテスト） |
| ------------ | -------------------------- |
| `DIFF_RATIO_NUM`・`DIFF_RATIO_DEN`・`diff_ratio`・`diff_ratio_f64` | 閾値 1.3 を整数比 13 / 10 で持つ。`f64` は表示用で判定に使わない（`threshold_constants`） |
| `RATIO_SCOPE_TASKS` | 適用範囲の TASK の ID の唯一の置き場。いまは空で、各要素は `RefId` の形に合う（`scope_entries_are_valid_ids`） |
| `classify` | 分子 ÷ 分母が閾値以上なら `Differs`（ちょうど閾値も含む）、未満なら `Equivalent`。片側だけの比較（単体テストの `boundary_exactly_threshold_differs`・`just_below_and_above`・`one_sided`） |
| `RatioClass` | 範囲外の ID は `Pending` で理由は `out-of-scope`。綴りは英小文字 ASCII で互いに異なる（`out_of_scope_is_pending`・`tokens`） |
| 入力の検査 | 有限でない・負・分母 0・上限超え・1/4 刻みに乗らない値は範囲外でも `Err`（`invalid_inputs_err_even_out_of_scope`）。`RunStats`・`AcrossRuns` の中央値をそのまま渡せる（`accepts_real_medians`） |

確かめ方は `make measure-verify`（3/8 と同じ）。

## 現状の形（7/8）

run ごとの記録を読む側の形の検査 `scripts/perf-record-check.sh` と、同じ測定を繰り返したときの記録の形を確かめるテストを足した。
外部依存は増えない（Python は標準ライブラリだけ・Rust は std だけ）。make ターゲットは持たない（検査本体は引数が要る）。
欄の定義はスクリプトの定数表が唯一の正で、書く側（`record`・`evidence`・`json`）との食い違いは `tests/repeat.rs` が検出する。
後続の段の範囲は本節に書かない。

| 公開アイテム | 振る舞い（対応するテスト） |
| ------------ | -------------------------- |
| `scripts/perf-record-check.sh <run>` | 欄の集合・型・値の形・配列の長さ・条件付きの欄・時刻順を検査し、終了コード 0 合格・1 違反・2 誤用。入力の値と表に無い欄の名前は出さない（`scripts/test-perf-record-check.sh` の `set:`・`type:`・`form:`・`len:`・`gpu:`・`order:`・`input:`・`misuse:`・`secret:`） |
| `scripts/perf-record-check.sh --session <回> [<run> ...]` | 回の記録の形を先に通し、回の側と run ごとの除外（回への参照・時間帯・電源・熱・検証レイヤー・取得不能・重複）を見る。除外は終了コード 1（同じ `session:`・`input:` 系のケース） |
| `docs/run-record.example.json` | ダミーの記入例。書き出しの実物とバイト一致し、回の記入例と組で判定に使える（`tests/repeat.rs` の r6・r5） |
| 繰り返しのテスト | 合成の標本は同じバイト列になり（r1）、CPU だけのダミー測定を繰り返しても run の ID 以外の条件の欄が一致して値の欄は件数どおり（r2）。実物の出力の各分岐が検査を通り（r3）、欄の増減・型違いは落ちる（r4） |

確かめ方は `make measure-verify`（`tests/repeat.rs` を含む）と `make test-perf-record-check`。

## 完了の条件の確かめ（8/8）

8/8（#129）で、本リポで観測できる事実による確かめを記録する（ポインタ: `docs/spec/05-tasks.md` の TASK-27）。
完了の条件との対応づけの記録の正は private spec 側で、条件の本文は転記しない。先行の節は書き換えない。
結果は 2026-10-15 に GPU の実機なしで実行した値。件数は実測。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| A1 | 合成の標本の記録が繰り返しで同じバイト列になる | `tests/repeat.rs` の `r1_synthetic_samples_repeat_to_identical_bytes` | 通過 |
| A2 | CPU だけのダミー測定を繰り返すと、run の ID 以外の条件の欄が一致し、値の欄が件数どおり | `r2_cpu_only_dummy_measurement_repeats_with_same_shape` | 通過 |
| A3 | 書き出しの実物の各分岐が形の検査を通り、欄の増減・型違いは落ちる | `r3_checker_accepts_every_branch_of_real_output`・`r4_checker_rejects_added_and_removed_fields` | 通過 |
| A4 | 記入例が書き出しの実物とバイト一致し、回の記入例と組で `--session` の検査を通る | `r5_session_mode_uses_the_documented_examples`・`r6_documented_example_is_the_exact_output` | 通過 |
| A5 | 手元での 2 回の繰り返し | `make measure-verify` を 2 回実行 | 2 回とも終了コード 0・`cargo test` の通過 60 件（`repeat.rs` は 6 件） |
| A6 | CI での繰り返し | main の CI（`7c2057f` の run）の `deny` ジョブ | `make measure-verify`・`make test-perf-record-check` の step とも success。本段の PR 自身の CI は作成時点で未検証 |
| B1 | 記録を読む側の検査の回帰テスト | `make test-perf-record-check` | 終了コード 0 |
| B2 | 記録の書き出しの部品のテスト | `measure/crates/perf/tests/` の `record.rs`（24 件）・`sample.rs`（5 件）・`stats.rs`（7 件）・`across.rs`（5 件）・`ratio.rs`（6 件） | 全件通過 |
| C1 | 製品の workspace が変わっていない | `cargo metadata --format-version 1 --no-deps` の `workspace_members`、ルート `Cargo.toml` の `exclude` | 4 crate・`exclude` に `measure` |
| C2 | 外部依存がない | `deps-approved.toml`・`measure/deps-approved.toml` の `[[` の数、`measure/Cargo.lock` の `[[package]]` の数、`make deps-check` | 0 件・0 件・1 件（`measure-perf` 自身）・終了コード 0 |
| C3 | `unsafe` を持たない | `measure/` 配下を `unsafe` で検索 | `measure/Cargo.toml` の lint 設定の行のみ（`forbid`・`deny`） |
| C4 | 時間を測る型が製品側にない | `crates`・`baseline-wgpu` を `Instant`・`SystemTime`・`std::time` で検索 | 0 件 |
| C5 | 製品の crate が測定の道具を参照しない | `crates/` を `measure` で検索 | 一致は `crates/core/tests` の関数名 2 件のみ（依存ではない） |
| D | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check`・`make measure-verify`・`make test-perf-record-check`・`make ci-frame-check` | すべて終了コード 0 |

### 判定

本リポで観測できる範囲では、run ごとの記録の出力と、同じ測定の繰り返しで記録の形が保たれることを、公開テストと品質ゲートで確かめた。
本書は TASK-27 の完了を宣言しない（記録の正は private spec 側）。

### 再実行の契機

- `measure-perf` の公開面、記録の欄、`scripts/perf-record-check.sh` の定数表、`docs/*.example.json`、CI の `deny` ジョブの手順を変えたとき
- `docs/spec` の gitlink を進めたとき

確かめた base は `7c2057f`、`docs/spec` の gitlink は `0696aed`。

## 現状の形（9/11）

別 workspace の依存の検査を `make deny` に入れた（親 #121 の 9/11、#7147）。新しいジョブ・必須チェックは作らず、CI の `deny` ジョブが `make deny` を呼ぶ。

| 点 | 事実 | 確かめ方 |
| -- | ---- | -------- |
| `measure/` の cargo-deny | ルートの `deny.toml` をそのまま `--config` で当てる（wgpu 系の禁止も効く） | `scripts/deny.sh` |
| `baseline-wgpu/` の cargo-deny | `baseline-wgpu/deny.toml` を `--config` で明示して当てる。ルートの `deny.toml` との違いは `[bans] deny` を空にした点だけ | `baseline-wgpu/deny.toml` |
| `baseline-wgpu/` の依存の承認の記録 | `scripts/deps-check.sh baseline-wgpu/Cargo.toml` を `make deny` が呼ぶ | `scripts/deny.sh` |
| `measure/` の deps-check と build / test | 従来どおり `make measure-verify` | `scripts/measure-verify.sh` |
| ビルド | `baseline-wgpu/` のビルド・実行は `make` にも CI にも入れない | `scripts/deny.sh` |
| 失敗の扱い | 設定ファイルや `jq` が無いときは成功にせず失敗する | `scripts/deny.sh` |

確かめ方は `make deny`（終了コード 0）。`baseline-wgpu/` にルートの `deny.toml` を当てると bans で落ちる。

## 本リポでの反映の方針

- 道具の置き場・記録の欄・統計・検査は、受け持つ段の PR のコードと公開テストが示す（親 #121）
- 本書には、その PR で入った振る舞いを書き足す

## 関連

- [measurement-record](measurement-record.md)
- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)
