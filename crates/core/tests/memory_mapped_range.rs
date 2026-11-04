//! `MappedRangeMut` の境界の入力と範囲の絞りを、外の crate から公開のパスで確かめる統合テスト
//! （TASK-312 の 3/4）。
//!
//! 長さ 0・長さちょうど・長さを超える・`u64` のあふれの扱いと、絞りが
//! `validate_buffer_range`（TASK-309）と同じ結果になることを確かめる。
//! 拒否の文言の字句は固定せず、`==` と `matches!` だけで比べる。

use core::mem::MaybeUninit;

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::memory::{MappedRangeMut, validate_buffer_range};

fn backing(n: usize) -> Vec<MaybeUninit<u8>> {
    vec![MaybeUninit::<u8>::uninit(); n]
}

fn is_invalid<T>(r: &Result<T, ContractError>) -> bool {
    matches!(r, Err(ContractError::InvalidArgument(_)))
}

/// 絞りの結果を長さだけにして返す（子のビューをその場で使い切る）。
fn sliced_len(view: &mut MappedRangeMut<'_>, o: u64, l: u64) -> Result<u64, ContractError> {
    view.slice(o, l).map(|v| v.len())
}

// MEM-60
#[test]
fn empty_view_len_and_is_empty() {
    let mut b: [MaybeUninit<u8>; 0] = [];
    let mut v = Vec::<MaybeUninit<u8>>::new();
    for mut view in [
        MappedRangeMut::__backend_new(&mut b),
        MappedRangeMut::__backend_new(&mut v),
    ] {
        assert_eq!(view.len(), 0);
        assert!(view.is_empty());
        assert_eq!(sliced_len(&mut view, 0, 0), Ok(0));
        assert!(is_invalid(&sliced_len(&mut view, 0, 1)));
        assert!(is_invalid(&sliced_len(&mut view, 1, 0)));
    }
}

// MEM-59
#[test]
fn len_and_is_empty_follow_backing_length() {
    for n in [1usize, 3, 4, 8, 255, 4096] {
        let mut bytes = backing(n);
        let view = MappedRangeMut::__backend_new(&mut bytes);
        assert_eq!(view.len(), n as u64);
        assert!(!view.is_empty());
    }
}

// MEM-61
#[test]
fn slice_up_to_exact_length_is_accepted() {
    for n in [1u64, 7, 16] {
        let mut bytes = backing(n as usize);
        let mut view = MappedRangeMut::__backend_new(&mut bytes);
        assert_eq!(sliced_len(&mut view, 0, n), Ok(n));
        for k in 0..=n {
            assert_eq!(sliced_len(&mut view, k, n - k), Ok(n - k));
        }
        let empty = view.slice(n, 0).unwrap();
        assert_eq!(empty.len(), 0);
        assert!(empty.is_empty());
    }
}

// MEM-61
#[test]
fn slice_beyond_length_is_rejected() {
    for n in [1u64, 7, 16] {
        let mut bytes = backing(n as usize);
        let mut view = MappedRangeMut::__backend_new(&mut bytes);
        assert!(is_invalid(&sliced_len(&mut view, 0, n + 1)));
        assert!(is_invalid(&sliced_len(&mut view, n, 1)));
        assert!(is_invalid(&sliced_len(&mut view, n + 1, 0)));
        assert!(is_invalid(&sliced_len(&mut view, 1, n)));
    }
}

// MEM-61
#[test]
fn slice_overflow_is_rejected() {
    let mut bytes = backing(16);
    let mut view = MappedRangeMut::__backend_new(&mut bytes);
    assert!(is_invalid(&sliced_len(&mut view, u64::MAX, 1)));
    assert!(is_invalid(&sliced_len(&mut view, 1, u64::MAX)));
    assert!(is_invalid(&sliced_len(&mut view, u64::MAX, u64::MAX)));
    assert!(is_invalid(&sliced_len(&mut view, u64::MAX, 0)));
}

// MEM-61
#[test]
fn slice_matches_validate_buffer_range() {
    for n in [0u64, 1, 7, 16] {
        let mut bytes = backing(n as usize);
        let mut view = MappedRangeMut::__backend_new(&mut bytes);
        let mut grid: Vec<u64> = (0..=n + 2).collect();
        grid.extend([u64::MAX - 1, u64::MAX]);
        for &o in &grid {
            for &l in &grid {
                let got = sliced_len(&mut view, o, l);
                let want = validate_buffer_range(o, l, n).map(|()| l);
                assert_eq!(got, want, "n={n} offset={o} len={l}");
            }
        }
    }
}

// MEM-61
#[test]
fn nested_slice_is_checked_against_parent() {
    let mut bytes = backing(16);
    let mut view = MappedRangeMut::__backend_new(&mut bytes);
    {
        let mut child = view.slice(4, 8).unwrap();
        assert_eq!(child.len(), 8);
        assert_eq!(sliced_len(&mut child, 0, 8), Ok(8));
        assert!(is_invalid(&sliced_len(&mut child, 0, 9)));
        assert!(is_invalid(&sliced_len(&mut child, 8, 1)));
        assert_eq!(sliced_len(&mut child, 2, 6), Ok(6));
    }

    let mut bytes = backing(32);
    let mut v0 = MappedRangeMut::__backend_new(&mut bytes);
    let mut v1 = v0.slice(3, 24).unwrap();
    assert_eq!(v1.len(), 24);
    let mut v2 = v1.slice(5, 16).unwrap();
    assert_eq!(v2.len(), 16);
    let mut v3 = v2.slice(1, 10).unwrap();
    assert_eq!(v3.len(), 10);
    let v4 = v3.slice(4, 6).unwrap();
    assert_eq!(v4.len(), 6);
}

// MEM-59
#[test]
fn parent_len_is_unchanged_after_slice() {
    let mut bytes = backing(16);
    let mut view = MappedRangeMut::__backend_new(&mut bytes);
    {
        let child = view.slice(4, 8).unwrap();
        assert_eq!(child.len(), 8);
    }
    assert_eq!(view.len(), 16);
    assert!(!view.is_empty());
    assert!(is_invalid(&sliced_len(&mut view, 9, 8)));
    assert_eq!(view.len(), 16);
    assert!(!view.is_empty());
}

#[test]
fn rejection_is_repeatable() {
    let mut bytes = backing(16);
    let mut view = MappedRangeMut::__backend_new(&mut bytes);
    for (o, l) in [(8u64, 9u64), (u64::MAX, 1), (17, 0)] {
        let first = sliced_len(&mut view, o, l);
        let second = sliced_len(&mut view, o, l);
        assert!(is_invalid(&first));
        assert_eq!(first, second);
    }
    for (o, l) in [(0u64, 16u64), (16, 0), (4, 8)] {
        let first = sliced_len(&mut view, o, l);
        assert_eq!(first, Ok(l));
        assert_eq!(first, sliced_len(&mut view, o, l));
    }
}

#[test]
fn rejection_is_invalid_argument_only() {
    let mut bytes = backing(16);
    let mut view = MappedRangeMut::__backend_new(&mut bytes);
    let rejected = [
        (0u64, 17u64),
        (16, 1),
        (17, 0),
        (1, 16),
        (u64::MAX, 1),
        (1, u64::MAX),
        (u64::MAX, u64::MAX),
        (u64::MAX, 0),
    ];
    for (o, l) in rejected {
        let r = sliced_len(&mut view, o, l);
        assert!(
            matches!(r, Err(ContractError::InvalidArgument(_))),
            "offset={o} len={l}"
        );
    }
}
