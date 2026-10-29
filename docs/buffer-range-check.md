# バッファの引数検査

TASK-309 の 1/4〜4/4（MEM-70・MEM-74〜MEM-76 のポインタ）。private spec の本文は写さない。

## 所在

`crates/core/src/memory/buffer_check.rs`。公開のパスは `fandhe_silicon_core::memory::` の下で、feature の条件はない。

| 名前 | 役割 |
| ---- | ---- |
| `validate_buffer_range` | 部分範囲 `[offset, offset + len)` が確保長に収まるかを判定する。`upload` の書き込み先にも `download` の読み出し元にも使える |
| `validate_allocation_size` | 確保のバイト数が `max_buffer_bytes` を超えないかを判定する |

## 現状の振る舞い

- どちらも `Result<(), ContractError>` を返し、違反は `InvalidArgument`（文言は英語）
- `validate_buffer_range`: 終端を `checked_add` で求める。`u64` であふれたら拒否、終端が確保長を超えたら拒否（判定の順はあふれ → 超過）。終端が確保長ちょうどなら通る。`len` が 0 でも `offset` が確保長を超えれば拒否する
- `validate_allocation_size`: 上限だけを見る。上限ちょうどは通す
- 算術は `checked_add` と比較だけ。背後実装の API は呼ばず、確保もしない（エラーの文字列を除く）。`usize` は受けない

## 対応するテスト

`crates/core/src/memory/buffer_check.rs` の単体テスト（`buffer_check_` で始まる名前）。範囲内・終端ちょうど・1 バイト超過・終端のあふれ・上限ちょうどと 1 超過を確かめる。

## 境界の入力の試験（2/4）

`crates/core/tests/memory_buffer_check.rs`（feature なしの統合テスト。公開のパスだけを使う）。

- 範囲の検査（試験名の接頭辞 `buffer_range_boundary_`）: 確保長 `{0, 1, 16, u64::MAX}` に対し、offset を `{0, 1, 確保長 - 1, 確保長, 確保長 + 1, u64::MAX}`、len を `{0, 1, 確保長, u64::MAX}` とした組をすべて回す。期待は `u128` の参照で求める（終端が `u64::MAX` を超えればあふれ、確保長を超えれば超過、それ以外は通る）
- 名前つきのケース: 終端が確保長ちょうど（通る）、確保長 + 1（超過で拒否）、offset が `u64::MAX`、len が 0、判定の順（あふれと超過が両方成り立つ入力はあふれの文言で返る）
- 確保長が `u64::MAX` のとき確保長 + 1 は `u64` で表せないので、その組は除き、あふれの枝で代える
- 確保の上限（接頭辞 `allocation_size_boundary_`）: 上限 `{0, 64, u64::MAX}` に対し、バイト数 `{0, 1, 上限, 上限 + 1, u64::MAX}` の組を回す。上限ちょうどは通り、超えれば拒否
- 拒否はどれも `InvalidArgument`

## 拒否のときの内容の不変の試験（3/4）

`crates/core/tests/memory_upload_reject.rs`（feature なしの統合テスト。公開のパスだけを使う）。

- 試験用の最小の実装: バイト列を持つ非公開の型 `MinimalBuffer` が、コピーの前に `validate_buffer_range` を呼び、通ったときだけ書き込む。テストファイルの中だけの型で、公開 API ではない
- 拒否の試験（接頭辞 `upload_reject_keeps_contents_`）: 一部が重なる超過・終端が確保長 + 1・offset が確保長を超える長さ 0・offset が `u64::MAX`・確保長 0 のバッファ、および確保長 `{0, 1, 16}` の境界の格子を回し、`InvalidArgument`（あふれ／超過の枝）が返ることと、呼ぶ前後で内容が全バイト一致することを確かめる。直前に通った upload の結果が保たれることも確かめる
- 陽性対照（試験名 `upload_accept_writes_only_range`）: 範囲内では `[offset, offset + len)` だけが書き換わり、その外は元のまま残る
- 背後実装の `upload` が実際にこの検査を呼ぶことは、ここでは確かめない（背後実装ごとのタスクのポインタに従う）

## 確かめ方

```bash
cargo test -p fandhe-silicon-core buffer_check_
cargo test -p fandhe-silicon-core --test memory_buffer_check
cargo test -p fandhe-silicon-core --test memory_upload_reject
make verify
```

## 完了の条件の確かめ（4/4）（TASK-309）

4/4（#4292）で記録した。本リポで観測できる事実だけを書く（ポインタ: `docs/spec/05-tasks.md` の TASK-309。関連: MEM-70・MEM-74〜MEM-76）。GPU の実機なしで、末尾の base で実行した値。件数は実測。コードは変えていない。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| A1 | 単体テスト | `cargo test -p fandhe-silicon-core --lib buffer_check_` | 4 passed・0 failed・0 ignored |
| A2 | 境界の入力の統合テスト | `cargo test -p fandhe-silicon-core --test memory_buffer_check` | 8 passed・0 failed・0 ignored |
| A3 | 拒否のときの内容の不変の統合テスト | `cargo test -p fandhe-silicon-core --test memory_upload_reject` | 8 passed・0 failed・0 ignored |
| A4 | 統合テストの関数の数 | `grep -c -E '^fn (buffer_range_boundary_\|allocation_size_boundary_)' crates/core/tests/memory_buffer_check.rs`・`grep -c -E '^fn upload_' crates/core/tests/memory_upload_reject.rs` | 8・8（A2・A3 の件数と一致） |
| A5 | 本体に panic・確保・unsafe が無い | `sed -n '1,/#\[cfg(test)\]/p' crates/core/src/memory/buffer_check.rs \| grep -nE 'unwrap\|expect\|panic\|Vec\|Box\|unsafe'`・`crates/core/src/lib.rs` の `#![forbid(unsafe_code)]` | 0 件・`forbid(unsafe_code)` あり |
| A6 | feature の条件が無い | feature なしの `cargo test -p fandhe-silicon-core`・`make public-api-check` | 190 passed・0 failed・0 ignored・終了コード 0 |
| C1 | 依存 | `crates/core/Cargo.toml` の `[dependencies]` の数・`grep -c '^used_by = .*fandhe-silicon-core' deps-approved.toml` | 0 件・0 件（`core` に外部 crate の追加なし。記録は `crates/contract` の macOS 向けの 4 件だけ） |
| E1 | base の CI | `gh run list --branch main --workflow ci.yml --commit <base>` | run 39513132267（push）。記録時点では実行中で、結論は未確認 |
| D | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check`（本節を足した状態） | すべて終了コード 0 |

### ビヘイビア ID と試験の対応

関数名だけを挙げる。試験は境界の格子であり、全入力の網羅ではない。

| ID | 試験 |
| -- | ---- |
| MEM-70 | `upload_reject_keeps_contents_partial_overlap`・`upload_reject_keeps_contents_end_one_past`・`upload_reject_keeps_contents_offset_past_end_zero_len`・`upload_reject_keeps_contents_zero_length_buffer`・`upload_reject_keeps_contents_grid_matches_u128_reference`・`upload_reject_keeps_contents_after_accepted_upload`・`upload_accept_writes_only_range`・`buffer_range_boundary_end_one_over_allocation_is_rejected`・`buffer_range_boundary_grid_matches_u128_reference`・`buffer_check_range_one_byte_over_is_rejected` |
| MEM-74〜MEM-76 | `buffer_check_range_end_overflow_is_rejected`・`buffer_range_boundary_offset_max`・`buffer_range_boundary_overflow_is_judged_before_exceed`・`upload_reject_keeps_contents_offset_max` |
| 範囲内が通ること | `buffer_check_range_within_allocation_passes`・`buffer_range_boundary_end_equals_allocation_passes`・`buffer_range_boundary_len_zero`・`upload_accept_writes_only_range` |
| 確保の上限 | `buffer_check_allocation_size_boundary`・`allocation_size_boundary_grid`・`allocation_size_boundary_named_cases` |

### 試験が照合するもの

拒否は `InvalidArgument` で返ること、あふれの枝と超過の枝があり、両方成り立つ入力はあふれの枝で返ることを、境界の格子と名前つきのケースで確かめる。

### 範囲の外

背後実装の `upload`・`download_blocking`・`allocate` が実際にこの検査を呼ぶことの確かめは本書の範囲の外（背後実装ごとのタスクのポインタに従う）。

完了の条件の正は private spec 側の記録に従う（ポインタのみ）。

確かめた base は `eede585e7b88b0a75f9ce162f49287b1ccfa7a81`、`docs/spec` の gitlink は `f603cba40283866ca89338a66a85a9bde085a0d9`。
