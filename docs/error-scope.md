# 捕捉されなかったエラーの口とエラーの範囲（TASK-140）の所在と本リポの現状

TASK-140 の作業の所在と、本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-140。ポインタのみ）。内容・理由・経緯は本書に置かない。
決定の記録は private spec 側の記録に従う（ポインタのみ）。関連する本リポの物は次のとおり。

| 物 | 文書 |
| -- | ---- |
| `crates/core/src/error.rs`（`ContractError`。TASK-269） | [contract-error-types](contract-error-types.md) |
| `crates/core/src/validation.rs`（`ValidationReporter`。TASK-554） | [validation-intake](validation-intake.md) |

## 捕捉されなかったエラーの口と保持（1/5）

型・メソッド・種類の名前は仮。コードと公開テストが示す事実だけを置く。

### 所在

- ファイル: `crates/core/src/error_scope.rs`
- 公開のパス: `fandhe_silicon_core::error_scope`（feature の条件なし）
- `ErrorFilter`: `Validation`・`OutOfMemory`・`Internal` の 3 値（`non_exhaustive`）
- 上限: `UNCAPTURED_RETENTION_LIMIT = 64`（持っておく列と、次の呼び出しで渡す列の合計）

### 振る舞い

関数の型 `UncapturedErrorCallback` は native が `FnMut(ContractError) + Send`、wasm32 は `Send` なし。

| 操作 | 入力・状態 | 動き |
| ---- | ---------- | ---- |
| `deliver_uncaptured` | `DeviceLost` | 何もしない |
| `deliver_uncaptured` | 関数が登録済みで実行中でない | 次の呼び出しで渡す列を古い順に、続けて今回の値を、lock の外で関数へ渡す |
| `deliver_uncaptured` | 関数の実行中 | 次の呼び出しで渡す列に積む。関数は呼ばない |
| `deliver_uncaptured` | 未登録 | 持っておく列に積む |
| `deliver_pending` | 登録済みで実行中でない | 次の呼び出しで渡す列を古い順に関数へ渡す |
| `deliver_pending` | 関数の実行中 | 何もしない |
| `deliver_pending` | 未登録 | 次の呼び出しで渡す列を持っておく列の末尾へ移す |
| `set_uncaptured_error_callback` | 登録済み | 置き換える。登録の前に持っていた値は渡さない |
| `take_uncaptured_errors` | 常に | 持っておく列を古い順に返し、上限を超えて数えた件数を返す。どちらも空にする。次の呼び出しで渡す列は含めない |

- 上限に達したあとの値は持たず、件数を `u64` の飽和加算で数える
- 関数が panic しても登録は残り、呼び切れなかった値は次の呼び出しで渡す列の先頭へ順を保って戻る
- lock が毒化していても panic しない

### 確かめ方

```sh
cargo test -p fandhe-silicon-core --lib error_scope
```
