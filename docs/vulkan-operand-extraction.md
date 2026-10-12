# Vulkan の成果物からオペランドの宣言を取り出す道具（TASK-98）の所在と現状の事実

SPIR-V のモジュールから storage buffer の宣言を取り出す検証用の道具について、本リポのコードと公開テストから説明できる振る舞いを記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-98。ポインタのみ）。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 置き場と有効になる条件

- 実装: `crates/contract/src/backend_vulkan/spirv_operands.rs`。`crates/contract/src/backend_vulkan.rs` の `#[cfg(feature = "test-support")] pub mod spirv_operands;` で公開する
- 有効になる条件: feature `backend-vulkan` と `test-support` の両方が有効で、wasm32 でないこと。`test-support` なしのビルドには入らない
- 統合テスト: `crates/contract/tests/backend_vulkan_spirv_operands.rs`。`crates/contract/Cargo.toml` の `[[test]]` で `required-features = ["backend-vulkan", "test-support"]`
- `unsafe`: 使わない（`#![forbid(unsafe_code)]`）。`crates/contract/tests/backend_vulkan_unsafe_count.rs` の期待値の表に個数 0 の行がある（[unsafe-policy](unsafe-policy.md)）

## 公開面

| 名前 | 内容 |
| ---- | ---- |
| `extract_storage_buffer_operands(&[u32]) -> Result<Vec<ExtractedOperand>, SpirvExtractError>` | 語の列を借用で受け、storage buffer の宣言を（set, binding）の昇順で返す |
| `ExtractedOperand` | `set`・`binding`・`element`（`ScalarKind`）・`element_bytes`・`array_stride`（`Option<u32>`）・`byte_length`（`ByteLength`）。フィールドは `pub` で、`Debug`・`Clone`・`PartialEq`・`Eq` を持つ |
| `ScalarKind` | `Float { bits }` / `Int { bits, signed }`（`non_exhaustive`） |
| `ByteLength` | `Fixed(u64)`（固定長の配列）/ `Unsized`（runtime array。0 や推測で埋めない）（`non_exhaustive`） |
| `UnsupportedShape` | 取り出しの対象外の形の種別（`non_exhaustive`） |
| `SpirvExtractError` | 入力の拒否の種別（`non_exhaustive`）。`Display`（英語）と `std::error::Error` を実装する |

## 取り出す内容と対象

- 対象は storage class が `StorageBuffer` の変数で、`Block` 装飾の struct がメンバー 1 つ（配列または runtime array）だけを持つ形。そのメンバーの `Offset` は 0
- 要素の型は浮動小数点か整数で、ビット幅は 8 の倍数。`element_bytes` はビット幅から求める
- 固定長の配列の `byte_length` は `Fixed(長さ × ArrayStride)`。runtime array は `Unsized`
- 並びは（set, binding）の昇順
- 対象でない storage class の変数（push constant・`Uniform` + `Block` を含む）はエラーにせず結果から外す。モジュール全体の変数が対象で、エントリポイントでの絞り込みはしない

## 対象外の形と拒否する入力

対象外の形は `SpirvExtractError::Unsupported { id, shape }` で返す。`shape`（`UnsupportedShape`）の種別は次のとおり。

| 種別 | 形 |
| ---- | -- |
| `Bool`・`VectorOrMatrix`・`StructElement` | 要素が bool・vector / matrix・struct |
| `BitWidth` | ビット幅が 0 か 8 の倍数でない |
| `BlockArray`・`BlockShape` | block の配列、メンバーが「配列 1 つ」でない block、`Block` 装飾が無い |
| `NonZeroMemberOffset`・`MissingMemberOffset` | メンバーの `Offset` が 0 でない、`Offset` が無い |
| `OtherType` | 上のどれでもない型 |
| `LegacyBufferBlock` | `Uniform` + `BufferBlock` |
| `LengthNotConstant`・`InvalidLength` | 配列の長さが 32 ビット整数の 1 語の定数でない、正でない |
| `ZeroStride`・`MissingArrayStride`・`StrideSmallerThanElement` | `ArrayStride` が 0、固定長の配列に無い、要素のバイト数より小さい |
| `DecorationGroup` | 装飾グループを使っている（`id` はグループの id） |

そのほかの拒否は `SpirvExtractError` の次の種別で返す。位置は語の番号で持ち、入力の中身は表示に出さない。

| 種別 | 入力 |
| ---- | ---- |
| `TooShort`・`BadMagic`・`ByteOrderReversed` | ヘッダー 5 語に満たない、マジックが違う、逆のバイト順のマジック |
| `ZeroWordCount`・`TruncatedInstruction`・`MissingOperands` | 語数 0 の命令、残りを超える語数、オペランド不足 |
| `UnknownId`・`WrongKind` | 未定義の id、参照先の種類が合わない |
| `StorageClassMismatch`・`MissingDecoration` | pointer と変数の storage class の食い違い、set か binding の欠落 |
| `DuplicateDefinition`・`DuplicateBinding` | 定義・装飾の重複、（set, binding）の重複 |

## 振る舞いとテストの対応

テストは `crates/contract/tests/backend_vulkan_spirv_operands.rs` の関数。`unsafe` の行だけ別ファイル。

| 振る舞い | テスト |
| -------- | ------ |
| 別 crate から入口に届き、ヘッダーだけのモジュールは空の列を返す | `tool_is_reachable_from_another_crate` |
| 取り出した宣言が、外で組んだ期待値と `==` で一致する | `extracted_operands_equal_expected_declarations` |
| 並びが（set, binding）の昇順になる | `order_is_by_set_then_binding` |
| 読まない命令や対象でない変数が混ざっても、storage buffer だけが返る | `compiler_like_module_yields_only_storage_buffers` |
| 型を共有する変数もそれぞれ返る | `variables_sharing_types_are_each_extracted` |
| 浮動小数点・整数の幅と符号が `ScalarKind` と `element_bytes` に写る | `scalar_kinds_and_element_bytes` |
| runtime array は `Unsized` で、0 にならない | `runtime_array_is_unsized_never_zero` |
| 固定長は長さ × stride で、最大の長さと stride でも溢れず `Fixed` で返る | `fixed_length_is_count_times_stride`、`largest_length_and_stride_do_not_overflow` |
| 長さの符号付き・符号なしの境界と 0 は `InvalidLength` の判定に従う | `signed_length_boundaries` |
| 長さが素の定数でない形は `LengthNotConstant` | `length_that_is_not_a_plain_constant_is_rejected` |
| stride が 0・要素より小さい・無いときの扱い | `stride_boundaries` |
| 5 語に満たない入力は `TooShort`。マジック違いと逆のバイト順の区別 | `inputs_shorter_than_header`、`magic_is_checked_after_length` |
| 入力の全部の接頭辞が panic せず分類される | `every_prefix_is_classified_without_panic` |
| 語数 0、残りを超える語数が位置付きのエラーになる | `zero_word_count_reports_its_offset`、`word_count_beyond_the_module` |
| 読む命令のオペランド不足が位置と opcode の付いたエラーになる | `too_few_operands_for_each_instruction_the_tool_reads` |
| 各位置の語を差し替えた入力で panic せず、成功なら（set, binding）が狭義の昇順、失敗なら表示が空でない ASCII | `single_word_corruption_never_panics` |
| 未定義の id、pointer でない型、set か binding の欠落 | `undefined_ids_are_reported`、`variable_type_that_is_not_a_pointer`、`missing_set_or_binding` |
| 装飾・定義の重複、（set, binding）の重複、storage class の食い違い（両方向） | `duplicate_decorations_are_rejected`、`duplicate_binding_reports_set_and_binding`、`storage_class_mismatch_in_both_directions` |
| `UnsupportedShape` の各種別を返す（`non_exhaustive` のため網羅は機械で確かめられず、手で保つ） | `unsupported_element_shapes_are_reported`、`unsupported_block_shapes_are_reported`、`unsupported_other_types_are_reported`、`unsupported_legacy_buffer_block_is_reported`、`unsupported_length_and_stride_shapes_are_reported`、`decoration_groups_are_rejected_with_the_group_id` |
| 対象でない storage class の変数（push constant を含む）と `Uniform` + `Block` はエラーにせず外れる | `non_storage_buffer_variables_are_skipped` |
| エラーが `std::error::Error` で、表示が空でない ASCII の 1 行 | `errors_are_std_errors_with_ascii_display` |
| 道具のファイルの `unsafe` の個数が 0 | `backend_vulkan_unsafe_count_matches_expected`（`crates/contract/tests/backend_vulkan_unsafe_count.rs`） |

公開テストの入力は、テストコードの中で手組みした SPIR-V の語の列だけで、バイナリのファイル・外部の道具・GPU の実機には依らない。
命令・装飾・storage class の番号は Khronos の公開の SPIR-V 仕様のもので、テスト側で独立に持つ。

## 確かめ方

worktree のルートで実行する。

- `make verify`: `--all-features` で fmt・clippy・test を流す（統合テストを含む）
- `cargo test --locked -p fandhe-silicon-contract --features backend-vulkan,test-support`: 道具の単体テスト・統合テスト・`unsafe` の個数テスト
- `cargo test --locked -p fandhe-silicon-contract --features backend-vulkan,test-support --test backend_vulkan_spirv_operands`: 統合テストだけ
- `make build-profiles`: feature の組ごとのビルド。`backend-vulkan` だけの組・wasm32 の組では道具はコンパイル対象外
- `make check`: editorconfig-checker と shellcheck

## 関連

- [unsafe-policy](unsafe-policy.md)・[workspace-boundary](workspace-boundary.md)・[parity-tool](parity-tool.md)（`test-support` の裏の公開の前例）
- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[security](../.claude/rules/security.md)
