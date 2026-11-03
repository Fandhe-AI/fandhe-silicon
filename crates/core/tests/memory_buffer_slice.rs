//! `BufferSlice` を作る口の境界の入力を、外の crate から公開のパスで確かめる統合テスト
//! （TASK-311 の 3/5）。
//!
//! 確保長ちょうど・1 超過・`u64` のあふれ・長さ 0 の扱いと、範囲の検査が
//! `validate_buffer_range`（TASK-309）と同じ結果になることを確かめる。
//! 拒否の文言の字句は固定せず、`==` と `matches!` だけで比べる。

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage, validate_buffer_range};

fn handle(len: u64) -> BufferHandle {
    BufferHandle::__backend_new(0, 1, 1, len, BufferUsage::STORAGE)
}

fn is_invalid<T>(r: &Result<T, ContractError>) -> bool {
    matches!(r, Err(ContractError::InvalidArgument(_)))
}

#[test]
fn slice_end_at_allocation_length_is_accepted() {
    // MEM-41
    let h = handle(16);
    for (o, s) in [(0u64, 16u64), (8, 8), (16, 0)] {
        let sl = h.slice(o, s).expect("accepted");
        assert_eq!((sl.offset(), sl.size()), (o, s));
    }
}

#[test]
fn slice_past_allocation_length_is_rejected() {
    // MEM-41
    let h = handle(16);
    for (o, s) in [(0u64, 17u64), (8, 9), (17, 0)] {
        assert!(is_invalid(&h.slice(o, s)), "({o}, {s})");
    }
}

#[test]
fn slice_end_overflow_is_rejected() {
    // MEM-41
    let h = handle(u64::MAX);
    assert!(is_invalid(&h.slice(u64::MAX, 1)));
    assert!(is_invalid(&h.slice(1, u64::MAX)));
    assert!(h.slice(u64::MAX, 0).is_ok());
    assert!(h.slice(0, u64::MAX).is_ok());
}

#[test]
fn zero_length_slices() {
    // MEM-57
    let h = handle(16);
    for o in [0u64, 16] {
        assert_eq!(h.slice(o, 0).expect("accepted").size(), 0);
    }
    let empty = handle(0);
    assert_eq!(empty.slice(0, 0).expect("accepted").size(), 0);
    assert!(is_invalid(&empty.slice(1, 0)));
}

#[test]
fn slice_matches_validate_buffer_range() {
    // TASK-309 の関数と同じ結果になること。
    for len in [0u64, 1, 16, u64::MAX] {
        let h = handle(len);
        for o in [0u64, 1, 15, 16, 17, u64::MAX - 1, u64::MAX] {
            for s in [0u64, 1, 15, 16, 17, u64::MAX] {
                let got = h.slice(o, s).map(|x| (x.offset(), x.size()));
                let want = validate_buffer_range(o, s, h.size()).map(|()| (o, s));
                assert_eq!(got, want, "(len, o, s) = ({len}, {o}, {s})");
            }
        }
    }
}

#[test]
fn slice_accessors_return_inputs() {
    // MEM-55 / MEM-56 / MEM-57
    let h = handle(16);
    let sl = h.slice(4, 8).expect("accepted");
    assert!(core::ptr::eq(sl.buffer(), &h));
    assert_eq!(sl.offset(), 4);
    assert_eq!(sl.size(), 8);
}

#[test]
fn slice_rejection_is_invalid_argument_only() {
    let h = handle(16);
    for (o, s) in [(0u64, 17u64), (8, 9), (17, 0)] {
        assert!(
            matches!(h.slice(o, s), Err(ContractError::InvalidArgument(_))),
            "({o}, {s})"
        );
    }
    let max = handle(u64::MAX);
    for (o, s) in [(u64::MAX, 1u64), (1, u64::MAX)] {
        assert!(
            matches!(max.slice(o, s), Err(ContractError::InvalidArgument(_))),
            "({o}, {s})"
        );
    }
    assert!(matches!(
        handle(0).slice(1, 0),
        Err(ContractError::InvalidArgument(_))
    ));
}
