//! `BufferHandle` の二重の `free` が型として拒否されることを rustdoc の doctest で確かめる置き場（`doc(hidden)`）。
//!
//! `MemoryOps::free` は `BufferHandle` を値で受け取り、`BufferHandle` は `Clone`・`Copy` を持たない。
//! 同じ変数で 2 回呼ぶコードは値の move の後の使用（E0382）でコンパイルできない。
//! 背後実装（Metal / Vulkan / CUDA）には依らないため、実装は書かず `&dyn MemoryOps` を受ける関数で確かめる。
//! 実行時の二重解放の検査はこの試験の範囲の外。
//!
//! core の型は feature `test-support` を有効にした構成の doctest から届くため、モジュールごとその裏に置く
//! （`make verify` の `--all-features` と `scripts/build-profiles.sh` の `test-support` を含む組で流れる）。
//! stable の rustdoc は `compile_fail` のエラーコードを照合しないので、
//! 2 回目の `free` の有無だけが違う通る例を対にして、import とシグネチャの正しさを担保する。
//! spec との対応: TASK-308 の (2)、MEM-7（ポインタ表記）。

/// 1 回だけ `free` する例は通る（下の `compile_fail` の例の対照）。
///
/// ```
/// use fandhe_silicon_core::memory::{BufferHandle, MemoryOps};
///
/// fn release(ops: &dyn MemoryOps, buffer: BufferHandle) {
///     ops.free(buffer);
/// }
/// ```
#[allow(dead_code)] // doctest を載せるためだけのアイテムで、実行時には使わない。
pub struct FreeOnce;

/// 同じ `BufferHandle` に対する 2 回目の `free` は move 済みの値の使用でコンパイルエラーになる。
///
/// ```compile_fail,E0382
/// use fandhe_silicon_core::memory::{BufferHandle, MemoryOps};
///
/// fn release(ops: &dyn MemoryOps, buffer: BufferHandle) {
///     ops.free(buffer);
///     ops.free(buffer);
/// }
/// ```
#[allow(dead_code)] // doctest を載せるためだけのアイテムで、実行時には使わない。
pub struct FreeTwice;

/// `BufferHandle` は `Clone` を実装しないので、複製して 2 回目の `free` に渡す逃げ道も無い。
///
/// ```compile_fail,E0599
/// use fandhe_silicon_core::memory::{BufferHandle, MemoryOps};
///
/// fn release(ops: &dyn MemoryOps, buffer: BufferHandle) {
///     let copy = buffer.clone();
///     ops.free(buffer);
///     ops.free(copy);
/// }
/// ```
#[allow(dead_code)] // doctest を載せるためだけのアイテムで、実行時には使わない。
pub struct NoClone;

/// `BufferHandle` は `Copy` を実装しない。
///
/// ```compile_fail,E0277
/// use fandhe_silicon_core::memory::BufferHandle;
///
/// fn need_copy<T: Copy>() {}
/// need_copy::<BufferHandle>();
/// ```
#[allow(dead_code)] // doctest を載せるためだけのアイテムで、実行時には使わない。
pub struct NoCopy;
