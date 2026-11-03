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
| 呼び出しの後に残る借用 | バッファの束縛と定数の束縛を `dispatch_ex` へ渡し、値を捨てた（または書き換えた）後も同じ記録先で `dispatch` と `submit` ができる。試験の中の queue が写した記録は呼び出しの時点の値（`bindings_and_constants_do_not_outlive_the_call`） | 同上 |
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

## 完了の条件の確かめ（3/3）

3/3（#7694）で、本リポで観測できる事実をコマンドと出力のまま記録する。時点のコミットは `3ba90b6`。GPU の実機なしで流した。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-772。ポインタのみ）。

| 確かめ | コマンド | 結果 |
| ------ | -------- | ---- |
| (1) 1/3 の統合テスト | `cargo test -p fandhe-silicon-core --test encoder_borrow_lifetime` | `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| (1) 2/3 の統合テスト | `cargo test -p fandhe-silicon-core --test dispatch_constants_per_call` | `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| (2) doctest | `cargo test -p fandhe-silicon-core --doc` | 下の出力 (2) |
| (3) 寿命の引数 | `git grep -n 'pub struct CommandEncoder' crates/` | `crates/core/src/execution/submit.rs:159:pub struct CommandEncoder {` |
| (4) 公開のメソッド | `awk '/^impl CommandEncoder/,/^}/' crates/core/src/execution/submit.rs \| grep -B1 'pub fn'` | 下の出力 (4) |
| (5) 定数を受ける口 | `git grep -n 'ConstantBinding<' crates/core/src` | 下の出力 (5) |
| (6) feature | `grep -n 'cfg(feature' crates/core/tests/encoder_borrow_lifetime.rs crates/core/tests/dispatch_constants_per_call.rs` | 下の出力 (6) |
| (6) feature なしの実行 | `cargo test -p fandhe-silicon-core --no-default-features --test encoder_borrow_lifetime --test dispatch_constants_per_call` | `test result: ok. 2 passed` と `test result: ok. 3 passed`（どちらも 0 failed, 0 ignored） |
| (7) wasm32 の型検査 | `make wasm-verify; echo "exit=$?"` | `exit=0` |
| (8) 変更の範囲 | `git show --stat --format= <sha> -- crates/` | 下の出力 (8) |

(6) の補足（`32c8e0b` で流し直した。上の表は時点の記録で、書き換えない）: 2 本の試験に背後実装の feature（`backend-*`）の条件は無い（`grep -n 'backend-' crates/core/tests/encoder_borrow_lifetime.rs crates/core/tests/dispatch_constants_per_call.rs; echo "exit=$?"` は出力なしで `exit=1`）。出力 (6) の `cfg(feature` の行は、`native-blocking-wait` の `wait_blocking`（試験用 queue の実装）に付くもの。

出力 (2)（`cargo test -p fandhe-silicon-core --doc` の `submit.rs` の行と `test result:` の行）

```text
test crates/core/src/execution/submit.rs - execution::submit::CommandQueue::dispatch_ex (line 361) ... ok
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test crates/core/src/execution/submit.rs - execution::submit::CommandEncoder (line 153) - compile fail ... ok
test crates/core/src/execution/submit.rs - execution::submit::CommandEncoder (line 137) - compile fail ... ok
test crates/core/src/execution/submit.rs - execution::submit::SubmitToken (line 40) - compile fail ... ok
test crates/core/src/execution/submit.rs - execution::submit::CommandEncoder (line 145) - compile fail ... ok
test crates/core/src/execution/submit.rs - execution::submit::CommandQueue::dispatch_ex (line 395) - compile fail ... ok
test crates/core/src/execution/submit.rs - execution::submit::CommandQueue::on_complete (line 271) - compile fail ... ok
test crates/core/src/execution/submit.rs - execution::submit::CommandQueue::submit (line 241) - compile fail ... ok
test crates/core/src/execution/submit.rs - execution::submit::SubmitToken (line 47) - compile fail ... ok
test crates/core/src/execution/submit.rs - execution::submit::CommandQueue::dispatch_ex (line 337) - compile fail ... ok
test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

出力 (4)

```text
    #[doc(hidden)]
    pub fn __backend_new(id: u64, closer: Option<Box<dyn __BackendEncoderCloser>>) -> Self {
--
    #[doc(hidden)]
    pub fn __backend_new_shared(
--
    #[doc(hidden)]
    pub fn __backend_id(&self) -> u64 {
--
    #[doc(hidden)]
    pub fn __backend_record(&mut self) {
--
    #[doc(hidden)]
    pub fn __backend_disarm_closer(&mut self) {
```

出力 (5)

```text
crates/core/src/execution/compute.rs:119:pub struct ConstantBinding<'a> {
crates/core/src/execution/compute.rs:126:impl<'a> ConstantBinding<'a> {
crates/core/src/execution/submit.rs:386:    ///         _c: &[ConstantBinding<'_>],
crates/core/src/execution/submit.rs:411:        consts: &[ConstantBinding<'_>],
crates/core/src/graphics.rs:299:        vertex_constants: &[ConstantBinding<'_>],
crates/core/src/stub_device.rs:458:        _consts: &[ConstantBinding<'_>],
crates/core/src/stub_device.rs:688:        _vertex_constants: &[ConstantBinding<'_>],
```

補い（TASK-150 の後）: 上の出力の `stub_device.rs:458`・`:688` は、今は `crates/core/src/stub_device.rs` の `:462`・`:740` である。

出力 (6)

```text
crates/core/tests/dispatch_constants_per_call.rs:76:    #[cfg(feature = "native-blocking-wait")]
crates/core/tests/encoder_borrow_lifetime.rs:80:    #[cfg(feature = "native-blocking-wait")]
```

出力 (8)（1/3 は `b5671aa`、2/3 は `3ba90b6`。どちらも `-- crates/core/src/stub_device.rs crates/contract` を付けた出力は空、終了コード 0）

```text
$ git show --stat --format= b5671aa -- crates/
crates/core/src/execution/submit.rs          |  21 +++
crates/core/tests/encoder_borrow_lifetime.rs | 219 +++++++++++++++++++++++++++
2 files changed, 240 insertions(+)
$ git show --stat --format= 3ba90b6 -- crates/
crates/core/tests/dispatch_constants_per_call.rs | 202 +++++++++++++++++++++++
1 file changed, 202 insertions(+)
```

## 段の表

| 段 | イシュー | 内容 | 本リポの物 |
| -- | -------- | ---- | ---------- |
| 1/3 | #7692 | 記録先の型に借用の寿命の引数が無いことの試験 | 本書の「記録先の型と借用の試験（1/3）」 |
| 2/3 | #7693 | 定数を dispatch ごとに渡す形の試験 | 本書の「定数を dispatch ごとに渡す形の試験（2/3）」 |
| 3/3 | #7694 | 完了の条件の確かめ | 本書の「完了の条件の確かめ（3/3）」 |
