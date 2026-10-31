//! 間接実行の引数の範囲・整列の検査を、境界の入力の格子と `u128` の参照で確かめる（TASK-1878 のポインタ）。
//!
//! `validate_indirect_args_range` を公開のパスから呼び、拒否の種類と判定の順を固定する。

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::execution::validate_indirect_args_range;

fn is_misaligned(r: &Result<(), ContractError>) -> bool {
    matches!(r, Err(ContractError::InvalidArgument(m)) if m.starts_with("indirect args offset"))
}

fn is_overflow(r: &Result<(), ContractError>) -> bool {
    matches!(r, Err(ContractError::InvalidArgument(m)) if m.starts_with("buffer range end overflows u64"))
}

fn is_exceed(r: &Result<(), ContractError>) -> bool {
    matches!(r, Err(ContractError::InvalidArgument(m))
        if m.starts_with("buffer range end (") && m.contains("exceeds allocation length"))
}

fn dedup(mut v: Vec<u64>) -> Vec<u64> {
    v.sort_unstable();
    v.dedup();
    v
}

fn sizes() -> Vec<u64> {
    vec![0, 1, 12, 16, 20]
}

fn lens(s: u64) -> Vec<u64> {
    dedup(vec![
        0,
        3,
        4,
        s.saturating_sub(1),
        s,
        s + 1,
        u64::MAX - 3,
        u64::MAX,
    ])
}

fn offsets(s: u64, len: u64) -> Vec<u64> {
    let mut v = vec![
        0,
        1,
        2,
        3,
        4,
        len,
        u64::MAX - 3,
        u64::MAX - 2,
        u64::MAX - 1,
        u64::MAX,
    ];
    for base in [len.saturating_sub(s), u64::MAX - s] {
        v.push(base);
        v.push(base.saturating_sub(1));
        v.push(base.saturating_add(1));
    }
    v.push(len.saturating_add(1));
    dedup(v)
}

#[test]
fn range_grid_matches_u128_reference() {
    let mut points = 0usize;
    for s in sizes() {
        for len in lens(s) {
            for off in offsets(s, len) {
                points += 1;
                let got = validate_indirect_args_range(off, s, len);
                let end = u128::from(off) + u128::from(s);
                let ctx = format!("(offset={off}, args_size={s}, buffer_len={len})");
                if off % 4 != 0 {
                    assert!(is_misaligned(&got), "{ctx}: {got:?}");
                } else if end > u128::from(u64::MAX) {
                    assert!(is_overflow(&got), "{ctx}: {got:?}");
                } else if end > u128::from(len) {
                    assert!(is_exceed(&got), "{ctx}: {got:?}");
                } else {
                    assert_eq!(got, Ok(()), "{ctx}");
                }
            }
        }
    }
    assert!(points > 200, "grid too small: {points}");
}

#[test]
fn every_rejection_is_invalid_argument() {
    for s in sizes() {
        for len in lens(s) {
            for off in offsets(s, len) {
                if let Err(e) = validate_indirect_args_range(off, s, len) {
                    assert!(
                        matches!(e, ContractError::InvalidArgument(_)),
                        "({off}, {s}, {len}): {e:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn misalignment_is_reported_before_overflow_and_excess() {
    for (off, s, len) in [(u64::MAX, 16, 0), (1, 16, 0), (u64::MAX - 2, 20, u64::MAX)] {
        let r = validate_indirect_args_range(off, s, len);
        assert!(is_misaligned(&r), "({off}, {s}, {len}): {r:?}");
    }
}

#[test]
fn end_exactly_at_length_is_accepted() {
    for s in [0u64, 12, 16, 20] {
        assert_eq!(validate_indirect_args_range(0, s, s), Ok(()));
        assert_eq!(validate_indirect_args_range(4, s, 4 + s), Ok(()));
        assert_eq!(validate_indirect_args_range(8, s, 8 + s), Ok(()));
    }
    assert_eq!(
        validate_indirect_args_range(u64::MAX - 3, 3, u64::MAX),
        Ok(())
    );
    assert_eq!(
        validate_indirect_args_range(u64::MAX - 3, 0, u64::MAX - 3),
        Ok(())
    );
}

#[test]
fn start_beyond_length_is_rejected_even_for_empty_args() {
    let r = validate_indirect_args_range(8, 0, 4);
    assert!(is_exceed(&r), "{r:?}");
    let r = validate_indirect_args_range(4, 0, 0);
    assert!(is_exceed(&r), "{r:?}");
}

#[test]
fn end_overflow_boundary_near_u64_max() {
    // u64::MAX - 3 は 4 の倍数。終端が u64::MAX ちょうどまでは通り、1 つ超えるとあふれる。
    let r = validate_indirect_args_range(u64::MAX - 3, 3, u64::MAX);
    assert_eq!(r, Ok(()));
    let r = validate_indirect_args_range(u64::MAX - 3, 4, u64::MAX);
    assert!(is_overflow(&r), "{r:?}");
}
