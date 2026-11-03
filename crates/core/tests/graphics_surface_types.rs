//! `graphics` の表示先の設定の大きさの検査と取得の結果の型を、外の crate から公開のパスで確かめる公開テスト（TASK-654）。
//! feature `graphics` の裏でだけ走る。

use std::collections::HashSet;

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::graphics::{
    CurrentSurfaceTexture, SurfaceStatus, validate_offscreen_target_size, validate_surface_size,
};

/// 試験の入力として使う上限の数。
const LIMIT: u32 = 16384;

fn is_invalid(r: &Result<(), ContractError>) -> bool {
    matches!(r, Err(ContractError::InvalidArgument(_)))
}

/// 取得した値を持つ結果だけ値を取り出す。外の crate では `_` の腕が要る。
fn carried(v: &CurrentSurfaceTexture<u8>) -> Option<u8> {
    match v {
        CurrentSurfaceTexture::Success(x) | CurrentSurfaceTexture::Suboptimal(x) => Some(*x),
        _ => None,
    }
}

fn all_results() -> [CurrentSurfaceTexture<u8>; 7] {
    [
        CurrentSurfaceTexture::Success(7),
        CurrentSurfaceTexture::Suboptimal(7),
        CurrentSurfaceTexture::Timeout,
        CurrentSurfaceTexture::Occluded,
        CurrentSurfaceTexture::Outdated,
        CurrentSurfaceTexture::Lost,
        CurrentSurfaceTexture::Validation,
    ]
}

// SURF-7: 0 は拒否する。
#[test]
fn zero_size_is_rejected() {
    for (w, h) in [(0, 720), (1280, 0), (0, 0)] {
        assert!(is_invalid(&validate_surface_size(w, h, LIMIT)));
    }
}

// SURF-7: 上限ちょうどは通る。
#[test]
fn exact_limit_is_accepted() {
    for (w, h) in [(LIMIT, LIMIT), (LIMIT, 1), (1, LIMIT)] {
        assert_eq!(validate_surface_size(w, h, LIMIT), Ok(()));
    }
}

// SURF-7: 上限を 1 超えると拒否する。
#[test]
fn limit_plus_one_is_rejected() {
    for (w, h) in [(LIMIT + 1, 1), (1, LIMIT + 1), (LIMIT + 1, LIMIT + 1)] {
        assert!(is_invalid(&validate_surface_size(w, h, LIMIT)));
    }
}

// SURF-7: 描画先の検査と通る・拒否の別が一致する。
#[test]
fn same_result_as_offscreen_check() {
    let sizes = [
        (0, 720),
        (1280, 0),
        (0, 0),
        (LIMIT, LIMIT),
        (LIMIT, 1),
        (1, LIMIT),
        (LIMIT + 1, 1),
        (1, LIMIT + 1),
        (LIMIT + 1, LIMIT + 1),
    ];
    for (w, h) in sizes {
        let s = validate_surface_size(w, h, LIMIT);
        let o = validate_offscreen_target_size(w, h, LIMIT);
        assert_eq!(s.is_ok(), o.is_ok());
        if s.is_err() {
            assert!(is_invalid(&s));
            assert!(is_invalid(&o));
        }
    }
}

// SURF-7: 同じ引数なら何度呼んでも同じ結果になる。
#[test]
fn rejection_is_repeatable() {
    for (w, h) in [(0, 720), (LIMIT + 1, 1), (LIMIT, LIMIT)] {
        assert_eq!(
            validate_surface_size(w, h, LIMIT),
            validate_surface_size(w, h, LIMIT)
        );
    }
}

// SURF-9: 値を持つのは Success と Suboptimal だけ。
#[test]
fn only_success_and_suboptimal_carry_value() {
    let got: Vec<Option<u8>> = all_results().iter().map(carried).collect();
    assert_eq!(got, [Some(7), Some(7), None, None, None, None, None]);
}

// SURF-9: status() の対応。
#[test]
fn status_mapping() {
    let got: Vec<SurfaceStatus> = all_results().iter().map(|r| r.status()).collect();
    assert_eq!(
        got,
        [
            SurfaceStatus::Good,
            SurfaceStatus::Suboptimal,
            SurfaceStatus::Timeout,
            SurfaceStatus::Occluded,
            SurfaceStatus::Outdated,
            SurfaceStatus::Lost,
            SurfaceStatus::Validation,
        ]
    );
}

// SURF-9: Outdated は値を持たない。
#[test]
fn outdated_carries_no_value() {
    let v = CurrentSurfaceTexture::<u8>::Outdated;
    assert_eq!(carried(&v), None);
    assert_eq!(v.status(), SurfaceStatus::Outdated);
}

// SURF-9: ALL は 7 値で重複がない。
#[test]
fn status_all_values() {
    assert_eq!(SurfaceStatus::ALL.len(), 7);
    let set: HashSet<_> = SurfaceStatus::ALL.iter().copied().collect();
    assert_eq!(set.len(), 7);
}
