//! 描画先の handle の型を置く module（TASK-375、TEX-12 のポインタ）。
//!
//! 背後実装が作り、上の層が受け渡す不透明な値の型だけを持つ。feature の条件なしで公開される。

/// 描画先を指す不透明な handle（TEX-12、TASK-375 のポインタ）。
///
/// 欄（世代・識別子・device の識別子）は非公開で、`Clone`・`Copy` を実装しない。
/// `__backend_*` は背後実装だけが使う口。`__backend_new` は safe で、誰でも値を作れる。
///
/// ```
/// use fandhe_silicon_core::render_target::RenderTargetHandle;
/// let h = RenderTargetHandle::__backend_new(5, 3, 1);
/// assert_eq!(h.__backend_generation(), 5);
/// assert_eq!(h.__backend_id(), 3);
/// assert_eq!(h.__backend_device_id(), 1);
/// ```
///
/// 構造体式は拒まれる。
///
/// ```compile_fail,E0451
/// use fandhe_silicon_core::render_target::RenderTargetHandle;
/// let _ = RenderTargetHandle { generation: 5, id: 3, device_id: 1 };
/// ```
///
/// `clone` は呼べない。
///
/// ```compile_fail,E0599
/// use fandhe_silicon_core::render_target::RenderTargetHandle;
/// let h = RenderTargetHandle::__backend_new(5, 3, 1);
/// let _ = h.clone();
/// ```
#[derive(Debug)]
pub struct RenderTargetHandle {
    generation: u64,
    id: u64,
    device_id: u64,
}

impl RenderTargetHandle {
    /// 背後実装向けの組み立ての口。検査はしない。
    #[doc(hidden)]
    pub fn __backend_new(generation: u64, id: u64, device_id: u64) -> Self {
        Self {
            generation,
            id,
            device_id,
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
}
