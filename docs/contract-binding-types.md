# 束縛の宣言・束縛の組・配置・パイプラインの配置の公開の型（TASK-419）の所在と本リポの現状

TASK-419 の作業の所在と、本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
対応づけは BIND-5・BIND-11・BIND-12・BIND-13 のポインタだけにする。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-419。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 公開に足した名前

すべて `crates/core/src/binding.rs`（`fandhe_silicon_core::binding`。feature の条件なし）。

| 名前 | 種類 |
| ---- | ---- |
| `BindingDeclaration` | 構造体（`#[non_exhaustive]`。`binding`・`visibility`・`ty` が `pub`。`new` で作る） |
| `BindingType` | enum（`#[non_exhaustive]`。変種は `Buffer { ty }` だけで、変種も `#[non_exhaustive]`。`buffer` で作る） |
| `BufferBindingType` | enum（`#[non_exhaustive]`。`Uniform`・`Storage { read_only }`） |
| `ShaderStageSet` | 構造体（内部のビットは非公開。`NONE`・単独の 9 値・`VERTEX_FRAGMENT` の定数、`is_empty`・`contains`・`BitOr`・`BitOrAssign`） |
| `BindGroupLayoutDescriptor<'a>` | 構造体（`#[non_exhaustive]`。`entries` が `pub`。`new` で作る） |
| `BindGroupLayout` | 構造体（欄は非公開。`entries()` で読む） |
| `PipelineLayoutDescriptor<'a>` | 構造体（`#[non_exhaustive]`。`bind_group_layouts`・`immediate_size` が `pub`。`new` で作る） |
| `PipelineLayout` | 構造体（欄は非公開。`bind_group_layouts()`・`immediate_size()` で読む。寿命の引数を持たない） |
| `BindingResource<'a>` | enum（`#[non_exhaustive]`。変種は `Buffer` だけ） |
| `BufferBinding<'a>` | 構造体（`#[non_exhaustive]`。`buffer`・`offset`・`size` が `pub`。`size` は `Option<NonZeroU64>`。`new` で作る） |
| `BindGroupEntry<'a>` | 構造体（`#[non_exhaustive]`。`binding`・`resource` が `pub`。`new` で作る） |
| `BindGroup<'a>` | 構造体（欄は非公開。`Clone`・`Copy` なし。`layout()`・`entries()` で読む） |
| `__backend_new`・`__backend_device_id` | `BindGroupLayout`・`PipelineLayout`・`BindGroup` の `#[doc(hidden)]` の口（背後実装向け） |

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 束縛の種類 | `BindingType` の変種は `Buffer` だけ。読み書きの別は `BufferBindingType::Storage { read_only }` の中にある | `crates/core/tests/binding_types.rs` |
| 可視性 | `ShaderStageSet` の単独の定数 9 値は互いに重ならず、`VERTEX_FRAGMENT` は `VERTEX | FRAGMENT` に等しい。ビット列から値は作れず、ビットの値を読む口も無い | `binding_types.rs`、rustdoc の `compile_fail,E0423` |
| 構築の口 | 外の crate は `#[non_exhaustive]` の型を構造体式で作れず、`new` / `buffer` を使う | rustdoc の `compile_fail,E0639` |
| 網羅の `match` | `BindingType`・`BufferBindingType`・`BindingResource` の `match` は `_` の腕が要る | `crates/core/tests/binding_non_exhaustive.rs`、rustdoc の `compile_fail,E0004` |
| 純粋なデータ | `BindGroupLayout`・`PipelineLayout` は宣言・配置の写しを持ち、作った元の値を落としても読める。`Clone` と `PartialEq` を持つ | `binding_types.rs` |
| 組の借用 | `BindGroup` は配置と資源を借用し、`Clone` を持たない。組が生きている間は資源・配置を手放せない | rustdoc の `compile_fail,E0599`・`E0505`・`E0597` |
| 項目の写し | `BindGroup::__backend_new` は項目を複製して持ち、呼び手の配列を借用し続けない | `binding_types.rs` |
| 検査 | `__backend_new` は番号の重なり・`immediate_size`・種類と資源の食い違いなどを検査せず、渡された値をそのまま写す。読み出しも渡された値を返すだけで失敗しない | `binding_types.rs`（渡した値が読めること・重なる番号や 4 の倍数でない `immediate_size` などもそのまま写すこと） |
| 範囲の外 | 値を作る操作・作成時の検査・下位の API へ渡す口・`Result` を返す口・新しいエラーの値は無い | `crates/core/src/binding.rs` |
| 外部依存 | 足していない（`Cargo.lock`・`deps-approved.toml` は変更なし）。`unsafe` は無い | `make deps-check` |

## 対応するテスト

| テスト | 内容 |
| ------ | ---- |
| `crates/core/tests/binding_types.rs` | 宣言の欄と複製、種類の区別と `match`、可視性の重ならない定数と和、2 つの配置の写しと空き、組の配置の借用と項目の写し |
| `crates/core/tests/binding_non_exhaustive.rs` | 3 つの列挙が別の crate から `_` の腕つきでだけ `match` できること |
| `binding.rs` の rustdoc（`compile_fail,E0639`） | 宣言・種類の変種・2 つの記述子・`BufferBinding`・`BindGroupEntry` の構造体式が拒まれること。それぞれ通る書き方の陽性対照つき |
| 同（`compile_fail,E0004`・`E0423`） | `_` の腕のない `match`、`ShaderStageSet(1)` が拒まれること |
| 同（`compile_fail,E0451`） | `BindGroupLayout`・`PipelineLayout`・`BindGroup` の構造体式が拒まれること |
| 同（`compile_fail,E0599`・`E0505`・`E0597`） | `BindGroup` の `clone`、組が生きている間の資源・配置の手放しが拒まれること。資源を組の後に手放す書き方の陽性対照つき |

## 確かめ方

```bash
cargo test -p fandhe-silicon-core --test binding_types --test binding_non_exhaustive
cargo test -p fandhe-silicon-core --doc binding
make verify && make check && make deny
make public-api-check && make public-name-check && make public-api-webgpu-check && make wasm-verify
```
