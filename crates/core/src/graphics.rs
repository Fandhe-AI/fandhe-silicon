//! 描画の公開の型と、描画の操作の trait [`GraphicsOps`] を置く module（TASK-837、DEV-63 のポインタ）。
//!
//! feature `graphics` の裏にあり、feature の無い構成では module ごと無い。
//! `PixelFormat`・`DepthFormat` から `TextureFormat` への変換（TASK-1312）も置く。
//! 描画パスの状態の値の型と、範囲つきの描画の範囲の型と検査も置く。
//! 描画パスの状態を設定する操作も trait にある。

use crate::error::ContractError;
use crate::execution::{
    BindingLayout, CommandEncoder, ConstantBinding, ImportedSemaphoreHandle, PipelineHandle,
    PipelineInput, SubmitToken,
};
use crate::memory::BufferHandle;
use crate::render_target::RenderTargetHandle;
use crate::texture_format::TextureFormat;

/// 色の描画先の画素の形式。外の crate からは、ワイルドカードの腕の無い `match` は書けない。
///
/// ```compile_fail,E0004
/// use fandhe_silicon_core::graphics::PixelFormat;
/// fn f(p: PixelFormat) -> u8 {
///     match p {
///         PixelFormat::Rgba8Unorm => 0,
///         PixelFormat::Bgra8Unorm => 1,
///         PixelFormat::Rgba32Float => 2,
///     }
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PixelFormat {
    /// 8 bit 4 成分の正規化整数（R, G, B, A の順）。
    Rgba8Unorm,
    /// 8 bit 4 成分の正規化整数（B, G, R, A の順）。
    Bgra8Unorm,
    /// 32 bit 浮動小数点 4 成分。
    Rgba32Float,
}

/// 深度の描画先の形式。外の crate からは、ワイルドカードの腕の無い `match` は書けない。
///
/// ```compile_fail,E0004
/// use fandhe_silicon_core::graphics::DepthFormat;
/// fn f(d: DepthFormat) -> u8 {
///     match d {
///         DepthFormat::Depth32Float => 0,
///     }
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DepthFormat {
    /// 32 bit 浮動小数点の深度。
    Depth32Float,
}

/// 同じ名前の `TextureFormat` の値を返す。失敗しない（TASK-1312）。
///
/// ```
/// use fandhe_silicon_core::graphics::PixelFormat;
/// use fandhe_silicon_core::texture_format::TextureFormat;
/// assert_eq!(TextureFormat::from(PixelFormat::Bgra8Unorm), TextureFormat::Bgra8Unorm);
/// ```
impl From<PixelFormat> for TextureFormat {
    fn from(p: PixelFormat) -> Self {
        match p {
            PixelFormat::Rgba8Unorm => TextureFormat::Rgba8Unorm,
            PixelFormat::Bgra8Unorm => TextureFormat::Bgra8Unorm,
            PixelFormat::Rgba32Float => TextureFormat::Rgba32Float,
        }
    }
}

/// 同じ名前の `TextureFormat` の値を返す。失敗しない（TASK-1312）。
///
/// ```
/// use fandhe_silicon_core::graphics::DepthFormat;
/// use fandhe_silicon_core::texture_format::TextureFormat;
/// let f: TextureFormat = DepthFormat::Depth32Float.into();
/// assert_eq!(f, TextureFormat::Depth32Float);
/// ```
impl From<DepthFormat> for TextureFormat {
    fn from(d: DepthFormat) -> Self {
        match d {
            DepthFormat::Depth32Float => TextureFormat::Depth32Float,
        }
    }
}

/// 深度の比較関数。外の crate からは、ワイルドカードの腕の無い `match` は書けない。
///
/// ```compile_fail,E0004
/// use fandhe_silicon_core::graphics::CompareFunction;
/// fn f(c: CompareFunction) -> u8 {
///     match c {
///         CompareFunction::Less => 0,
///         CompareFunction::LessEqual => 1,
///         CompareFunction::Always => 2,
///     }
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CompareFunction {
    /// 手前なら通す。
    Less,
    /// 手前か同じなら通す。
    LessEqual,
    /// 常に通す。
    Always,
}

/// 深度の描画先の設定。欄は公開で、外の crate が構造体式で作れる。
///
/// ```
/// use fandhe_silicon_core::graphics::{CompareFunction, DepthConfig, DepthFormat};
/// let c = DepthConfig {
///     format: DepthFormat::Depth32Float,
///     clear_value: 1.0,
///     compare: CompareFunction::Less,
///     write_enabled: true,
/// };
/// assert_eq!(c, c);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DepthConfig {
    /// 深度の形式。
    pub format: DepthFormat,
    /// 消去に使う深度の値。
    pub clear_value: f32,
    /// 深度の比較関数。
    pub compare: CompareFunction,
    /// 深度の書き込みを許すか。
    pub write_enabled: bool,
}

/// 描画の記録先。欄は非公開で、`Clone`・`Copy` を実装しない。[`GraphicsOps::submit_render`] に値で渡す。
///
/// `__backend_*` は背後実装だけが使う口で、`__backend_new` は safe で誰でも値を作れる。
/// 読み出しの口は組み立てで渡した値を返す。
///
/// ```
/// use fandhe_silicon_core::error::ContractError;
/// use fandhe_silicon_core::graphics::RenderEncoder;
/// let e = RenderEncoder::__backend_new(5, 3, 1);
/// assert_eq!(e.__backend_generation(), 5);
/// assert_eq!(e.__backend_id(), 3);
/// assert_eq!(e.__backend_device_id(), 1);
/// assert_eq!(e.__backend_rejected(), None);
/// let r = RenderEncoder::__backend_new_rejected(5, 3, 1, ContractError::DeviceMismatch);
/// assert_eq!(r.__backend_rejected(), Some(&ContractError::DeviceMismatch));
/// ```
///
/// 構造体式は拒まれる。
///
/// ```compile_fail,E0451
/// use fandhe_silicon_core::graphics::RenderEncoder;
/// let _ = RenderEncoder { generation: 5, id: 3, device_id: 1, rejected: None };
/// ```
///
/// `clone` は呼べない。
///
/// ```compile_fail,E0599
/// use fandhe_silicon_core::graphics::RenderEncoder;
/// let e = RenderEncoder::__backend_new(5, 3, 1);
/// let _ = e.clone();
/// ```
#[derive(Debug)]
pub struct RenderEncoder {
    generation: u64,
    id: u64,
    device_id: u64,
    rejected: Option<ContractError>,
}

impl RenderEncoder {
    /// 背後実装向けの組み立ての口。検査はしない。
    #[doc(hidden)]
    pub fn __backend_new(generation: u64, id: u64, device_id: u64) -> Self {
        Self {
            generation,
            id,
            device_id,
            rejected: None,
        }
    }

    /// 背後実装向けの組み立ての口（エラーの値つき）。検査はしない。
    #[doc(hidden)]
    pub fn __backend_new_rejected(
        generation: u64,
        id: u64,
        device_id: u64,
        error: ContractError,
    ) -> Self {
        Self {
            generation,
            id,
            device_id,
            rejected: Some(error),
        }
    }

    /// 背後実装向けの読み出しの口（世代）。
    #[doc(hidden)]
    pub fn __backend_generation(&self) -> u64 {
        self.generation
    }

    /// 背後実装向けの読み出しの口（識別子）。
    #[doc(hidden)]
    pub fn __backend_id(&self) -> u64 {
        self.id
    }

    /// 背後実装向けの読み出しの口（所属する device の識別子）。
    #[doc(hidden)]
    pub fn __backend_device_id(&self) -> u64 {
        self.device_id
    }

    /// 背後実装向けの読み出しの口（組み立てで渡したエラーの値）。
    #[doc(hidden)]
    pub fn __backend_rejected(&self) -> Option<&ContractError> {
        self.rejected.as_ref()
    }
}

/// 描画の操作の trait。背後実装が実装し、`Device::graphics()` から届く（TASK-837 のポインタ）。
///
/// `&dyn GraphicsOps` として書ける。
///
/// ```
/// use fandhe_silicon_core::graphics::{GraphicsOps, RenderEncoder};
/// fn f(g: &dyn GraphicsOps, e: RenderEncoder) {
///     let _ = g.submit_render(e, &[]);
/// }
/// ```
///
/// 描画パスの状態を設定する 8 操作（`set_pipeline`・`set_vertex_buffer`・`set_index_buffer`・`set_immediates`・`set_viewport`・`set_scissor_rect`・`set_blend_constant`・`set_stencil_reference`）に既定の本体は無い。
/// 次の例は、1 操作だけを持たない実装がそれぞれ拒まれることを示す。
///
/// `set_pipeline` だけを持たない実装は拒まれる。
///
/// ```compile_fail,E0046
/// # use fandhe_silicon_core::error::ContractError;
/// # use fandhe_silicon_core::execution::{BindingLayout, CommandEncoder, ConstantBinding, ImportedSemaphoreHandle, PipelineHandle, PipelineInput, SubmitToken};
/// # use fandhe_silicon_core::graphics::{DepthConfig, DepthFormat, GraphicsOps, IndexFormat, PixelFormat, RenderEncoder, ScissorRect, Viewport};
/// # use fandhe_silicon_core::memory::BufferHandle;
/// # use fandhe_silicon_core::render_target::RenderTargetHandle;
/// struct G;
/// impl GraphicsOps for G {
///     fn create_render_pipeline(&self, _vs: PipelineInput<'_>, _fs: PipelineInput<'_>, _layout: &BindingLayout) -> Result<PipelineHandle, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn create_render_pipeline_with_depth(&self, _vs: PipelineInput<'_>, _fs: PipelineInput<'_>, _layout: &BindingLayout, _depth_format: DepthFormat) -> Result<PipelineHandle, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn create_offscreen_target(&self, _width: u32, _height: u32, _format: PixelFormat) -> Result<RenderTargetHandle, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn create_offscreen_target_with_depth(&self, _width: u32, _height: u32, _color_format: PixelFormat, _depth: DepthConfig) -> Result<(RenderTargetHandle, RenderTargetHandle), ContractError> { Err(ContractError::DeviceMismatch) }
///     fn destroy_offscreen_target(&self, _target: RenderTargetHandle) {}
///     fn create_render_encoder(&self, _target: &RenderTargetHandle) -> RenderEncoder { loop {} }
///     fn create_render_encoder_with_depth(&self, _target: &RenderTargetHandle, _depth_target: &RenderTargetHandle, _clear_color: [f32; 4]) -> RenderEncoder { loop {} }
///     fn draw(&self, _encoder: &mut RenderEncoder, _pipeline: &PipelineHandle, _vertex_buffer: &BufferHandle, _vertex_count: u32) -> Result<(), ContractError> { Ok(()) }
///     fn draw_ex(&self, _encoder: &mut RenderEncoder, _pipeline: &PipelineHandle, _vertex_buffer: &BufferHandle, _vertex_count: u32, _vertex_constants: &[ConstantBinding<'_>]) -> Result<(), ContractError> { Ok(()) }
///     fn set_vertex_buffer(&self, _encoder: &mut RenderEncoder, _slot: u32, _buffer: &BufferHandle, _offset: u64, _size: Option<u64>) -> Result<(), ContractError> { Ok(()) }
///     fn set_index_buffer(&self, _encoder: &mut RenderEncoder, _buffer: &BufferHandle, _format: IndexFormat, _offset: u64, _size: Option<u64>) -> Result<(), ContractError> { Ok(()) }
///     fn set_immediates(&self, _encoder: &mut RenderEncoder, _offset: u32, _data: &[u8]) -> Result<(), ContractError> { Ok(()) }
///     fn set_viewport(&self, _encoder: &mut RenderEncoder, _viewport: Viewport) -> Result<(), ContractError> { Ok(()) }
///     fn set_scissor_rect(&self, _encoder: &mut RenderEncoder, _rect: ScissorRect) -> Result<(), ContractError> { Ok(()) }
///     fn set_blend_constant(&self, _encoder: &mut RenderEncoder, _color: [f32; 4]) -> Result<(), ContractError> { Ok(()) }
///     fn set_stencil_reference(&self, _encoder: &mut RenderEncoder, _reference: u32) -> Result<(), ContractError> { Ok(()) }
///     fn submit_render(&self, _encoder: RenderEncoder, _wait: &[ImportedSemaphoreHandle]) -> Result<SubmitToken, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn copy_target_to_buffer(&self, _encoder: &mut CommandEncoder, _target: &RenderTargetHandle, _dst: &BufferHandle, _dst_offset: u64) -> Result<(), ContractError> { Ok(()) }
/// }
/// ```
///
/// `set_vertex_buffer` だけを持たない実装は拒まれる。
///
/// ```compile_fail,E0046
/// # use fandhe_silicon_core::error::ContractError;
/// # use fandhe_silicon_core::execution::{BindingLayout, CommandEncoder, ConstantBinding, ImportedSemaphoreHandle, PipelineHandle, PipelineInput, SubmitToken};
/// # use fandhe_silicon_core::graphics::{DepthConfig, DepthFormat, GraphicsOps, IndexFormat, PixelFormat, RenderEncoder, ScissorRect, Viewport};
/// # use fandhe_silicon_core::memory::BufferHandle;
/// # use fandhe_silicon_core::render_target::RenderTargetHandle;
/// struct G;
/// impl GraphicsOps for G {
///     fn create_render_pipeline(&self, _vs: PipelineInput<'_>, _fs: PipelineInput<'_>, _layout: &BindingLayout) -> Result<PipelineHandle, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn create_render_pipeline_with_depth(&self, _vs: PipelineInput<'_>, _fs: PipelineInput<'_>, _layout: &BindingLayout, _depth_format: DepthFormat) -> Result<PipelineHandle, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn create_offscreen_target(&self, _width: u32, _height: u32, _format: PixelFormat) -> Result<RenderTargetHandle, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn create_offscreen_target_with_depth(&self, _width: u32, _height: u32, _color_format: PixelFormat, _depth: DepthConfig) -> Result<(RenderTargetHandle, RenderTargetHandle), ContractError> { Err(ContractError::DeviceMismatch) }
///     fn destroy_offscreen_target(&self, _target: RenderTargetHandle) {}
///     fn create_render_encoder(&self, _target: &RenderTargetHandle) -> RenderEncoder { loop {} }
///     fn create_render_encoder_with_depth(&self, _target: &RenderTargetHandle, _depth_target: &RenderTargetHandle, _clear_color: [f32; 4]) -> RenderEncoder { loop {} }
///     fn draw(&self, _encoder: &mut RenderEncoder, _pipeline: &PipelineHandle, _vertex_buffer: &BufferHandle, _vertex_count: u32) -> Result<(), ContractError> { Ok(()) }
///     fn draw_ex(&self, _encoder: &mut RenderEncoder, _pipeline: &PipelineHandle, _vertex_buffer: &BufferHandle, _vertex_count: u32, _vertex_constants: &[ConstantBinding<'_>]) -> Result<(), ContractError> { Ok(()) }
///     fn set_pipeline(&self, _encoder: &mut RenderEncoder, _pipeline: &PipelineHandle) -> Result<(), ContractError> { Ok(()) }
///     fn set_index_buffer(&self, _encoder: &mut RenderEncoder, _buffer: &BufferHandle, _format: IndexFormat, _offset: u64, _size: Option<u64>) -> Result<(), ContractError> { Ok(()) }
///     fn set_immediates(&self, _encoder: &mut RenderEncoder, _offset: u32, _data: &[u8]) -> Result<(), ContractError> { Ok(()) }
///     fn set_viewport(&self, _encoder: &mut RenderEncoder, _viewport: Viewport) -> Result<(), ContractError> { Ok(()) }
///     fn set_scissor_rect(&self, _encoder: &mut RenderEncoder, _rect: ScissorRect) -> Result<(), ContractError> { Ok(()) }
///     fn set_blend_constant(&self, _encoder: &mut RenderEncoder, _color: [f32; 4]) -> Result<(), ContractError> { Ok(()) }
///     fn set_stencil_reference(&self, _encoder: &mut RenderEncoder, _reference: u32) -> Result<(), ContractError> { Ok(()) }
///     fn submit_render(&self, _encoder: RenderEncoder, _wait: &[ImportedSemaphoreHandle]) -> Result<SubmitToken, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn copy_target_to_buffer(&self, _encoder: &mut CommandEncoder, _target: &RenderTargetHandle, _dst: &BufferHandle, _dst_offset: u64) -> Result<(), ContractError> { Ok(()) }
/// }
/// ```
///
/// `set_index_buffer` だけを持たない実装は拒まれる。
///
/// ```compile_fail,E0046
/// # use fandhe_silicon_core::error::ContractError;
/// # use fandhe_silicon_core::execution::{BindingLayout, CommandEncoder, ConstantBinding, ImportedSemaphoreHandle, PipelineHandle, PipelineInput, SubmitToken};
/// # use fandhe_silicon_core::graphics::{DepthConfig, DepthFormat, GraphicsOps, IndexFormat, PixelFormat, RenderEncoder, ScissorRect, Viewport};
/// # use fandhe_silicon_core::memory::BufferHandle;
/// # use fandhe_silicon_core::render_target::RenderTargetHandle;
/// struct G;
/// impl GraphicsOps for G {
///     fn create_render_pipeline(&self, _vs: PipelineInput<'_>, _fs: PipelineInput<'_>, _layout: &BindingLayout) -> Result<PipelineHandle, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn create_render_pipeline_with_depth(&self, _vs: PipelineInput<'_>, _fs: PipelineInput<'_>, _layout: &BindingLayout, _depth_format: DepthFormat) -> Result<PipelineHandle, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn create_offscreen_target(&self, _width: u32, _height: u32, _format: PixelFormat) -> Result<RenderTargetHandle, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn create_offscreen_target_with_depth(&self, _width: u32, _height: u32, _color_format: PixelFormat, _depth: DepthConfig) -> Result<(RenderTargetHandle, RenderTargetHandle), ContractError> { Err(ContractError::DeviceMismatch) }
///     fn destroy_offscreen_target(&self, _target: RenderTargetHandle) {}
///     fn create_render_encoder(&self, _target: &RenderTargetHandle) -> RenderEncoder { loop {} }
///     fn create_render_encoder_with_depth(&self, _target: &RenderTargetHandle, _depth_target: &RenderTargetHandle, _clear_color: [f32; 4]) -> RenderEncoder { loop {} }
///     fn draw(&self, _encoder: &mut RenderEncoder, _pipeline: &PipelineHandle, _vertex_buffer: &BufferHandle, _vertex_count: u32) -> Result<(), ContractError> { Ok(()) }
///     fn draw_ex(&self, _encoder: &mut RenderEncoder, _pipeline: &PipelineHandle, _vertex_buffer: &BufferHandle, _vertex_count: u32, _vertex_constants: &[ConstantBinding<'_>]) -> Result<(), ContractError> { Ok(()) }
///     fn set_pipeline(&self, _encoder: &mut RenderEncoder, _pipeline: &PipelineHandle) -> Result<(), ContractError> { Ok(()) }
///     fn set_vertex_buffer(&self, _encoder: &mut RenderEncoder, _slot: u32, _buffer: &BufferHandle, _offset: u64, _size: Option<u64>) -> Result<(), ContractError> { Ok(()) }
///     fn set_immediates(&self, _encoder: &mut RenderEncoder, _offset: u32, _data: &[u8]) -> Result<(), ContractError> { Ok(()) }
///     fn set_viewport(&self, _encoder: &mut RenderEncoder, _viewport: Viewport) -> Result<(), ContractError> { Ok(()) }
///     fn set_scissor_rect(&self, _encoder: &mut RenderEncoder, _rect: ScissorRect) -> Result<(), ContractError> { Ok(()) }
///     fn set_blend_constant(&self, _encoder: &mut RenderEncoder, _color: [f32; 4]) -> Result<(), ContractError> { Ok(()) }
///     fn set_stencil_reference(&self, _encoder: &mut RenderEncoder, _reference: u32) -> Result<(), ContractError> { Ok(()) }
///     fn submit_render(&self, _encoder: RenderEncoder, _wait: &[ImportedSemaphoreHandle]) -> Result<SubmitToken, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn copy_target_to_buffer(&self, _encoder: &mut CommandEncoder, _target: &RenderTargetHandle, _dst: &BufferHandle, _dst_offset: u64) -> Result<(), ContractError> { Ok(()) }
/// }
/// ```
///
/// `set_immediates` だけを持たない実装は拒まれる。
///
/// ```compile_fail,E0046
/// # use fandhe_silicon_core::error::ContractError;
/// # use fandhe_silicon_core::execution::{BindingLayout, CommandEncoder, ConstantBinding, ImportedSemaphoreHandle, PipelineHandle, PipelineInput, SubmitToken};
/// # use fandhe_silicon_core::graphics::{DepthConfig, DepthFormat, GraphicsOps, IndexFormat, PixelFormat, RenderEncoder, ScissorRect, Viewport};
/// # use fandhe_silicon_core::memory::BufferHandle;
/// # use fandhe_silicon_core::render_target::RenderTargetHandle;
/// struct G;
/// impl GraphicsOps for G {
///     fn create_render_pipeline(&self, _vs: PipelineInput<'_>, _fs: PipelineInput<'_>, _layout: &BindingLayout) -> Result<PipelineHandle, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn create_render_pipeline_with_depth(&self, _vs: PipelineInput<'_>, _fs: PipelineInput<'_>, _layout: &BindingLayout, _depth_format: DepthFormat) -> Result<PipelineHandle, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn create_offscreen_target(&self, _width: u32, _height: u32, _format: PixelFormat) -> Result<RenderTargetHandle, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn create_offscreen_target_with_depth(&self, _width: u32, _height: u32, _color_format: PixelFormat, _depth: DepthConfig) -> Result<(RenderTargetHandle, RenderTargetHandle), ContractError> { Err(ContractError::DeviceMismatch) }
///     fn destroy_offscreen_target(&self, _target: RenderTargetHandle) {}
///     fn create_render_encoder(&self, _target: &RenderTargetHandle) -> RenderEncoder { loop {} }
///     fn create_render_encoder_with_depth(&self, _target: &RenderTargetHandle, _depth_target: &RenderTargetHandle, _clear_color: [f32; 4]) -> RenderEncoder { loop {} }
///     fn draw(&self, _encoder: &mut RenderEncoder, _pipeline: &PipelineHandle, _vertex_buffer: &BufferHandle, _vertex_count: u32) -> Result<(), ContractError> { Ok(()) }
///     fn draw_ex(&self, _encoder: &mut RenderEncoder, _pipeline: &PipelineHandle, _vertex_buffer: &BufferHandle, _vertex_count: u32, _vertex_constants: &[ConstantBinding<'_>]) -> Result<(), ContractError> { Ok(()) }
///     fn set_pipeline(&self, _encoder: &mut RenderEncoder, _pipeline: &PipelineHandle) -> Result<(), ContractError> { Ok(()) }
///     fn set_vertex_buffer(&self, _encoder: &mut RenderEncoder, _slot: u32, _buffer: &BufferHandle, _offset: u64, _size: Option<u64>) -> Result<(), ContractError> { Ok(()) }
///     fn set_index_buffer(&self, _encoder: &mut RenderEncoder, _buffer: &BufferHandle, _format: IndexFormat, _offset: u64, _size: Option<u64>) -> Result<(), ContractError> { Ok(()) }
///     fn set_viewport(&self, _encoder: &mut RenderEncoder, _viewport: Viewport) -> Result<(), ContractError> { Ok(()) }
///     fn set_scissor_rect(&self, _encoder: &mut RenderEncoder, _rect: ScissorRect) -> Result<(), ContractError> { Ok(()) }
///     fn set_blend_constant(&self, _encoder: &mut RenderEncoder, _color: [f32; 4]) -> Result<(), ContractError> { Ok(()) }
///     fn set_stencil_reference(&self, _encoder: &mut RenderEncoder, _reference: u32) -> Result<(), ContractError> { Ok(()) }
///     fn submit_render(&self, _encoder: RenderEncoder, _wait: &[ImportedSemaphoreHandle]) -> Result<SubmitToken, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn copy_target_to_buffer(&self, _encoder: &mut CommandEncoder, _target: &RenderTargetHandle, _dst: &BufferHandle, _dst_offset: u64) -> Result<(), ContractError> { Ok(()) }
/// }
/// ```
///
/// `set_viewport` だけを持たない実装は拒まれる。
///
/// ```compile_fail,E0046
/// # use fandhe_silicon_core::error::ContractError;
/// # use fandhe_silicon_core::execution::{BindingLayout, CommandEncoder, ConstantBinding, ImportedSemaphoreHandle, PipelineHandle, PipelineInput, SubmitToken};
/// # use fandhe_silicon_core::graphics::{DepthConfig, DepthFormat, GraphicsOps, IndexFormat, PixelFormat, RenderEncoder, ScissorRect, Viewport};
/// # use fandhe_silicon_core::memory::BufferHandle;
/// # use fandhe_silicon_core::render_target::RenderTargetHandle;
/// struct G;
/// impl GraphicsOps for G {
///     fn create_render_pipeline(&self, _vs: PipelineInput<'_>, _fs: PipelineInput<'_>, _layout: &BindingLayout) -> Result<PipelineHandle, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn create_render_pipeline_with_depth(&self, _vs: PipelineInput<'_>, _fs: PipelineInput<'_>, _layout: &BindingLayout, _depth_format: DepthFormat) -> Result<PipelineHandle, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn create_offscreen_target(&self, _width: u32, _height: u32, _format: PixelFormat) -> Result<RenderTargetHandle, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn create_offscreen_target_with_depth(&self, _width: u32, _height: u32, _color_format: PixelFormat, _depth: DepthConfig) -> Result<(RenderTargetHandle, RenderTargetHandle), ContractError> { Err(ContractError::DeviceMismatch) }
///     fn destroy_offscreen_target(&self, _target: RenderTargetHandle) {}
///     fn create_render_encoder(&self, _target: &RenderTargetHandle) -> RenderEncoder { loop {} }
///     fn create_render_encoder_with_depth(&self, _target: &RenderTargetHandle, _depth_target: &RenderTargetHandle, _clear_color: [f32; 4]) -> RenderEncoder { loop {} }
///     fn draw(&self, _encoder: &mut RenderEncoder, _pipeline: &PipelineHandle, _vertex_buffer: &BufferHandle, _vertex_count: u32) -> Result<(), ContractError> { Ok(()) }
///     fn draw_ex(&self, _encoder: &mut RenderEncoder, _pipeline: &PipelineHandle, _vertex_buffer: &BufferHandle, _vertex_count: u32, _vertex_constants: &[ConstantBinding<'_>]) -> Result<(), ContractError> { Ok(()) }
///     fn set_pipeline(&self, _encoder: &mut RenderEncoder, _pipeline: &PipelineHandle) -> Result<(), ContractError> { Ok(()) }
///     fn set_vertex_buffer(&self, _encoder: &mut RenderEncoder, _slot: u32, _buffer: &BufferHandle, _offset: u64, _size: Option<u64>) -> Result<(), ContractError> { Ok(()) }
///     fn set_index_buffer(&self, _encoder: &mut RenderEncoder, _buffer: &BufferHandle, _format: IndexFormat, _offset: u64, _size: Option<u64>) -> Result<(), ContractError> { Ok(()) }
///     fn set_immediates(&self, _encoder: &mut RenderEncoder, _offset: u32, _data: &[u8]) -> Result<(), ContractError> { Ok(()) }
///     fn set_scissor_rect(&self, _encoder: &mut RenderEncoder, _rect: ScissorRect) -> Result<(), ContractError> { Ok(()) }
///     fn set_blend_constant(&self, _encoder: &mut RenderEncoder, _color: [f32; 4]) -> Result<(), ContractError> { Ok(()) }
///     fn set_stencil_reference(&self, _encoder: &mut RenderEncoder, _reference: u32) -> Result<(), ContractError> { Ok(()) }
///     fn submit_render(&self, _encoder: RenderEncoder, _wait: &[ImportedSemaphoreHandle]) -> Result<SubmitToken, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn copy_target_to_buffer(&self, _encoder: &mut CommandEncoder, _target: &RenderTargetHandle, _dst: &BufferHandle, _dst_offset: u64) -> Result<(), ContractError> { Ok(()) }
/// }
/// ```
///
/// `set_scissor_rect` だけを持たない実装は拒まれる。
///
/// ```compile_fail,E0046
/// # use fandhe_silicon_core::error::ContractError;
/// # use fandhe_silicon_core::execution::{BindingLayout, CommandEncoder, ConstantBinding, ImportedSemaphoreHandle, PipelineHandle, PipelineInput, SubmitToken};
/// # use fandhe_silicon_core::graphics::{DepthConfig, DepthFormat, GraphicsOps, IndexFormat, PixelFormat, RenderEncoder, ScissorRect, Viewport};
/// # use fandhe_silicon_core::memory::BufferHandle;
/// # use fandhe_silicon_core::render_target::RenderTargetHandle;
/// struct G;
/// impl GraphicsOps for G {
///     fn create_render_pipeline(&self, _vs: PipelineInput<'_>, _fs: PipelineInput<'_>, _layout: &BindingLayout) -> Result<PipelineHandle, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn create_render_pipeline_with_depth(&self, _vs: PipelineInput<'_>, _fs: PipelineInput<'_>, _layout: &BindingLayout, _depth_format: DepthFormat) -> Result<PipelineHandle, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn create_offscreen_target(&self, _width: u32, _height: u32, _format: PixelFormat) -> Result<RenderTargetHandle, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn create_offscreen_target_with_depth(&self, _width: u32, _height: u32, _color_format: PixelFormat, _depth: DepthConfig) -> Result<(RenderTargetHandle, RenderTargetHandle), ContractError> { Err(ContractError::DeviceMismatch) }
///     fn destroy_offscreen_target(&self, _target: RenderTargetHandle) {}
///     fn create_render_encoder(&self, _target: &RenderTargetHandle) -> RenderEncoder { loop {} }
///     fn create_render_encoder_with_depth(&self, _target: &RenderTargetHandle, _depth_target: &RenderTargetHandle, _clear_color: [f32; 4]) -> RenderEncoder { loop {} }
///     fn draw(&self, _encoder: &mut RenderEncoder, _pipeline: &PipelineHandle, _vertex_buffer: &BufferHandle, _vertex_count: u32) -> Result<(), ContractError> { Ok(()) }
///     fn draw_ex(&self, _encoder: &mut RenderEncoder, _pipeline: &PipelineHandle, _vertex_buffer: &BufferHandle, _vertex_count: u32, _vertex_constants: &[ConstantBinding<'_>]) -> Result<(), ContractError> { Ok(()) }
///     fn set_pipeline(&self, _encoder: &mut RenderEncoder, _pipeline: &PipelineHandle) -> Result<(), ContractError> { Ok(()) }
///     fn set_vertex_buffer(&self, _encoder: &mut RenderEncoder, _slot: u32, _buffer: &BufferHandle, _offset: u64, _size: Option<u64>) -> Result<(), ContractError> { Ok(()) }
///     fn set_index_buffer(&self, _encoder: &mut RenderEncoder, _buffer: &BufferHandle, _format: IndexFormat, _offset: u64, _size: Option<u64>) -> Result<(), ContractError> { Ok(()) }
///     fn set_immediates(&self, _encoder: &mut RenderEncoder, _offset: u32, _data: &[u8]) -> Result<(), ContractError> { Ok(()) }
///     fn set_viewport(&self, _encoder: &mut RenderEncoder, _viewport: Viewport) -> Result<(), ContractError> { Ok(()) }
///     fn set_blend_constant(&self, _encoder: &mut RenderEncoder, _color: [f32; 4]) -> Result<(), ContractError> { Ok(()) }
///     fn set_stencil_reference(&self, _encoder: &mut RenderEncoder, _reference: u32) -> Result<(), ContractError> { Ok(()) }
///     fn submit_render(&self, _encoder: RenderEncoder, _wait: &[ImportedSemaphoreHandle]) -> Result<SubmitToken, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn copy_target_to_buffer(&self, _encoder: &mut CommandEncoder, _target: &RenderTargetHandle, _dst: &BufferHandle, _dst_offset: u64) -> Result<(), ContractError> { Ok(()) }
/// }
/// ```
///
/// `set_blend_constant` だけを持たない実装は拒まれる。
///
/// ```compile_fail,E0046
/// # use fandhe_silicon_core::error::ContractError;
/// # use fandhe_silicon_core::execution::{BindingLayout, CommandEncoder, ConstantBinding, ImportedSemaphoreHandle, PipelineHandle, PipelineInput, SubmitToken};
/// # use fandhe_silicon_core::graphics::{DepthConfig, DepthFormat, GraphicsOps, IndexFormat, PixelFormat, RenderEncoder, ScissorRect, Viewport};
/// # use fandhe_silicon_core::memory::BufferHandle;
/// # use fandhe_silicon_core::render_target::RenderTargetHandle;
/// struct G;
/// impl GraphicsOps for G {
///     fn create_render_pipeline(&self, _vs: PipelineInput<'_>, _fs: PipelineInput<'_>, _layout: &BindingLayout) -> Result<PipelineHandle, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn create_render_pipeline_with_depth(&self, _vs: PipelineInput<'_>, _fs: PipelineInput<'_>, _layout: &BindingLayout, _depth_format: DepthFormat) -> Result<PipelineHandle, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn create_offscreen_target(&self, _width: u32, _height: u32, _format: PixelFormat) -> Result<RenderTargetHandle, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn create_offscreen_target_with_depth(&self, _width: u32, _height: u32, _color_format: PixelFormat, _depth: DepthConfig) -> Result<(RenderTargetHandle, RenderTargetHandle), ContractError> { Err(ContractError::DeviceMismatch) }
///     fn destroy_offscreen_target(&self, _target: RenderTargetHandle) {}
///     fn create_render_encoder(&self, _target: &RenderTargetHandle) -> RenderEncoder { loop {} }
///     fn create_render_encoder_with_depth(&self, _target: &RenderTargetHandle, _depth_target: &RenderTargetHandle, _clear_color: [f32; 4]) -> RenderEncoder { loop {} }
///     fn draw(&self, _encoder: &mut RenderEncoder, _pipeline: &PipelineHandle, _vertex_buffer: &BufferHandle, _vertex_count: u32) -> Result<(), ContractError> { Ok(()) }
///     fn draw_ex(&self, _encoder: &mut RenderEncoder, _pipeline: &PipelineHandle, _vertex_buffer: &BufferHandle, _vertex_count: u32, _vertex_constants: &[ConstantBinding<'_>]) -> Result<(), ContractError> { Ok(()) }
///     fn set_pipeline(&self, _encoder: &mut RenderEncoder, _pipeline: &PipelineHandle) -> Result<(), ContractError> { Ok(()) }
///     fn set_vertex_buffer(&self, _encoder: &mut RenderEncoder, _slot: u32, _buffer: &BufferHandle, _offset: u64, _size: Option<u64>) -> Result<(), ContractError> { Ok(()) }
///     fn set_index_buffer(&self, _encoder: &mut RenderEncoder, _buffer: &BufferHandle, _format: IndexFormat, _offset: u64, _size: Option<u64>) -> Result<(), ContractError> { Ok(()) }
///     fn set_immediates(&self, _encoder: &mut RenderEncoder, _offset: u32, _data: &[u8]) -> Result<(), ContractError> { Ok(()) }
///     fn set_viewport(&self, _encoder: &mut RenderEncoder, _viewport: Viewport) -> Result<(), ContractError> { Ok(()) }
///     fn set_scissor_rect(&self, _encoder: &mut RenderEncoder, _rect: ScissorRect) -> Result<(), ContractError> { Ok(()) }
///     fn set_stencil_reference(&self, _encoder: &mut RenderEncoder, _reference: u32) -> Result<(), ContractError> { Ok(()) }
///     fn submit_render(&self, _encoder: RenderEncoder, _wait: &[ImportedSemaphoreHandle]) -> Result<SubmitToken, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn copy_target_to_buffer(&self, _encoder: &mut CommandEncoder, _target: &RenderTargetHandle, _dst: &BufferHandle, _dst_offset: u64) -> Result<(), ContractError> { Ok(()) }
/// }
/// ```
///
/// `set_stencil_reference` だけを持たない実装は拒まれる。
///
/// ```compile_fail,E0046
/// # use fandhe_silicon_core::error::ContractError;
/// # use fandhe_silicon_core::execution::{BindingLayout, CommandEncoder, ConstantBinding, ImportedSemaphoreHandle, PipelineHandle, PipelineInput, SubmitToken};
/// # use fandhe_silicon_core::graphics::{DepthConfig, DepthFormat, GraphicsOps, IndexFormat, PixelFormat, RenderEncoder, ScissorRect, Viewport};
/// # use fandhe_silicon_core::memory::BufferHandle;
/// # use fandhe_silicon_core::render_target::RenderTargetHandle;
/// struct G;
/// impl GraphicsOps for G {
///     fn create_render_pipeline(&self, _vs: PipelineInput<'_>, _fs: PipelineInput<'_>, _layout: &BindingLayout) -> Result<PipelineHandle, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn create_render_pipeline_with_depth(&self, _vs: PipelineInput<'_>, _fs: PipelineInput<'_>, _layout: &BindingLayout, _depth_format: DepthFormat) -> Result<PipelineHandle, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn create_offscreen_target(&self, _width: u32, _height: u32, _format: PixelFormat) -> Result<RenderTargetHandle, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn create_offscreen_target_with_depth(&self, _width: u32, _height: u32, _color_format: PixelFormat, _depth: DepthConfig) -> Result<(RenderTargetHandle, RenderTargetHandle), ContractError> { Err(ContractError::DeviceMismatch) }
///     fn destroy_offscreen_target(&self, _target: RenderTargetHandle) {}
///     fn create_render_encoder(&self, _target: &RenderTargetHandle) -> RenderEncoder { loop {} }
///     fn create_render_encoder_with_depth(&self, _target: &RenderTargetHandle, _depth_target: &RenderTargetHandle, _clear_color: [f32; 4]) -> RenderEncoder { loop {} }
///     fn draw(&self, _encoder: &mut RenderEncoder, _pipeline: &PipelineHandle, _vertex_buffer: &BufferHandle, _vertex_count: u32) -> Result<(), ContractError> { Ok(()) }
///     fn draw_ex(&self, _encoder: &mut RenderEncoder, _pipeline: &PipelineHandle, _vertex_buffer: &BufferHandle, _vertex_count: u32, _vertex_constants: &[ConstantBinding<'_>]) -> Result<(), ContractError> { Ok(()) }
///     fn set_pipeline(&self, _encoder: &mut RenderEncoder, _pipeline: &PipelineHandle) -> Result<(), ContractError> { Ok(()) }
///     fn set_vertex_buffer(&self, _encoder: &mut RenderEncoder, _slot: u32, _buffer: &BufferHandle, _offset: u64, _size: Option<u64>) -> Result<(), ContractError> { Ok(()) }
///     fn set_index_buffer(&self, _encoder: &mut RenderEncoder, _buffer: &BufferHandle, _format: IndexFormat, _offset: u64, _size: Option<u64>) -> Result<(), ContractError> { Ok(()) }
///     fn set_immediates(&self, _encoder: &mut RenderEncoder, _offset: u32, _data: &[u8]) -> Result<(), ContractError> { Ok(()) }
///     fn set_viewport(&self, _encoder: &mut RenderEncoder, _viewport: Viewport) -> Result<(), ContractError> { Ok(()) }
///     fn set_scissor_rect(&self, _encoder: &mut RenderEncoder, _rect: ScissorRect) -> Result<(), ContractError> { Ok(()) }
///     fn set_blend_constant(&self, _encoder: &mut RenderEncoder, _color: [f32; 4]) -> Result<(), ContractError> { Ok(()) }
///     fn submit_render(&self, _encoder: RenderEncoder, _wait: &[ImportedSemaphoreHandle]) -> Result<SubmitToken, ContractError> { Err(ContractError::DeviceMismatch) }
///     fn copy_target_to_buffer(&self, _encoder: &mut CommandEncoder, _target: &RenderTargetHandle, _dst: &BufferHandle, _dst_offset: u64) -> Result<(), ContractError> { Ok(()) }
/// }
/// ```
pub trait GraphicsOps {
    /// 描画パイプラインを作る。
    fn create_render_pipeline(
        &self,
        vs: PipelineInput<'_>,
        fs: PipelineInput<'_>,
        layout: &BindingLayout,
    ) -> Result<PipelineHandle, ContractError>;

    /// 深度の形式を指定して描画パイプラインを作る。
    fn create_render_pipeline_with_depth(
        &self,
        vs: PipelineInput<'_>,
        fs: PipelineInput<'_>,
        layout: &BindingLayout,
        depth_format: DepthFormat,
    ) -> Result<PipelineHandle, ContractError>;

    /// オフスクリーンの描画先を作る。
    fn create_offscreen_target(
        &self,
        width: u32,
        height: u32,
        format: PixelFormat,
    ) -> Result<RenderTargetHandle, ContractError>;

    /// 色と深度の描画先の組を作る。
    fn create_offscreen_target_with_depth(
        &self,
        width: u32,
        height: u32,
        color_format: PixelFormat,
        depth: DepthConfig,
    ) -> Result<(RenderTargetHandle, RenderTargetHandle), ContractError>;

    /// 描画先を破棄する（TEX-13 のポインタ）。handle を値で受けて消費し、`Result` は返さない。
    /// `create_offscreen_target_with_depth` が返す色と深度の handle は、それぞれをこの操作で破棄する。
    ///
    /// ```
    /// use fandhe_silicon_core::graphics::GraphicsOps;
    /// use fandhe_silicon_core::render_target::RenderTargetHandle;
    /// fn f(g: &dyn GraphicsOps, t: RenderTargetHandle) {
    ///     g.destroy_offscreen_target(t);
    /// }
    /// ```
    ///
    /// 破棄の後に同じ handle を使う呼び出しはコンパイルで拒まれる。
    ///
    /// ```compile_fail,E0382
    /// use fandhe_silicon_core::graphics::GraphicsOps;
    /// use fandhe_silicon_core::render_target::RenderTargetHandle;
    /// fn f(g: &dyn GraphicsOps, t: RenderTargetHandle) {
    ///     g.destroy_offscreen_target(t);
    ///     let _ = g.create_render_encoder(&t);
    /// }
    /// ```
    fn destroy_offscreen_target(&self, target: RenderTargetHandle);

    /// 描画先に描く記録先を作る。
    fn create_render_encoder(&self, target: &RenderTargetHandle) -> RenderEncoder;

    /// 深度の描画先と消去の色を添えて記録先を作る。
    fn create_render_encoder_with_depth(
        &self,
        target: &RenderTargetHandle,
        depth_target: &RenderTargetHandle,
        clear_color: [f32; 4],
    ) -> RenderEncoder;

    /// 描く。
    fn draw(
        &self,
        encoder: &mut RenderEncoder,
        pipeline: &PipelineHandle,
        vertex_buffer: &BufferHandle,
        vertex_count: u32,
    ) -> Result<(), ContractError>;

    /// 頂点の定数を添えて描く。
    fn draw_ex(
        &self,
        encoder: &mut RenderEncoder,
        pipeline: &PipelineHandle,
        vertex_buffer: &BufferHandle,
        vertex_count: u32,
        vertex_constants: &[ConstantBinding<'_>],
    ) -> Result<(), ContractError>;

    /// パイプラインを記録先に設定する（REND-52）。
    fn set_pipeline(
        &self,
        encoder: &mut RenderEncoder,
        pipeline: &PipelineHandle,
    ) -> Result<(), ContractError>;

    /// 頂点バッファを番号の位置に設定する（REND-55）。
    fn set_vertex_buffer(
        &self,
        encoder: &mut RenderEncoder,
        slot: u32,
        buffer: &BufferHandle,
        offset: u64,
        size: Option<u64>,
    ) -> Result<(), ContractError>;

    /// インデックスバッファを設定する（REND-54）。
    fn set_index_buffer(
        &self,
        encoder: &mut RenderEncoder,
        buffer: &BufferHandle,
        format: IndexFormat,
        offset: u64,
        size: Option<u64>,
    ) -> Result<(), ContractError>;

    /// 即値のバイト列を設定する（REND-76）。
    fn set_immediates(
        &self,
        encoder: &mut RenderEncoder,
        offset: u32,
        data: &[u8],
    ) -> Result<(), ContractError>;

    /// ビューポートを設定する（REND-57）。
    ///
    /// ```
    /// use fandhe_silicon_core::graphics::{GraphicsOps, RenderEncoder, Viewport};
    /// fn f(g: &dyn GraphicsOps, e: &mut RenderEncoder) {
    ///     let v = Viewport { x: 0.0, y: 0.0, width: 1.0, height: 1.0, min_depth: 0.0, max_depth: 1.0 };
    ///     let _ = g.set_viewport(e, v);
    ///     let _ = g.set_stencil_reference(e, 0);
    /// }
    /// ```
    fn set_viewport(
        &self,
        encoder: &mut RenderEncoder,
        viewport: Viewport,
    ) -> Result<(), ContractError>;

    /// シザー矩形を設定する（REND-56）。
    fn set_scissor_rect(
        &self,
        encoder: &mut RenderEncoder,
        rect: ScissorRect,
    ) -> Result<(), ContractError>;

    /// ブレンド定数を設定する（REND-53）。
    fn set_blend_constant(
        &self,
        encoder: &mut RenderEncoder,
        color: [f32; 4],
    ) -> Result<(), ContractError>;

    /// ステンシル参照値を設定する（REND-58）。
    fn set_stencil_reference(
        &self,
        encoder: &mut RenderEncoder,
        reference: u32,
    ) -> Result<(), ContractError>;

    /// 記録先を値で受けて投入する。受けた記録先は再び使えない。
    ///
    /// ```compile_fail,E0382
    /// use fandhe_silicon_core::graphics::{GraphicsOps, RenderEncoder};
    /// fn f(g: &dyn GraphicsOps, e: RenderEncoder) {
    ///     let _ = g.submit_render(e, &[]);
    ///     let _ = g.submit_render(e, &[]);
    /// }
    /// ```
    fn submit_render(
        &self,
        encoder: RenderEncoder,
        wait: &[ImportedSemaphoreHandle],
    ) -> Result<SubmitToken, ContractError>;

    /// 描画先をバッファへ写す記録を足す。
    fn copy_target_to_buffer(
        &self,
        encoder: &mut CommandEncoder,
        target: &RenderTargetHandle,
        dst: &BufferHandle,
        dst_offset: u64,
    ) -> Result<(), ContractError>;
}

mod target_check;
pub use target_check::{
    depth_format_bytes, offscreen_target_bytes, pixel_format_bytes, validate_copy_target_to_buffer,
    validate_offscreen_target_size,
};

mod surface;
pub use surface::{CurrentSurfaceTexture, SurfaceStatus, validate_surface_size};

mod pass_state;
pub use pass_state::{IndexFormat, ScissorRect, Viewport, index_format_bytes};

mod draw_range;
pub use draw_range::{
    DrawRange, IndexedDrawRange, validate_draw_range, validate_indexed_draw_range,
};

/// `PixelFormat`・`DepthFormat` の値の数と、`TextureFormat` への変換で名前が保たれることを core の中で固定する試験。
/// 番号を返す `match` は `_` の腕を持たないので、値を足すとコンパイルが落ちて番号を足させる。
/// 一覧への足し忘れは、各番号がちょうど 1 回ずつ現れるかを数える試験が落とす。
#[cfg(test)]
mod tests {
    use super::*;

    const PIXEL_VARIANTS: usize = 3;
    const DEPTH_VARIANTS: usize = 1;

    fn pixel_ordinal(p: PixelFormat) -> usize {
        match p {
            PixelFormat::Rgba8Unorm => 0,
            PixelFormat::Bgra8Unorm => 1,
            PixelFormat::Rgba32Float => 2,
        }
    }

    fn depth_ordinal(d: DepthFormat) -> usize {
        match d {
            DepthFormat::Depth32Float => 0,
        }
    }

    const PIXELS: [PixelFormat; 3] = [
        PixelFormat::Rgba8Unorm,
        PixelFormat::Bgra8Unorm,
        PixelFormat::Rgba32Float,
    ];
    const DEPTHS: [DepthFormat; 1] = [DepthFormat::Depth32Float];

    #[test]
    fn pixel_format_has_three_values() {
        assert_eq!(PIXELS.len(), PIXEL_VARIANTS);
        let mut seen = [0usize; PIXEL_VARIANTS];
        for p in PIXELS {
            seen[pixel_ordinal(p)] += 1;
        }
        assert_eq!(seen, [1; PIXEL_VARIANTS]);
    }

    #[test]
    fn depth_format_has_one_value() {
        assert_eq!(DEPTHS.len(), DEPTH_VARIANTS);
        let mut seen = [0usize; DEPTH_VARIANTS];
        for d in DEPTHS {
            seen[depth_ordinal(d)] += 1;
        }
        assert_eq!(seen, [1; DEPTH_VARIANTS]);
    }

    #[test]
    fn conversions_keep_names() {
        for p in PIXELS {
            let t = TextureFormat::from(p);
            assert_eq!(format!("{p:?}"), format!("{t:?}"));
        }
        for d in DEPTHS {
            let t = TextureFormat::from(d);
            assert_eq!(format!("{d:?}"), format!("{t:?}"));
        }
    }
}
