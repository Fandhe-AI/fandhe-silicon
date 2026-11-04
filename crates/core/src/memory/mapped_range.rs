//! 書き込み用のビューの型と、その長さ・空の判定・範囲の絞りの置き場。
//!
//! `memory` から再公開される（TASK-312 のポインタ）。背後実装の API は呼ばない。
//! 範囲の検査は `validate_buffer_range`（TASK-309）に委ねる。

use core::fmt;
use core::mem::MaybeUninit;

use super::validate_buffer_range;
use crate::error::ContractError;

/// 書き込み用のビューの型。バイト列を排他で借り、長さと空の判定・範囲の絞りの口を持つ。
///
/// 欄は非公開。長さは [`MappedRangeMut::len`]、空かどうかは [`MappedRangeMut::is_empty`] で読み、
/// 範囲は [`MappedRangeMut::slice`] で絞る。絞ったビューが生きている間は、同じビューから
/// 次のビューを絞れない。
///
/// ```compile_fail,E0499
/// use fandhe_silicon_core::memory::MappedRangeMut;
/// fn f(view: &mut MappedRangeMut<'_>) {
///     let a = view.slice(0, 4).unwrap();
///     let b = view.slice(4, 4).unwrap();
///     let _ = (a.len(), b.len());
/// }
/// ```
///
/// 使い切ってから次を絞る形は通る。
///
/// ```
/// use fandhe_silicon_core::memory::MappedRangeMut;
/// fn f(view: &mut MappedRangeMut<'_>) {
///     let a = view.slice(0, 4).unwrap().len();
///     let b = view.slice(4, 4).unwrap().len();
///     let _ = (a, b);
/// }
/// ```
///
/// ```
/// # use core::mem::MaybeUninit;
/// # use fandhe_silicon_core::memory::MappedRangeMut;
/// # let mut bytes = [MaybeUninit::<u8>::uninit(); 16];
/// # let view = MappedRangeMut::__backend_new(&mut bytes);
/// assert_eq!(view.len(), 16);
/// assert!(!view.is_empty());
/// ```
pub struct MappedRangeMut<'a> {
    bytes: &'a mut [MaybeUninit<u8>],
}

impl<'a> MappedRangeMut<'a> {
    /// 背後実装がビューを作る。`bytes` を排他で借りる。
    #[doc(hidden)]
    pub fn __backend_new(bytes: &'a mut [MaybeUninit<u8>]) -> Self {
        Self { bytes }
    }

    /// ビューのバイト数。背後実装を呼ばず、失敗しない（MEM-59 のポインタ）。
    pub fn len(&self) -> u64 {
        self.bytes.len() as u64
    }

    /// 長さが 0 のとき真（MEM-60 のポインタ）。
    ///
    /// ```
    /// # use core::mem::MaybeUninit;
    /// # use fandhe_silicon_core::memory::MappedRangeMut;
    /// # let mut bytes: [MaybeUninit<u8>; 0] = [];
    /// # let view = MappedRangeMut::__backend_new(&mut bytes);
    /// assert!(view.is_empty());
    /// assert_eq!(view.len(), 0);
    /// ```
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// このビューの先頭からの相対で `offset` から `len` バイトを絞ったビューを返す。
    /// 元のビューを排他で借りる（MEM-61 のポインタ）。長さちょうど・長さ 0 も受理する。
    ///
    /// ```
    /// # use core::mem::MaybeUninit;
    /// # use fandhe_silicon_core::error::ContractError;
    /// # use fandhe_silicon_core::memory::MappedRangeMut;
    /// # let mut bytes = [MaybeUninit::<u8>::uninit(); 16];
    /// # let mut view = MappedRangeMut::__backend_new(&mut bytes);
    /// assert_eq!(view.slice(4, 8).unwrap().len(), 8);
    /// assert!(view.slice(16, 0).unwrap().is_empty());
    /// assert!(matches!(view.slice(8, 9), Err(ContractError::InvalidArgument(_))));
    /// ```
    ///
    /// # Errors
    ///
    /// 終端 `offset + len` が `u64` であふれるか、このビューの長さを超えるとき
    /// `ContractError::InvalidArgument`（`validate_buffer_range` の値のまま）。
    pub fn slice(&mut self, offset: u64, len: u64) -> Result<MappedRangeMut<'_>, ContractError> {
        validate_buffer_range(offset, len, self.len())?;
        let narrowed = usize::try_from(offset).ok().and_then(|start| {
            let n = usize::try_from(len).ok()?;
            let end = start.checked_add(n)?;
            self.bytes.get_mut(start..end)
        });
        match narrowed {
            Some(bytes) => Ok(MappedRangeMut { bytes }),
            None => Err(ContractError::InvalidArgument(format!(
                "mapped range (offset {offset}, len {len}) does not fit in usize"
            ))),
        }
    }
}

impl fmt::Debug for MappedRangeMut<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MappedRangeMut")
            .field("len", &self.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_length_array_is_empty() {
        let mut bytes: [MaybeUninit<u8>; 0] = [];
        let view = MappedRangeMut::__backend_new(&mut bytes);
        assert_eq!(view.len(), 0);
        assert!(view.is_empty());
        assert_eq!(format!("{view:?}"), "MappedRangeMut { len: 0 }");
    }

    #[test]
    fn zero_length_vec_is_empty() {
        let mut bytes: Vec<MaybeUninit<u8>> = Vec::new();
        let view = MappedRangeMut::__backend_new(&mut bytes);
        assert_eq!(view.len(), 0);
        assert!(view.is_empty());
    }

    #[test]
    fn len_follows_backing_length() {
        for n in [1usize, 16, 4096] {
            let mut bytes = vec![MaybeUninit::<u8>::uninit(); n];
            let view = MappedRangeMut::__backend_new(&mut bytes);
            assert_eq!(view.len(), n as u64);
            assert!(!view.is_empty());
            assert_eq!(
                format!("{view:?}"),
                format!("MappedRangeMut {{ len: {n} }}")
            );
        }
    }

    fn addrs(v: &MappedRangeMut<'_>) -> (usize, usize) {
        let r = v.bytes.as_ptr_range();
        (r.start.addr(), r.end.addr())
    }

    #[test]
    fn slice_boundaries() {
        let n = 16u64;
        let mut bytes = vec![MaybeUninit::<u8>::uninit(); n as usize];
        let mut view = MappedRangeMut::__backend_new(&mut bytes);
        assert_eq!(view.slice(0, n).unwrap().len(), n);
        assert!(view.slice(n, 0).unwrap().is_empty());
        for (o, l) in [(0, n + 1), (n, 1), (n + 1, 0), (u64::MAX, 1), (1, u64::MAX)] {
            let got = view.slice(o, l).map(|v| v.len());
            assert!(matches!(got, Err(ContractError::InvalidArgument(_))));
            assert_eq!(got.map(|_| ()), validate_buffer_range(o, l, n));
        }
        assert_eq!(view.len(), n);
        assert!(!view.is_empty());
    }

    #[test]
    fn slice_of_empty_view() {
        let mut bytes: [MaybeUninit<u8>; 0] = [];
        let mut view = MappedRangeMut::__backend_new(&mut bytes);
        assert!(view.slice(0, 0).unwrap().is_empty());
        assert!(matches!(
            view.slice(0, 1).map(|v| v.len()),
            Err(ContractError::InvalidArgument(_))
        ));
    }

    #[test]
    fn slice_stays_inside_parent() {
        let mut bytes = vec![MaybeUninit::<u8>::uninit(); 32];
        let mut view = MappedRangeMut::__backend_new(&mut bytes);
        let (ps, pe) = addrs(&view);
        let (cs, ce) = {
            let mut child = view.slice(4, 20).unwrap();
            let (cs, ce) = addrs(&child);
            let (gs, ge) = {
                let grand = child.slice(3, 10).unwrap();
                addrs(&grand)
            };
            assert_eq!((gs, ge), (cs + 3, cs + 13));
            (cs, ce)
        };
        assert_eq!((cs, ce), (ps + 4, ps + 24));
        assert!(ce <= pe);
        assert_eq!(view.len(), 32);
    }
}
