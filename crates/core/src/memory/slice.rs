//! バッファの部分範囲の値の型と、それを作る・読む口の置き場。
//!
//! `memory` から再公開される（TASK-311 のポインタ）。範囲の検査は
//! `validate_buffer_range` に委ね、背後実装の API は呼ばない。

use super::{BufferHandle, validate_buffer_range};
use crate::error::ContractError;

/// 元の [`BufferHandle`] を共有で借りるバッファの部分範囲（MEM-41 のポインタ）。
///
/// 開始位置は元のバッファの先頭からのバイト数。欄は非公開で、
/// 範囲は [`BufferHandle::slice`]・[`BufferHandle::as_entire_slice`] からだけ作れる。
#[derive(Debug, Clone, Copy)]
pub struct BufferSlice<'a> {
    buffer: &'a BufferHandle,
    offset: u64,
    size: u64,
}

impl BufferHandle {
    /// 開始位置 `offset`・大きさ `size` の部分範囲を作る（MEM-41 のポインタ）。
    ///
    /// 検査は [`validate_buffer_range`] を使い、確保長ちょうどと長さ 0 は受理する。
    ///
    /// # Errors
    ///
    /// 終端 `offset + size` が `u64` であふれるか [`BufferHandle::size`] を超えるとき、
    /// [`ContractError::InvalidArgument`] を返す。
    ///
    /// # Examples
    ///
    /// ```
    /// use fandhe_silicon_core::error::ContractError;
    /// use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};
    ///
    /// # let h = BufferHandle::__backend_new(0, 1, 1, 16, BufferUsage::STORAGE);
    /// let s = h.slice(4, 8).unwrap();
    /// assert_eq!(s.offset(), 4);
    /// assert_eq!(s.size(), 8);
    /// assert!(matches!(h.slice(8, 9), Err(ContractError::InvalidArgument(_))));
    ///
    /// let e = h.as_entire_slice();
    /// assert_eq!((e.offset(), e.size()), (0, 16));
    /// ```
    pub fn slice(&self, offset: u64, size: u64) -> Result<BufferSlice<'_>, ContractError> {
        validate_buffer_range(offset, size, self.size())?;
        Ok(BufferSlice {
            buffer: self,
            offset,
            size,
        })
    }

    /// バッファ全体の範囲（開始位置 0・大きさ [`BufferHandle::size`]）を返す（MEM-38 のポインタ）。
    ///
    /// 失敗しない。
    pub fn as_entire_slice(&self) -> BufferSlice<'_> {
        BufferSlice {
            buffer: self,
            offset: 0,
            size: self.size(),
        }
    }
}

impl<'a> BufferSlice<'a> {
    /// 範囲を作った元のハンドルを返す（MEM-55 のポインタ）。
    pub fn buffer(&self) -> &'a BufferHandle {
        self.buffer
    }

    /// 元のバッファの先頭からの開始位置（バイト）を返す（MEM-56 のポインタ）。
    pub fn offset(&self) -> u64 {
        self.offset
    }

    /// 範囲の大きさ（バイト）を返す（MEM-57 のポインタ）。
    pub fn size(&self) -> u64 {
        self.size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::BufferUsage;

    fn handle(len: u64) -> BufferHandle {
        BufferHandle::__backend_new(0, 1, 1, len, BufferUsage::STORAGE)
    }

    fn is_invalid<T>(r: &Result<T, ContractError>) -> bool {
        matches!(r, Err(ContractError::InvalidArgument(_)))
    }

    #[test]
    fn accepts_up_to_allocation_length() {
        let h = handle(16);
        for (o, s) in [(0, 16), (8, 8), (16, 0), (3, 5)] {
            let sl = h.slice(o, s).expect("in range");
            assert_eq!((sl.offset(), sl.size()), (o, s));
        }
    }

    #[test]
    fn rejects_one_past_the_end() {
        let h = handle(16);
        for (o, s) in [(0, 17), (8, 9), (17, 0), (16, 1)] {
            assert!(is_invalid(&h.slice(o, s)), "({o}, {s})");
        }
    }

    #[test]
    fn zero_length_ranges() {
        let h = handle(16);
        for o in [0, 16] {
            assert_eq!(h.slice(o, 0).expect("ok").size(), 0);
        }
        assert!(is_invalid(&h.slice(17, 0)));
    }

    #[test]
    fn rejects_u64_overflow() {
        let h = handle(u64::MAX);
        assert!(is_invalid(&h.slice(u64::MAX, 1)));
        assert!(h.slice(u64::MAX, 0).is_ok());
    }

    #[test]
    fn buffer_is_the_source_handle() {
        let h = handle(16);
        let s = h.slice(2, 4).expect("ok");
        assert!(core::ptr::eq(s.buffer(), &h));
    }

    #[test]
    fn entire_slice_covers_whole_buffer() {
        for len in [0, 1, 16, u64::MAX] {
            let h = handle(len);
            let s = h.as_entire_slice();
            assert_eq!((s.offset(), s.size()), (0, h.size()));
            assert!(core::ptr::eq(s.buffer(), &h));
        }
    }
}
