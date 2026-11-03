//! feature `graphics` の描画の公開の型と `GraphicsOps` の形を、外の crate から公開のパスだけで確かめる統合テスト
//! （TASK-837 の 4/4 のポインタ）。
//!
//! 局所の型に 23 操作を実装し、`dyn GraphicsOps` 越しに呼ぶ。本体は固定の値を返すだけ。

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::execution::{
    BindingLayout, CommandEncoder, ConstantBinding, ImportedSemaphoreHandle, PipelineHandle,
    PipelineInput, SubmitToken,
};
use fandhe_silicon_core::graphics::{
    CompareFunction, DepthConfig, DepthFormat, GraphicsOps, IndexFormat, PixelFormat,
    RenderEncoder, ScissorRect, Viewport,
};
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};
use fandhe_silicon_core::render_target::RenderTargetHandle;

/// 試験用の局所の実装。固定の値を返す。
struct FixedGraphics;

impl GraphicsOps for FixedGraphics {
    fn create_render_pipeline(
        &self,
        _vs: PipelineInput<'_>,
        _fs: PipelineInput<'_>,
        _layout: &BindingLayout,
    ) -> Result<PipelineHandle, ContractError> {
        Ok(PipelineHandle::__backend_new(1, 9))
    }

    fn create_render_pipeline_with_depth(
        &self,
        _vs: PipelineInput<'_>,
        _fs: PipelineInput<'_>,
        _layout: &BindingLayout,
        _depth_format: DepthFormat,
    ) -> Result<PipelineHandle, ContractError> {
        Ok(PipelineHandle::__backend_new(2, 9))
    }

    fn create_offscreen_target(
        &self,
        _width: u32,
        _height: u32,
        _format: PixelFormat,
    ) -> Result<RenderTargetHandle, ContractError> {
        Ok(RenderTargetHandle::__backend_new(1, 3, 9))
    }

    fn create_offscreen_target_with_depth(
        &self,
        _width: u32,
        _height: u32,
        _color_format: PixelFormat,
        _depth: DepthConfig,
    ) -> Result<(RenderTargetHandle, RenderTargetHandle), ContractError> {
        Ok((
            RenderTargetHandle::__backend_new(1, 4, 9),
            RenderTargetHandle::__backend_new(1, 5, 9),
        ))
    }

    fn destroy_offscreen_target(&self, _target: RenderTargetHandle) {}

    fn create_render_encoder(&self, _target: &RenderTargetHandle) -> RenderEncoder {
        RenderEncoder::__backend_new(1, 6, 9)
    }

    fn create_render_encoder_with_depth(
        &self,
        _target: &RenderTargetHandle,
        _depth_target: &RenderTargetHandle,
        _clear_color: [f32; 4],
    ) -> RenderEncoder {
        RenderEncoder::__backend_new(1, 7, 9)
    }

    fn draw(
        &self,
        _encoder: &mut RenderEncoder,
        _pipeline: &PipelineHandle,
        _vertex_buffer: &BufferHandle,
        _vertex_count: u32,
    ) -> Result<(), ContractError> {
        Ok(())
    }

    fn draw_ex(
        &self,
        _encoder: &mut RenderEncoder,
        _pipeline: &PipelineHandle,
        _vertex_buffer: &BufferHandle,
        _vertex_count: u32,
        _vertex_constants: &[ConstantBinding<'_>],
    ) -> Result<(), ContractError> {
        Err(ContractError::DeviceMismatch)
    }

    fn set_pipeline(
        &self,
        _encoder: &mut RenderEncoder,
        _pipeline: &PipelineHandle,
    ) -> Result<(), ContractError> {
        Ok(())
    }

    fn set_vertex_buffer(
        &self,
        _encoder: &mut RenderEncoder,
        _slot: u32,
        _buffer: &BufferHandle,
        _offset: u64,
        _size: Option<u64>,
    ) -> Result<(), ContractError> {
        Ok(())
    }

    fn set_index_buffer(
        &self,
        _encoder: &mut RenderEncoder,
        _buffer: &BufferHandle,
        _format: IndexFormat,
        _offset: u64,
        _size: Option<u64>,
    ) -> Result<(), ContractError> {
        Ok(())
    }

    fn set_immediates(
        &self,
        _encoder: &mut RenderEncoder,
        _offset: u32,
        _data: &[u8],
    ) -> Result<(), ContractError> {
        Ok(())
    }

    fn set_viewport(
        &self,
        _encoder: &mut RenderEncoder,
        _viewport: Viewport,
    ) -> Result<(), ContractError> {
        Ok(())
    }

    fn set_scissor_rect(
        &self,
        _encoder: &mut RenderEncoder,
        _rect: ScissorRect,
    ) -> Result<(), ContractError> {
        Ok(())
    }

    fn set_blend_constant(
        &self,
        _encoder: &mut RenderEncoder,
        _color: [f32; 4],
    ) -> Result<(), ContractError> {
        Ok(())
    }

    fn set_stencil_reference(
        &self,
        _encoder: &mut RenderEncoder,
        _reference: u32,
    ) -> Result<(), ContractError> {
        Ok(())
    }

    fn draw_mesh_tasks(
        &self,
        _encoder: &mut RenderEncoder,
        _group_count: [u32; 3],
    ) -> Result<(), ContractError> {
        Ok(())
    }

    fn draw_mesh_tasks_indirect(
        &self,
        _encoder: &mut RenderEncoder,
        _indirect: &BufferHandle,
        _indirect_offset: u64,
    ) -> Result<(), ContractError> {
        Ok(())
    }

    fn multi_draw_mesh_tasks_indirect(
        &self,
        _encoder: &mut RenderEncoder,
        _indirect: &BufferHandle,
        _indirect_offset: u64,
        _count: u32,
    ) -> Result<(), ContractError> {
        Ok(())
    }

    fn multi_draw_mesh_tasks_indirect_count(
        &self,
        _encoder: &mut RenderEncoder,
        _indirect: &BufferHandle,
        _indirect_offset: u64,
        _count_buffer: &BufferHandle,
        _count_offset: u64,
        _max_count: u32,
    ) -> Result<(), ContractError> {
        Ok(())
    }

    fn submit_render(
        &self,
        encoder: RenderEncoder,
        _wait: &[ImportedSemaphoreHandle],
    ) -> Result<SubmitToken, ContractError> {
        Ok(SubmitToken::__backend_new(
            encoder.__backend_generation(),
            encoder.__backend_id(),
            encoder.__backend_device_id(),
        ))
    }

    fn copy_target_to_buffer(
        &self,
        _encoder: &mut CommandEncoder,
        _target: &RenderTargetHandle,
        _dst: &BufferHandle,
        _dst_offset: u64,
    ) -> Result<(), ContractError> {
        Ok(())
    }
}

fn depth_config() -> DepthConfig {
    DepthConfig {
        format: DepthFormat::Depth32Float,
        clear_value: 1.0,
        compare: CompareFunction::LessEqual,
        write_enabled: true,
    }
}

fn msl(entry_point: &'static str) -> PipelineInput<'static> {
    PipelineInput::Msl {
        source: "",
        entry_point,
    }
}

#[test]
fn graphics_ops_is_implementable_and_object_safe() {
    let g: Box<dyn GraphicsOps> = Box::new(FixedGraphics);
    let layout = BindingLayout::new(vec![]);

    let p = g
        .create_render_pipeline(msl("vs"), msl("fs"), &layout)
        .unwrap();
    assert_eq!((p.__backend_id(), p.__backend_device_id()), (1, 9));
    let pd = g
        .create_render_pipeline_with_depth(msl("vs"), msl("fs"), &layout, DepthFormat::Depth32Float)
        .unwrap();
    assert_eq!((pd.__backend_id(), pd.__backend_device_id()), (2, 9));

    let target = g
        .create_offscreen_target(4, 4, PixelFormat::Rgba8Unorm)
        .unwrap();
    assert_eq!(target.__backend_id(), 3);
    let (color, depth) = g
        .create_offscreen_target_with_depth(4, 4, PixelFormat::Bgra8Unorm, depth_config())
        .unwrap();
    assert_eq!((color.__backend_id(), depth.__backend_id()), (4, 5));

    let ed = g.create_render_encoder_with_depth(&color, &depth, [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(ed.__backend_id(), 7);
    g.destroy_offscreen_target(depth);

    let buf = BufferHandle::__backend_new(1, 8, 9, 64, BufferUsage::VERTEX);
    let mut e = g.create_render_encoder(&target);
    assert_eq!(e.__backend_id(), 6);
    assert_eq!(g.draw(&mut e, &p, &buf, 3), Ok(()));
    assert_eq!(
        g.draw_ex(
            &mut e,
            &p,
            &buf,
            3,
            &[ConstantBinding::new(0, &[1, 2, 3, 4])]
        ),
        Err(ContractError::DeviceMismatch)
    );

    let mut cmd = CommandEncoder::__backend_new(1, None);
    assert_eq!(g.copy_target_to_buffer(&mut cmd, &target, &buf, 0), Ok(()));

    let token = g.submit_render(e, &[]).unwrap();
    assert_eq!(token.generation, 1);
    assert_eq!(token.sequence, 6);
    assert_eq!(token.__backend_device_id(), 9);
}

#[test]
fn value_types_have_listed_variants() {
    // `#[non_exhaustive]` のため外の crate ではワイルドカードの腕が要る。
    fn pixel(p: PixelFormat) -> u8 {
        match p {
            PixelFormat::Rgba8Unorm => 0,
            PixelFormat::Bgra8Unorm => 1,
            PixelFormat::Rgba32Float => 2,
            _ => 99,
        }
    }
    fn depth(d: DepthFormat) -> u8 {
        match d {
            DepthFormat::Depth32Float => 0,
            _ => 99,
        }
    }
    fn compare(c: CompareFunction) -> u8 {
        match c {
            CompareFunction::Less => 0,
            CompareFunction::LessEqual => 1,
            CompareFunction::Always => 2,
            _ => 99,
        }
    }
    assert_eq!(pixel(PixelFormat::Rgba8Unorm), 0);
    assert_eq!(pixel(PixelFormat::Bgra8Unorm), 1);
    assert_eq!(pixel(PixelFormat::Rgba32Float), 2);
    assert_eq!(depth(DepthFormat::Depth32Float), 0);
    assert_eq!(compare(CompareFunction::Less), 0);
    assert_eq!(compare(CompareFunction::LessEqual), 1);
    assert_eq!(compare(CompareFunction::Always), 2);
}

#[test]
fn depth_config_is_built_from_outside() {
    let c = depth_config();
    let copy = c;
    assert_eq!(c, copy);
    let other = DepthConfig {
        write_enabled: false,
        ..c
    };
    assert_ne!(c, other);
    let other = DepthConfig {
        clear_value: 0.5,
        ..c
    };
    assert_ne!(c, other);
}

#[test]
fn render_encoder_is_static() {
    fn assert_static<T: 'static>() {}
    assert_static::<RenderEncoder>();
}
