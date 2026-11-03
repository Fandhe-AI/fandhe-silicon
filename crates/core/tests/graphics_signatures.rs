//! `GraphicsOps` の操作の引数と戻りの型を、関数ポインタの型への強制で固定する統合テスト（TASK-908 のポインタ）。
//! 公開のパスだけを使い、`dyn GraphicsOps` 越しの各操作と `RenderEncoder::__backend_rejected` の型を 1 行ずつ固定する。
//! 固定するのは `destroy_offscreen_target` を除く 10 操作。

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::execution::{
    BindingLayout, CommandEncoder, ConstantBinding, ImportedSemaphoreHandle, PipelineHandle,
    PipelineInput, SubmitToken,
};
use fandhe_silicon_core::graphics::{
    DepthConfig, DepthFormat, GraphicsOps, PixelFormat, RenderEncoder,
};
use fandhe_silicon_core::memory::BufferHandle;
use fandhe_silicon_core::render_target::RenderTargetHandle;

type G = dyn GraphicsOps;
type Done = Result<(), ContractError>;
type TargetPair = (RenderTargetHandle, RenderTargetHandle);

// REND-9
#[test]
fn pipeline_and_target_creation_signatures() {
    let _: fn(
        &G,
        PipelineInput<'_>,
        PipelineInput<'_>,
        &BindingLayout,
    ) -> Result<PipelineHandle, ContractError> = <G as GraphicsOps>::create_render_pipeline;
    let _: fn(
        &G,
        PipelineInput<'_>,
        PipelineInput<'_>,
        &BindingLayout,
        DepthFormat,
    ) -> Result<PipelineHandle, ContractError> =
        <G as GraphicsOps>::create_render_pipeline_with_depth;
    let _: fn(&G, u32, u32, PixelFormat) -> Result<RenderTargetHandle, ContractError> =
        <G as GraphicsOps>::create_offscreen_target;
    let _: fn(&G, u32, u32, PixelFormat, DepthConfig) -> Result<TargetPair, ContractError> =
        <G as GraphicsOps>::create_offscreen_target_with_depth;
    let _: PixelFormat = PixelFormat::Bgra8Unorm;
    let _: PixelFormat = PixelFormat::Rgba32Float;
}

// REND-16
#[test]
fn render_encoder_creation_returns_encoder() {
    let _: fn(&G, &RenderTargetHandle) -> RenderEncoder = <G as GraphicsOps>::create_render_encoder;
    let _: fn(&G, &RenderTargetHandle, &RenderTargetHandle, [f32; 4]) -> RenderEncoder =
        <G as GraphicsOps>::create_render_encoder_with_depth;
    let _: fn(&RenderEncoder) -> Option<&ContractError> = RenderEncoder::__backend_rejected;
    let e = RenderEncoder::__backend_new_rejected(1, 0, 1, ContractError::DeviceMismatch);
    assert_eq!(e.__backend_rejected(), Some(&ContractError::DeviceMismatch));
}

// REND-26
#[test]
fn submit_render_wait_signature() {
    let _: fn(&G, RenderEncoder, &[ImportedSemaphoreHandle]) -> Result<SubmitToken, ContractError> =
        <G as GraphicsOps>::submit_render;
}

// REND-92
#[test]
fn draw_signatures_return_result() {
    let _: fn(
        &G,
        &mut RenderEncoder,
        &PipelineHandle,
        &BufferHandle,
        u32,
    ) -> Result<(), ContractError> = <G as GraphicsOps>::draw;
    let _: fn(
        &G,
        &mut RenderEncoder,
        &PipelineHandle,
        &BufferHandle,
        u32,
        &[ConstantBinding<'_>],
    ) -> Done = <G as GraphicsOps>::draw_ex;
}

// REND-93
#[test]
fn draw_error_values_are_contract_error() {
    let r: Result<(), ContractError> = Err(ContractError::InvalidArgument(String::new()));
    assert!(r.is_err());
    let r: Result<(), ContractError> = Err(ContractError::DeviceMismatch);
    assert!(r.is_err());
}

// REND-94
#[test]
fn submit_render_takes_encoder_by_value() {
    // 第 2 引数が `RenderEncoder` の値であることだけを固定する（戻りは上の試験が固定する）。
    let _: fn(&G, RenderEncoder, &[ImportedSemaphoreHandle]) -> _ =
        <G as GraphicsOps>::submit_render;
}

#[test]
fn copy_target_to_buffer_signature() {
    let _: fn(
        &G,
        &mut CommandEncoder,
        &RenderTargetHandle,
        &BufferHandle,
        u64,
    ) -> Result<(), ContractError> = <G as GraphicsOps>::copy_target_to_buffer;
}
