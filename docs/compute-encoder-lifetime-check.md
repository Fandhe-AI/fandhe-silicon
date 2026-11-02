# compute の記録先の借用の寿命と定数の渡し方の試験（TASK-772）の所在と本リポの現状

TASK-772 の作業の所在と、本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
対応づけは TASK-772 のポインタだけにする。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-772。ポインタのみ）。内容・理由・経緯は本書に置かない。
型の所在は [contract-compute-types](contract-compute-types.md)（TASK-769）を参照する。

## 記録先の型と借用の試験（1/3）

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 寿命の指定の無い構造体の欄 | `CommandEncoder` を寿命の引数を書かない構造体の欄に置いて持ち回し、`dispatch` を記録した後、欄から値で取り出して `submit` へ渡せる（統合テスト `encoder_sits_in_struct_without_lifetime_and_submits`） | `cargo test -p fandhe-silicon-core --test encoder_borrow_lifetime` |
| 呼び出しの後に残る借用 | バッファの束縛と定数の束縛を `dispatch_ex` へ渡し、値を捨てた（または書き換えた）後も同じ記録先で `dispatch` と `submit` ができる。記録は呼び出しの時点の値（`bindings_and_constants_do_not_outlive_the_call`） | 同上 |
| `submit` の前の再利用 | 同じ記録先を `dispatch`・`dispatch_ex` へ `&mut` で複数回渡せる（`encoder_is_reused_by_mut_until_submit`） | 同上 |
| `dispatch_ex` の rustdoc | 投入した記録先を `dispatch` へ渡すと `compile_fail,E0382` | `cargo test -p fandhe-silicon-core --doc` |
| `CommandEncoder` の rustdoc | 寿命の引数を持つ `ConstantBinding` を寿命の指定の無い欄に置くと `compile_fail,E0106`（対照） | 同上 |
| 既存の試験 | `CommandQueue::submit` の rustdoc の `compile_fail,E0382` と `crates/core/tests/encoder_send_sync.rs`（TASK-590 のポインタ） | 同上 |
| 定義の所在 | `CommandEncoder` の定義は core の 1 か所で、寿命の引数を持たない | `git grep -n 'pub struct CommandEncoder' crates/` |
| 公開のメソッド | `impl CommandEncoder` の `pub fn` はすべて `#[doc(hidden)]` の `__backend_` の口 | `awk '/^impl CommandEncoder/,/^}/' crates/core/src/execution/submit.rs \| grep -B1 'pub fn'` |

## 定数を dispatch ごとに渡す形の試験（2/3）

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 引き継がない | 同じ記録先で `dispatch_ex` に定数を渡した後、定数なしの `dispatch_ex` と簡易版 `dispatch` を続けると、後の 2 回の `consts` は空（統合テスト `constants_are_not_carried_to_the_next_call`） | `cargo test -p fandhe-silicon-core --test dispatch_constants_per_call` |
| 借用が呼び出しで終わる | 定数のバイト列の借用は呼び出しの間だけで足り、呼び出しの後に元を書き換えても捨てても型として成り立つ。確かめるのは借用の制約で、試験用の queue が取った写しが呼び出しの時点のままであること。製品側の実装がコピーすることの確認ではない（`constants_borrow_ends_with_the_call`） | 同上 |
| 同じ記録先の投入 | 上のどの試験も、最後に同じ記録先を `submit` へ値で渡せる | 同上 |
| 定数を受ける公開の口 | trait のメソッドの引数として `ConstantBinding` の列を受けるのは `CommandQueue::dispatch_ex` の `consts` と、feature `graphics` の `GraphicsOps::draw_ex` の `vertex_constants` だけ（ほかの行は型の定義・`impl`・rustdoc の例・`stub_device.rs` の実装）。`CommandEncoder` に定数を設定する公開のメソッドは無い（上の「公開のメソッド」） | `git grep -n 'ConstantBinding<' crates/core/src` |

## 対応するテスト

| 場所 | 内容 |
| ---- | ---- |
| `crates/core/tests/encoder_borrow_lifetime.rs` | 上の表の 3 本（試験の中だけの記録する queue を使い、機材には触れない） |
| `crates/core/tests/dispatch_constants_per_call.rs` | 上の 2/3 の表の 2 本（試験の中だけの記録する queue を使い、機材には触れない） |
| `crates/core/src/execution/submit.rs` の rustdoc | `CommandEncoder` の `compile_fail,E0106`、`dispatch_ex` の `compile_fail,E0382` |

## 確かめ方

```sh
cargo test -p fandhe-silicon-core --test encoder_borrow_lifetime
cargo test -p fandhe-silicon-core --test dispatch_constants_per_call
cargo test -p fandhe-silicon-core --doc
git grep -n 'pub struct CommandEncoder' crates/
git grep -n 'ConstantBinding<' crates/core/src
```

## 段の表

| 段 | イシュー | 内容 | 本リポの物 |
| -- | -------- | ---- | ---------- |
| 1/3 | #7692 | 記録先の型に借用の寿命の引数が無いことの試験 | 本書の「記録先の型と借用の試験（1/3）」 |
| 2/3 | #7693 | 定数を dispatch ごとに渡す形の試験 | 本書の「定数を dispatch ごとに渡す形の試験（2/3）」 |
