# Metal の `create_pipeline` のコンパイルのオプション（TASK-774）の所在と本リポの現状

TASK-774 の作業のうち、Metal の背後実装の `create_pipeline` がコンパイルのオプション付きの入力を受け付ける部分の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-774。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装 | `crates/contract/src/backend_metal.rs` の 1 ファイル | `crates/contract/src/backend_metal.rs` |
| 入力から設定を読む関数 | 非公開の `compile_plan`（device に触らない純粋関数）と `CompilePlan` | `grep -n 'fn compile_plan' crates/contract/src/backend_metal.rs` |
| オプションを作る関数 | `imp` の中の非公開の `compile_options` | `grep -n 'fn compile_options' crates/contract/src/backend_metal.rs` |
| 入口 | `create_pipeline`。`Device::compute()` から届く（TASK-2956・TASK-773 のポインタ。device の取得は TASK-158 のポインタ） | `crates/contract/src/backend_metal.rs` |
| cfg | feature `backend-metal`。`imp` は macOS だけ | `crates/contract/src/lib.rs` |

## オプション付きの入力の生成（1/3）

`create_pipeline` が受け付ける入力と、ライブラリのコンパイルに渡すオプション:

| 入力 | 数学モード | 関数の精度 | 言語版 | 渡すオプション |
| ---- | ---------- | ---------- | ------ | -------------- |
| `Msl` | 設定しない | 設定しない | 設定しない | 既定（`None`） |
| `MslSpecialized` | 設定しない | 設定しない | 設定しない | 既定（`None`） |
| `MslWithOptions`（`Safe`） | `Safe` を設定 | 設定しない | 設定しない | 新しいオプション |
| `MslWithOptions`（`Fast`） | 設定しない | 設定しない | 設定しない | 既定（`None`） |
| `MslSpecializedWithOptions`（`Safe`・`Precise`） | `Safe` を設定 | `Precise` を設定 | 設定しない | 新しいオプション |
| `MslSpecializedWithOptions`（`Safe`・`Fast`） | `Safe` を設定 | 設定しない | 設定しない | 新しいオプション |
| `MslSpecializedWithOptions`（`Fast`・`Precise`） | 設定しない | `Precise` を設定 | 設定しない | 新しいオプション |
| `MslSpecializedWithOptions`（`Fast`・`Fast`） | 設定しない | 設定しない | 設定しない | 既定（`None`） |
| `Wgsl` | - | - | - | `Unsupported` |

- `MslWithOptions` は特殊化しない関数、`MslSpecializedWithOptions` は function constant が空でも特殊化の API で関数を作る
- 照らす順: 入力の分類、`compile_plan`、`layout.slots` の重複、function constant の検査、`compile_options`、ライブラリのコンパイル、以降は `Msl`・`MslSpecialized` と同じ
- 設定の関数を持たない環境では、Metal の設定の API を呼ぶ前に `Unsupported { op: "create_pipeline", backend: Metal }`（既定のオプションで黙って作らない）
- 受け付けない言語の成果物の理由の文は `accepted inputs are Msl, MslSpecialized, MslWithOptions and MslSpecializedWithOptions`

| 試験 | 内容 | 確かめ方 |
| ---- | ---- | -------- |
| `classify_accepts_option_variants_like_their_plain_forms` | 2 つの入力の取り出しと特殊化の印 | `cargo test -p fandhe-silicon-contract --features backend-metal --lib backend_metal::precheck_tests` |
| `compile_plan_sets_only_safe_math_and_precise_functions` | 入力 × 値ごとの計画の表 | 同上 |
| `classify_marks_wgsl_unsupported`・`classify_rejects_other_languages_and_names_the_variant` | `Wgsl` の `Unsupported` と理由の文 | 同上 |
| `compile_options_tests`（2 本） | 計画がオプションに書かれ、言語版が変わらないこと。device に触らない。macOS だけ | macOS で `cargo test -p fandhe-silicon-contract --features backend-metal --lib compile_options_tests` |

## device の要る生成の試験（2/3）

`create_pipeline` にオプション付きの入力を渡す試験を、device に触る試験として `#[ignore]` 付きで足している。通常の `cargo test` では流れない。dispatch・投入・出力の読み出しは行わず、生成の成否と表の件数だけを見る。

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `msl_with_options_safe_and_fast_are_created` | 同じソースの `MslWithOptions`（`Safe`・`Fast`） | どちらも `Ok`。device の識別子が入り、`__backend_id()` が違い、表に 2 件 |
| `msl_specialized_with_options_is_created_for_every_mode_and_function_choice` | `MslSpecializedWithOptions`（数学モード 2 値 × 関数の精度 2 値、定数 2 個） | 4 つとも `Ok`、表に 4 件 |
| `invalid_msl_with_options_is_artifact_rejected` | 文法の誤りと存在しない入口名（`Safe`・`Fast`） | `ArtifactRejected { kind: "msl" }`、表は空 |
| `option_inputs_without_a_required_constant_are_artifact_rejected` | 必須の定数を持つソースに、定数が空の `MslSpecializedWithOptions` と `MslWithOptions` | `ArtifactRejected { kind: "msl" }` |
| `option_inputs_reject_duplicate_slots_and_bad_constants_before_compiling` | 重複した slot、重複した定数の番号、範囲外（65536）の番号 | `InvalidArgument`、表は空 |
| `option_inputs_reach_create_pipeline_through_the_public_device`（`tests/backend_metal_device.rs`） | 公開の `Device` の `compute()` から `MslWithOptions`（`Safe`・`Fast`）と `MslSpecializedWithOptions`（`Safe`・`Precise`、定数 1 個） | すべて `Ok` |

device の試験の枠で流すときのコマンドの形:

```text
cargo test -p fandhe-silicon-contract --features backend-metal --lib -- --ignored <試験の名前>
cargo test -p fandhe-silicon-contract --features backend-metal,native-blocking-wait --test backend_metal_device -- --ignored option_inputs_reach_create_pipeline_through_the_public_device
```

## 完了の条件の確かめ（3/3）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-774。ポインタのみ）。

時点のコミット: `a9c5f47`

| # | 確かめ | コマンド | 結果 |
| - | ------ | -------- | ---- |
| 1 | 分類と計画の純粋な試験 | `cargo test -p fandhe-silicon-contract --features backend-metal --lib backend_metal::precheck_tests` | 84 passed; 0 failed、終了コード 0 |
| 2 | 入力の分類が 4 つを受け付けること | `grep -n 'PipelineInput::MslWithOptions\|PipelineInput::MslSpecializedWithOptions\|PipelineInput::Wgsl' crates/contract/src/backend_metal.rs` | 本体の 5 行。下の (2) の出力 |
| 3 | オプションの渡し方 | `grep -n 'newLibraryWithSource_options_error\|setMathMode\|setMathFloatingPointFunctions\|setLanguageVersion' crates/contract/src/backend_metal.rs` | `setLanguageVersion` は 0 件。ほかの行は下の (3) の出力 |
| 4 | macOS の読み戻しの試験 | `gh run view 42442092097 --log --job 128613273404 \| grep compile_options_tests`（2/3 のマージの後の main の push の run の `make verify + build-profiles (macos-latest)`） | 2 本とも `ok` |
| 5 | device の要る試験の宣言 | `#[ignore = "touches the Metal device` の次の行の `fn` を、試験の名前で絞る（`crates/contract/src/backend_metal.rs` と `crates/contract/tests/backend_metal_device.rs`） | lib 5 本・`backend_metal_device.rs` 1 本。`--ignored` は流していない |
| 6 | unsafe・依存 | `cargo test -p fandhe-silicon-contract --features backend-metal --test backend_metal_unsafe_count`、`git diff --stat b95bb5b 688d771 -- Cargo.toml Cargo.lock deps-approved.toml crates/contract/Cargo.toml crates/contract/tests/backend_metal_unsafe_count.rs`、`make deps-check` | 1 passed; 0 failed、差分は 0 行、終了コード 0 |
| 7 | CI | `gh run view 42442092097 --json conclusion,headSha,jobs`（2/3 のマージの後の main の push の run） | conclusion `success`、headSha `688d771b56fa07301ef27d846c21ef0766bb6e21`。`make verify (fmt + clippy + test)`・`make verify + build-profiles (macos-latest)`・`make deny (cargo-deny)` が `success` |

(1) の出力（`running` と `test result:` の行と、名前に `classify`・`compile_plan` を含む試験の行）。

```text
running 84 tests
test backend_metal::precheck_tests::a_panicking_classify_does_not_lose_registrations ... ok
test backend_metal::precheck_tests::classify_accepts_msl_and_msl_specialized ... ok
test backend_metal::precheck_tests::classify_accepts_option_variants_like_their_plain_forms ... ok
test backend_metal::precheck_tests::classify_distinguishes_empty_msl_specialized_from_msl ... ok
test backend_metal::precheck_tests::classify_marks_wgsl_unsupported ... ok
test backend_metal::precheck_tests::classify_rejects_other_languages_and_names_the_variant ... ok
test backend_metal::precheck_tests::compile_plan_sets_only_safe_math_and_precise_functions ... ok
test result: ok. 84 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out; finished in 0.01s
```

(2) の出力（試験の行を除く）。

```text
123:        PipelineInput::MslWithOptions {
128:        PipelineInput::MslSpecializedWithOptions {
134:        PipelineInput::Wgsl { .. } => Err(unsupported()),
173:        PipelineInput::MslWithOptions { math_mode, .. } => Ok(CompilePlan {
177:        PipelineInput::MslSpecializedWithOptions {
```

(3) の出力。

```text
1308:            if !options.respondsToSelector(sel!(setMathMode:)) {
1311:            options.setMathMode(MTLMathMode::Safe);
1314:            if !options.respondsToSelector(sel!(setMathFloatingPointFunctions:)) {
1317:            options.setMathFloatingPointFunctions(MTLMathFloatingPointFunctions::Precise);
2388:                .newLibraryWithSource_options_error(&NSString::from_str(source), options.as_deref())
2602:                sel!(setMathMode:),
2603:                sel!(setMathFloatingPointFunctions:),
```

(4) の出力（ログの接頭辞を除く）。

```text
test backend_metal::imp::compile_options_tests::default_plan_gives_no_options ... ok
test backend_metal::imp::compile_options_tests::plan_is_written_to_the_options_and_language_version_is_left_alone ... ok
```

(5) の出力（`backend_metal.rs`、`backend_metal_device.rs` の順。行番号は `fn` の行）。

```text
3304-        fn msl_with_options_safe_and_fast_are_created() {
3323-        fn msl_specialized_with_options_is_created_for_every_mode_and_function_choice() {
3356-        fn invalid_msl_with_options_is_artifact_rejected() {
3386-        fn option_inputs_without_a_required_constant_are_artifact_rejected() {
3426-        fn option_inputs_reject_duplicate_slots_and_bad_constants_before_compiling() {
--- crates/contract/tests/backend_metal_device.rs
1271-fn option_inputs_reach_create_pipeline_through_the_public_device() {
```

(6) の出力。

```text
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | オプション付きの入力の生成 | #5627 |
| 2/3 | device の要る生成の試験 | #5628 |
| 3/3 | 完了の条件の確かめ | #5629 |
