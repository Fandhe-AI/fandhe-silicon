//! `validate_query_resolve` の整列・範囲・長さの検査を、境界の入力の格子と `u128` の参照で確かめる（TASK-1877・CMD-129・CMD-130 のポインタ）。
//!
//! 公開のパスだけから呼ぶ。誤りが 2 つ以上重なる入力は種類（`InvalidArgument`）だけを照らし、
//! 誤りが 1 つだけの入力は文言の先頭の語まで照らす。

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::execution::{
    QUERY_RESOLVE_BUFFER_ALIGNMENT, QUERY_SIZE, validate_query_resolve,
};

type Res = Result<u64, ContractError>;
type Args = (u32, u64, u32, u32, u64, u64);

fn starts(r: &Res, head: &str) -> bool {
    matches!(r, Err(ContractError::InvalidArgument(m)) if m.starts_with(head))
}

fn is_misaligned(r: &Res) -> bool {
    starts(r, "query resolve offset")
}

fn is_bad_stride(r: &Res) -> bool {
    starts(r, "query resolve stride")
}

fn is_range(r: &Res) -> bool {
    starts(r, "query range")
}

fn is_size_overflow(r: &Res) -> bool {
    starts(r, "query resolve size overflows")
}

fn is_end_overflow(r: &Res) -> bool {
    starts(r, "buffer range end overflows u64")
}

fn is_exceed(r: &Res) -> bool {
    matches!(r, Err(ContractError::InvalidArgument(m))
        if m.starts_with("buffer range end (") && m.contains("exceeds allocation length"))
}

fn dedup(mut v: Vec<u64>) -> Vec<u64> {
    v.sort_unstable();
    v.dedup();
    v
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fault {
    Misaligned,
    BadStride,
    Range,
    SizeOverflow,
    EndOverflow,
    Exceed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Expect {
    Ok(u64),
    Single(Fault),
    AnyInvalid,
}

/// 実装の計算を写さず、`u128` で独立に期待を分類する。
fn reference(a: Args) -> Expect {
    let (set_count, stride, first, count, dst_size, dst_offset) = a;
    let max = u128::from(u64::MAX);
    let mut faults = Vec::new();
    if u128::from(dst_offset) % 256 != 0 {
        faults.push(Fault::Misaligned);
    }
    if stride == 0 || stride % 8 != 0 {
        faults.push(Fault::BadStride);
    }
    if u128::from(first) + u128::from(count) > u128::from(set_count) {
        faults.push(Fault::Range);
    }
    let bytes = u128::from(count) * u128::from(stride);
    if bytes > max {
        faults.push(Fault::SizeOverflow);
    } else {
        let end = u128::from(dst_offset) + bytes;
        if end > max {
            faults.push(Fault::EndOverflow);
        } else if end > u128::from(dst_size) {
            faults.push(Fault::Exceed);
        }
    }
    match faults.len() {
        0 => Expect::Ok(u64::try_from(bytes).expect("fits u64")),
        1 => Expect::Single(faults[0]),
        _ => Expect::AnyInvalid,
    }
}

fn single_matches(fault: Fault, r: &Res) -> bool {
    match fault {
        Fault::Misaligned => is_misaligned(r),
        Fault::BadStride => is_bad_stride(r),
        Fault::Range => is_range(r),
        Fault::SizeOverflow => is_size_overflow(r),
        Fault::EndOverflow => is_end_overflow(r),
        Fault::Exceed => is_exceed(r),
    }
}

fn call(a: Args) -> Res {
    validate_query_resolve(a.0, a.1, a.2, a.3, a.4, a.5)
}

/// 参照の分類と実物を突き合わせる。
fn check(a: Args) -> Expect {
    let got = call(a);
    let want = reference(a);
    match want {
        Expect::Ok(b) => assert_eq!(got, Ok(b), "args {a:?}"),
        Expect::Single(f) => {
            assert!(single_matches(f, &got), "args {a:?} want {f:?} got {got:?}");
        }
        Expect::AnyInvalid => assert!(
            matches!(got, Err(ContractError::InvalidArgument(_))),
            "args {a:?} got {got:?}"
        ),
    }
    want
}

#[test]
fn resolve_offset_alignment() {
    assert_eq!(QUERY_RESOLVE_BUFFER_ALIGNMENT, 256);
    for off in [0u64, 256, 512] {
        assert_eq!(
            validate_query_resolve(4, 16, 0, 2, u64::MAX, off),
            Ok(32),
            "off {off}"
        );
    }
    for off in [1u64, 8, 255, 257, u64::MAX] {
        let r = validate_query_resolve(4, 16, 0, 2, u64::MAX, off);
        assert!(is_misaligned(&r), "off {off} got {r:?}");
    }
    // 256 の倍数で最大の offset。
    let off = u64::MAX - 255;
    assert_eq!(validate_query_resolve(4, 8, 0, 1, u64::MAX, off), Ok(8));
    let r = validate_query_resolve(40, 8, 0, 32, u64::MAX, off);
    assert!(is_end_overflow(&r), "got {r:?}");
    let r = validate_query_resolve(4, 8, 0, 1, 1024, off);
    assert!(is_exceed(&r), "got {r:?}");
    check((4, 8, 0, 1, u64::MAX, off));
    check((40, 8, 0, 32, u64::MAX, off));
    check((4, 8, 0, 1, 1024, off));
}

#[test]
fn resolve_boundaries() {
    // 長さちょうどと 1 バイト不足。
    assert_eq!(validate_query_resolve(4, 16, 0, 2, 288, 256), Ok(32));
    assert!(is_exceed(&validate_query_resolve(4, 16, 0, 2, 287, 256)));
    // 終端のあふれ。
    let r = validate_query_resolve(40, 8, 0, 32, u64::MAX, u64::MAX - 255);
    assert!(is_end_overflow(&r), "got {r:?}");
    // 範囲ちょうどと 1 超過。
    assert_eq!(validate_query_resolve(4, 8, 2, 2, 8192, 0), Ok(16));
    assert!(is_range(&validate_query_resolve(4, 8, 2, 3, 8192, 0)));
    assert!(is_range(&validate_query_resolve(4, 8, 3, 2, 8192, 0)));
    assert!(is_range(&validate_query_resolve(
        4,
        8,
        u32::MAX,
        u32::MAX,
        u64::MAX,
        0
    )));
    assert!(is_range(&validate_query_resolve(
        4,
        8,
        0,
        u32::MAX,
        u64::MAX,
        0
    )));
    assert!(is_range(&validate_query_resolve(
        4,
        8,
        u32::MAX,
        1,
        u64::MAX,
        0
    )));
    // 個数 0。
    assert_eq!(validate_query_resolve(4, 8, 0, 0, 0, 0), Ok(0));
    assert_eq!(validate_query_resolve(4, 8, 4, 0, 256, 256), Ok(0));
    assert!(is_range(&validate_query_resolve(4, 8, 5, 0, 256, 256)));
    assert!(is_exceed(&validate_query_resolve(4, 8, 0, 0, 256, 512)));
}

#[test]
fn resolve_grid_matches_reference() {
    let mut points = 0usize;
    let (mut n_ok, mut n_mis, mut n_range, mut n_end, mut n_exceed, mut n_multi) =
        (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
    for set_count in [1u32, 2, 4095, 4096] {
        let sc = u64::from(set_count);
        let qs: Vec<u32> = dedup(vec![
            0,
            1,
            sc.saturating_sub(1),
            sc,
            sc + 1,
            u64::from(u32::MAX),
        ])
        .into_iter()
        .map(|v| u32::try_from(v).expect("fits u32"))
        .collect();
        for stride in [8u64, 16, 24, 32, 40] {
            for &first in &qs {
                for &count in &qs {
                    let bytes = u64::from(count) * stride;
                    let offs = dedup(vec![
                        0,
                        1,
                        8,
                        255,
                        256,
                        257,
                        512,
                        bytes / 256 * 256,
                        bytes.div_ceil(256) * 256,
                        u64::MAX - 256,
                        u64::MAX - 255,
                        u64::MAX,
                    ]);
                    for &off in &offs {
                        let end = off.saturating_add(bytes);
                        let sizes = dedup(vec![
                            0,
                            end.saturating_sub(1),
                            end,
                            end.saturating_add(1),
                            u64::MAX,
                        ]);
                        for &size in &sizes {
                            let want = check((set_count, stride, first, count, size, off));
                            points += 1;
                            match want {
                                Expect::Ok(_) => n_ok += 1,
                                Expect::Single(Fault::Misaligned) => n_mis += 1,
                                Expect::Single(Fault::Range) => n_range += 1,
                                Expect::Single(Fault::EndOverflow) => n_end += 1,
                                Expect::Single(Fault::Exceed) => n_exceed += 1,
                                Expect::AnyInvalid => n_multi += 1,
                                Expect::Single(_) => {}
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(points > 3000, "points {points}");
    for (name, n) in [
        ("ok", n_ok),
        ("misaligned", n_mis),
        ("range", n_range),
        ("end overflow", n_end),
        ("exceed", n_exceed),
        ("multi", n_multi),
    ] {
        assert!(n > 0, "category {name} was never hit");
    }
}

#[test]
fn resolve_stride_values() {
    assert_eq!(QUERY_SIZE, 8);
    for stride in [0u64, 7, 9, u64::MAX] {
        let r = validate_query_resolve(4, stride, 0, 1, u64::MAX, 0);
        assert!(is_bad_stride(&r), "stride {stride} got {r:?}");
    }
    for stride in [8u64, 40, u64::MAX - 7] {
        for (count, off, size) in [
            (1u32, 0u64, u64::MAX),
            (1, 256, u64::MAX),
            (2, 0, u64::MAX),
            (0, 0, 0),
        ] {
            check((4, stride, 0, count, size, off));
        }
    }
    let r = validate_query_resolve(4, u64::MAX - 7, 0, 2, u64::MAX, 0);
    assert!(is_size_overflow(&r), "got {r:?}");
    assert_eq!(
        validate_query_resolve(4, u64::MAX - 7, 0, 1, u64::MAX, 0),
        Ok(u64::MAX - 7)
    );
    let r = validate_query_resolve(4, u64::MAX - 7, 0, 1, u64::MAX, 256);
    assert!(is_end_overflow(&r), "got {r:?}");
    // stride の誤りと積のあふれが重なる入力は種類だけ。
    assert_eq!(check((4, u64::MAX, 0, 2, u64::MAX, 0)), Expect::AnyInvalid);
}

#[test]
fn resolve_is_repeatable() {
    let inputs: [Args; 8] = [
        (4, 16, 0, 2, 512, 256),
        (4, 16, 0, 2, 287, 256),
        (4, 8, 0, 0, 0, 0),
        (4, 8, 0, 1, 64, 255),
        (4, 0, 0, 1, 64, 0),
        (4, 8, 3, 2, 64, 0),
        (4, u64::MAX, 0, 2, u64::MAX, 255),
        (40, 8, 0, 32, u64::MAX, u64::MAX - 255),
    ];
    for t in inputs {
        assert_eq!(call(t), call(t), "args {t:?}");
    }
}
