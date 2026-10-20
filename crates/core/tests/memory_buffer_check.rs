//! `memory` のバッファ引数検査を外の crate から公開のパスで呼ぶ統合テスト
//! （TASK-309 の 2/4。MEM-70・MEM-74〜MEM-76 のポインタ）。
//!
//! feature なしで動く。範囲の検査は `upload` の書き込み先にも `download` の読み出し元にも
//! 同じ関数を使うので、境界の入力の格子を 1 回回して確かめる。期待値は `u128` の参照で求め、
//! 検査の実装とは独立に終端のあふれと超過を判定する。

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::memory::{validate_allocation_size, validate_buffer_range};

fn is_overflow(r: &Result<(), ContractError>) -> bool {
    matches!(r, Err(ContractError::InvalidArgument(m)) if m.starts_with("buffer range end overflows u64"))
}

fn is_exceed(r: &Result<(), ContractError>) -> bool {
    matches!(r, Err(ContractError::InvalidArgument(m))
        if m.starts_with("buffer range end (") && m.contains("exceeds allocation length"))
}

fn is_size_exceed(r: &Result<(), ContractError>) -> bool {
    matches!(r, Err(ContractError::InvalidArgument(m)) if m.starts_with("allocation size ("))
}

fn dedup(mut v: Vec<u64>) -> Vec<u64> {
    v.sort_unstable();
    v.dedup();
    v
}

/// 確保長 `a` に対する境界の offset の一覧。`a + 1` は `u64` で表せないとき入れない。
fn offsets(a: u64) -> Vec<u64> {
    let mut v = vec![0, 1, a, u64::MAX];
    if a >= 1 {
        v.push(a - 1);
    }
    if let Some(n) = a.checked_add(1) {
        v.push(n);
    }
    dedup(v)
}

#[test]
fn buffer_range_boundary_grid_matches_u128_reference() {
    for a in [0u64, 1, 16, u64::MAX] {
        for offset in offsets(a) {
            for len in dedup(vec![0, 1, a, u64::MAX]) {
                let end = u128::from(offset) + u128::from(len);
                let r = validate_buffer_range(offset, len, a);
                let ctx = format!("(allocation {a}, offset {offset}, len {len})");
                if end > u128::from(u64::MAX) {
                    assert!(is_overflow(&r), "expected overflow {ctx}: {r:?}");
                } else if end > u128::from(a) {
                    assert!(is_exceed(&r), "expected exceed {ctx}: {r:?}");
                } else {
                    assert!(r.is_ok(), "expected ok {ctx}: {r:?}");
                }
            }
        }
    }
}

#[test]
fn buffer_range_boundary_end_equals_allocation_passes() {
    for a in [0u64, 1, 16, u64::MAX] {
        assert!(validate_buffer_range(0, a, a).is_ok());
        assert!(validate_buffer_range(a, 0, a).is_ok());
        if a >= 1 {
            assert!(validate_buffer_range(1, a - 1, a).is_ok());
        }
    }
}

#[test]
fn buffer_range_boundary_end_one_over_allocation_is_rejected() {
    // a == u64::MAX では a + 1 を u64 で表せない（式に書くとあふれる）ので除く。
    // その場合のあふれの枝は buffer_range_boundary_offset_max が確かめる。
    for a in [0u64, 1, 16, u64::MAX - 1] {
        let over = a + 1;
        assert!(is_exceed(&validate_buffer_range(0, over, a)));
        assert!(is_exceed(&validate_buffer_range(over, 0, a)));
        assert!(is_exceed(&validate_buffer_range(a, 1, a)));
    }
}

#[test]
fn buffer_range_boundary_offset_max() {
    assert!(validate_buffer_range(u64::MAX, 0, u64::MAX).is_ok());
    assert!(is_exceed(&validate_buffer_range(u64::MAX, 0, u64::MAX - 1)));
    assert!(is_exceed(&validate_buffer_range(u64::MAX, 0, 0)));
    for len in [1, 2, u64::MAX] {
        for a in [0, 16, u64::MAX] {
            assert!(is_overflow(&validate_buffer_range(u64::MAX, len, a)));
        }
    }
}

#[test]
fn buffer_range_boundary_len_zero() {
    for a in [0u64, 1, 16, u64::MAX] {
        for offset in offsets(a) {
            let r = validate_buffer_range(offset, 0, a);
            if offset <= a {
                assert!(r.is_ok(), "(allocation {a}, offset {offset}): {r:?}");
            } else {
                assert!(is_exceed(&r), "(allocation {a}, offset {offset}): {r:?}");
            }
        }
    }
}

#[test]
fn buffer_range_boundary_overflow_is_judged_before_exceed() {
    // あふれと超過が両方成り立つ入力は、あふれの文言で返る。
    let r = validate_buffer_range(u64::MAX, 1, 0);
    assert!(is_overflow(&r));
    assert!(!is_exceed(&r));
}

#[test]
fn allocation_size_boundary_grid() {
    for m in [0u64, 64, u64::MAX] {
        let mut lens = vec![0, 1, m, u64::MAX];
        if let Some(n) = m.checked_add(1) {
            lens.push(n);
        }
        for byte_len in dedup(lens) {
            let r = validate_allocation_size(byte_len, m);
            let ctx = format!("(byte_len {byte_len}, max {m})");
            if byte_len > m {
                assert!(is_size_exceed(&r), "expected reject {ctx}: {r:?}");
            } else {
                assert!(r.is_ok(), "expected ok {ctx}: {r:?}");
            }
        }
    }
}

#[test]
fn allocation_size_boundary_named_cases() {
    assert!(validate_allocation_size(64, 64).is_ok());
    assert!(is_size_exceed(&validate_allocation_size(65, 64)));
    assert!(validate_allocation_size(0, 64).is_ok());
    assert!(validate_allocation_size(0, 0).is_ok());
    assert!(validate_allocation_size(u64::MAX, u64::MAX).is_ok());
    assert!(is_size_exceed(&validate_allocation_size(
        u64::MAX,
        u64::MAX - 1
    )));
}
