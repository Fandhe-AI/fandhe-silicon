//! 描画の公開の型と、描画の操作の trait [`GraphicsOps`] を置く module（TASK-837、DEV-63 のポインタ）。
//!
//! feature `graphics` の裏にあり、feature の無い構成では module ごと無い。

use crate::error::ContractError;
use crate::execution::{
    BindingLayout, CommandEncoder, ConstantBinding, ImportedSemaphoreHandle, PipelineHandle,
    PipelineInput, SubmitToken,
};
use crate::memory::BufferHandle;
use crate::render_target::RenderTargetHandle;

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

/// 描画の操作の trait。背後実装が実装する（TASK-837 のポインタ）。
///
/// `&dyn GraphicsOps` として書ける。
///
/// ```
/// use fandhe_silicon_core::graphics::{GraphicsOps, RenderEncoder};
/// fn f(g: &dyn GraphicsOps, e: RenderEncoder) {
///     let _ = g.submit_render(e, &[]);
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
