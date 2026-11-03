//! `BufferSlice` を作る口の境界の入力を、外の crate から公開のパスで確かめる統合テスト
//! （TASK-311 の 3/5）。
//!
//! 確保長ちょうど・1 超過・`u64` のあふれ・長さ 0 の扱いと、範囲の検査が
//! `validate_buffer_range`（TASK-309）と同じ結果になることを確かめる。
//! 拒否の文言の字句は固定せず、`==` と `matches!` だけで比べる。

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::memory::{BufferHandle, BufferSlice, BufferUsage, validate_buffer_range};

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
fn slice_buffer_returns_origin_handle() {
    // MEM-55
    let h = handle(16);
    let sl = h.slice(4, 8).expect("accepted");
    assert!(core::ptr::eq(sl.buffer(), &h));
}

#[test]
fn slice_offset_returns_input() {
    // MEM-56
    let h = handle(16);
    let sl = h.slice(4, 8).expect("accepted");
    assert_eq!(sl.offset(), 4);
}

#[test]
fn slice_size_returns_input() {
    // MEM-57
    let h = handle(16);
    let sl = h.slice(4, 8).expect("accepted");
    assert_eq!(sl.size(), 8);
}

#[test]
fn slice_debug_output_is_non_empty() {
    // 出力の形は固定せず、`Debug` を持ち空でない文字列になることだけを確かめる。
    let h = handle(16);
    let sl = h.slice(4, 8).expect("accepted");
    assert!(!format!("{sl:?}").is_empty());
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

fn pair(r: Result<BufferSlice<'_>, ContractError>) -> Result<(u64, u64), ContractError> {
    r.map(|x| (x.offset(), x.size()))
}

const OFFSETS: [u64; 7] = [0, 1, 15, 16, 17, u64::MAX - 1, u64::MAX];
const SIZES: [u64; 6] = [0, 1, 15, 16, 17, u64::MAX];

#[test]
fn nested_slice_is_relative_to_parent() {
    // MEM-54
    let h = handle(16);
    let p = h.slice(4, 8).expect("accepted");
    let c = p.slice(2, 2).expect("accepted");
    assert_eq!((c.offset(), c.size()), (6, 2));
    assert!(core::ptr::eq(c.buffer(), &h));
}

#[test]
fn nested_slice_is_checked_against_parent_size() {
    // MEM-54
    let h = handle(16);
    let p = h.slice(4, 8).expect("accepted");
    assert!(is_invalid(&p.slice(2, 7)));
    let end = p.slice(8, 0).expect("accepted");
    assert_eq!((end.offset(), end.size()), (12, 0));
    assert!(is_invalid(&p.slice(9, 0)));
    assert!(is_invalid(&p.slice(u64::MAX, 1)));
}

#[test]
fn nested_slice_matches_validate_buffer_range() {
    // TASK-309 の関数と同じ結果になること（親の大きさで検査し、開始位置は親の分を足す）。
    let h = handle(16);
    for (po, ps) in [(0u64, 0u64), (0, 1), (4, 8)] {
        let p = h.slice(po, ps).expect("accepted");
        for o in OFFSETS {
            for s in SIZES {
                let got = pair(p.slice(o, s));
                let want = validate_buffer_range(o, s, p.size()).map(|()| (po + o, s));
                assert_eq!(got, want, "(parent, o, s) = (({po}, {ps}), {o}, {s})");
            }
        }
    }
}

#[test]
fn multi_level_nesting_offsets_accumulate() {
    // MEM-56
    let h = handle(32);
    let a = h.slice(3, 24).expect("accepted");
    let b = a.slice(5, 16).expect("accepted");
    let c = b.slice(1, 10).expect("accepted");
    let d = c.slice(4, 6).expect("accepted");
    for (sl, o, s) in [(a, 3, 24), (b, 8, 16), (c, 9, 10), (d, 13, 6)] {
        assert_eq!((sl.offset(), sl.size()), (o, s));
        assert!(core::ptr::eq(sl.buffer(), &h));
    }
}

#[test]
fn entire_slice_covers_whole_buffer() {
    // MEM-38
    for len in [0u64, 1, 16, u64::MAX] {
        let h = handle(len);
        let e = h.as_entire_slice();
        assert_eq!((e.offset(), e.size()), (0, h.size()));
        assert!(core::ptr::eq(e.buffer(), &h));
        for o in OFFSETS {
            for s in SIZES {
                assert_eq!(
                    pair(e.slice(o, s)),
                    pair(h.slice(o, s)),
                    "(len, o, s) = ({len}, {o}, {s})"
                );
            }
        }
    }
}

#[test]
fn slices_are_copy_and_coexist() {
    let h = handle(16);
    let a = h.slice(0, 8).expect("accepted");
    let b = h.slice(8, 8).expect("accepted");
    let a2 = a;
    let b2 = b;
    let n = a.slice(2, 4).expect("accepted");
    for (sl, o, s) in [(a, 0, 8), (b, 8, 8), (a2, 0, 8), (b2, 8, 8), (n, 2, 4)] {
        assert_eq!((sl.offset(), sl.size()), (o, s));
        assert!(core::ptr::eq(sl.buffer(), &h));
    }
}

#[test]
fn slice_is_repeatable() {
    let h = handle(16);
    let p = h.slice(4, 8).expect("accepted");
    for _ in 0..2 {
        assert_eq!(pair(h.slice(4, 8)), Ok((4, 8)));
        assert_eq!(pair(p.slice(2, 2)), Ok((6, 2)));
        assert_eq!(pair(h.as_entire_slice().slice(0, 16)), Ok((0, 16)));
        assert!(is_invalid(&h.slice(8, 9)));
        assert!(is_invalid(&p.slice(2, 7)));
    }
}
