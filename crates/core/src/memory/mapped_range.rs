//! 書き込み用のビューの型と、その長さ・空の判定の置き場。
//!
//! `memory` から再公開される（TASK-312 のポインタ）。背後実装の API は呼ばない。

use core::fmt;
use core::mem::MaybeUninit;

/// 書き込み用のビューの型。バイト列を排他で借り、長さと空の判定の口を持つ。
///
/// 欄は非公開。長さは [`MappedRangeMut::len`]、空かどうかは [`MappedRangeMut::is_empty`] で読む。
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
}
