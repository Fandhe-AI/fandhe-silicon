//! feature `graphics` の mesh shader の描画の 4 操作の引数と戻り値の型を、
//! 公開のパスだけで外の crate から確かめる統合テスト（TASK-839）。

// 引数と戻り値の型を `fn` ポインタの型として書き下すこと自体が試験の目的なので、複雑さの警告は許す。
#![allow(clippy::type_complexity)]

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::graphics::{GraphicsOps, RenderEncoder};
use fandhe_silicon_core::memory::BufferHandle;

#[test]
fn mesh_op_signatures_have_listed_types() {
    let _: fn(&dyn GraphicsOps, &mut RenderEncoder, [u32; 3]) -> Result<(), ContractError> =
        |g, e, n| g.draw_mesh_tasks(e, n);
    let _: fn(
        &dyn GraphicsOps,
        &mut RenderEncoder,
        &BufferHandle,
        u64,
    ) -> Result<(), ContractError> = |g, e, b, o| g.draw_mesh_tasks_indirect(e, b, o);
    let _: fn(
        &dyn GraphicsOps,
        &mut RenderEncoder,
        &BufferHandle,
        u64,
        u32,
    ) -> Result<(), ContractError> = |g, e, b, o, c| g.multi_draw_mesh_tasks_indirect(e, b, o, c);
    let _: fn(
        &dyn GraphicsOps,
        &mut RenderEncoder,
        &BufferHandle,
        u64,
        &BufferHandle,
        u64,
        u32,
    ) -> Result<(), ContractError> =
        |g, e, b, o, cb, co, m| g.multi_draw_mesh_tasks_indirect_count(e, b, o, cb, co, m);
}
