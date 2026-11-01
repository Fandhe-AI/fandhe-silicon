//! 描画先の handle の型を置く module（TASK-375、TEX-12 のポインタ）。
//!
//! 背後実装が作り、上の層が受け渡す不透明な値の型と、handle が自 device のものかを照らす純粋関数
//! [`check_render_target_device`] を持つ。feature の条件なしで公開される。

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

/// handle が自 device のものかを照らす純粋関数（TEX-12、TASK-375 のポインタ）。
///
/// 入力は照らす handle・自 device の識別子・呼んだ時点の世代・ロストの有無。
/// 次の順で判定する。
///
/// 1. `lost` が真なら `Err(ContractError::DeviceLost { generation })`（渡した `generation` をそのまま載せる）
/// 2. handle の device の識別子が `device_id` と違えば `Err(ContractError::DeviceMismatch)`
/// 3. それ以外は `Ok(())`
///
/// handle の世代・識別子は見ない。状態を持たず、何も読み書きしない。
///
/// ```
/// use fandhe_silicon_core::error::ContractError;
/// use fandhe_silicon_core::render_target::{RenderTargetHandle, check_render_target_device};
///
/// // 同じ識別子でロストなし
/// let h = RenderTargetHandle::__backend_new(5, 3, 1);
/// assert_eq!(check_render_target_device(&h, 1, 9, false), Ok(()));
///
/// // 違う識別子
/// assert_eq!(
///     check_render_target_device(&h, 2, 9, false),
///     Err(ContractError::DeviceMismatch)
/// );
///
/// // ロストは渡した世代を載せる
/// assert_eq!(
///     check_render_target_device(&h, 1, 9, true),
///     Err(ContractError::DeviceLost { generation: 9 })
/// );
///
/// // ロストが識別子の違いより先
/// assert_eq!(
///     check_render_target_device(&h, 2, 9, true),
///     Err(ContractError::DeviceLost { generation: 9 })
/// );
///
/// // handle の世代・識別子は見ない
/// let z = RenderTargetHandle::__backend_new(0, 0, 1);
/// let m = RenderTargetHandle::__backend_new(u64::MAX, u64::MAX, 1);
/// assert_eq!(check_render_target_device(&z, 1, 9, false), Ok(()));
/// assert_eq!(check_render_target_device(&m, 1, 9, false), Ok(()));
/// ```
pub fn check_render_target_device(
    handle: &RenderTargetHandle,
    device_id: u64,
    generation: u64,
    lost: bool,
) -> Result<(), crate::error::ContractError> {
    if lost {
        return Err(crate::error::ContractError::DeviceLost { generation });
    }
    if handle.__backend_device_id() != device_id {
        return Err(crate::error::ContractError::DeviceMismatch);
    }
    Ok(())
}
