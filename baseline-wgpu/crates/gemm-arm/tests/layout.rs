//! 詰め物・切り出し・起動の引数の純粋関数の試験（device は使わない）。値は行の表から計算した合成の値だけ。

use baseline_wgpu_gemm_arm::layout::{
    GemmLayout, LayoutError, constants_bytes, crop_c, pad_a_binary16, pad_a_f32, pad_b_binary16,
    pad_b_f32, padded_dim, workgroup_count,
};
use fandhe_silicon_core::parity::gemm_inputs::GemmGateRow;

#[test]
fn row_sizes_after_padding() {
    for row in GemmGateRow::ALL {
        let l = GemmLayout::for_row(row).expect("layout");
        let expected = if row == GemmGateRow::Odd37x41x53 {
            (40, 48, 56)
        } else {
            (row.m() as u32, row.n() as u32, row.k() as u32)
        };
        assert_eq!((l.mp(), l.np(), l.kp()), expected, "{}", row.name());
        assert_eq!(
            (l.m() as usize, l.n() as usize, l.k() as usize),
            (row.m(), row.n(), row.k())
        );
    }
}

#[test]
fn padded_dim_boundaries() {
    for (x, want) in [
        (0, 0),
        (1, 8),
        (8, 8),
        (9, 16),
        (u32::MAX - 7, u32::MAX - 7),
    ] {
        assert_eq!(padded_dim(x), Some(want));
    }
    assert_eq!(padded_dim(u32::MAX - 6), None);
    assert_eq!(padded_dim(u32::MAX), None);
}

#[test]
fn new_rejects_overflowing_dimensions() {
    assert_eq!(
        GemmLayout::new(u32::MAX as usize, 8, 8),
        Err(LayoutError::DimensionOverflow)
    );
    assert_eq!(
        GemmLayout::new(8, u32::MAX as usize - 6, 8),
        Err(LayoutError::DimensionOverflow)
    );
    assert_eq!(
        GemmLayout::new(8, 8, u32::MAX as usize + 1),
        Err(LayoutError::DimensionOverflow)
    );
}

#[test]
fn padding_places_zeros_and_keeps_values() {
    let l = GemmLayout::new(3, 5, 6).expect("layout");
    assert_eq!((l.mp(), l.np(), l.kp()), (8, 8, 8));
    let a: Vec<f32> = (1..=18).map(|v| v as f32).collect();
    let b: Vec<f32> = (1..=30).map(|v| v as f32).collect();
    let pa = pad_a_f32(&l, &a).expect("pad a");
    let pb = pad_b_f32(&l, &b).expect("pad b");
    assert_eq!(pa.len(), 64);
    assert_eq!(pb.len(), 64);
    for r in 0..8 {
        for c in 0..8 {
            let want = if r < 3 && c < 6 { a[r * 6 + c] } else { 0.0 };
            assert_eq!(pa[r * 8 + c].to_bits(), want.to_bits());
            let want = if r < 6 && c < 5 { b[r * 5 + c] } else { 0.0 };
            assert_eq!(pb[r * 8 + c].to_bits(), want.to_bits());
        }
    }
}

#[test]
fn binary16_padding_uses_zero_bits() {
    let l = GemmLayout::new(3, 5, 6).expect("layout");
    let a: Vec<u16> = (1..=18).map(|v| 0x3c00 + v).collect();
    let b: Vec<u16> = (1..=30).map(|v| 0x3c00 + v).collect();
    let pa = pad_a_binary16(&l, &a).expect("pad a");
    let pb = pad_b_binary16(&l, &b).expect("pad b");
    for r in 0..8 {
        for c in 0..8 {
            let want = if r < 3 && c < 6 { a[r * 6 + c] } else { 0 };
            assert_eq!(pa[r * 8 + c], want);
            let want = if r < 6 && c < 5 { b[r * 5 + c] } else { 0 };
            assert_eq!(pb[r * 8 + c], want);
        }
    }
}

#[test]
fn crop_restores_original() {
    let l = GemmLayout::new(3, 5, 6).expect("layout");
    let c: Vec<f32> = (1..=15).map(|v| v as f32).collect();
    // M x N を Mp x Np へ詰めてから切り出すと元に戻る。
    let mut padded = vec![-1.0f32; l.c_padded_len().expect("len")];
    for r in 0..3 {
        for col in 0..5 {
            padded[r * l.np() as usize + col] = c[r * 5 + col];
        }
    }
    assert_eq!(crop_c(&l, &padded).expect("crop"), c);
}

#[test]
fn length_mismatch_is_err() {
    let l = GemmLayout::new(3, 5, 6).expect("layout");
    for len in [17usize, 19] {
        assert_eq!(
            pad_a_f32(&l, &vec![0.0; len]),
            Err(LayoutError::LengthMismatch)
        );
        assert_eq!(
            pad_a_binary16(&l, &vec![0; len]),
            Err(LayoutError::LengthMismatch)
        );
    }
    for len in [29usize, 31] {
        assert_eq!(
            pad_b_f32(&l, &vec![0.0; len]),
            Err(LayoutError::LengthMismatch)
        );
        assert_eq!(
            pad_b_binary16(&l, &vec![0; len]),
            Err(LayoutError::LengthMismatch)
        );
    }
    for len in [63usize, 65] {
        assert_eq!(
            crop_c(&l, &vec![0.0; len]),
            Err(LayoutError::LengthMismatch)
        );
    }
}

#[test]
fn workgroup_count_and_constants() {
    let l = GemmLayout::for_row(GemmGateRow::Odd37x41x53).expect("layout");
    assert_eq!(workgroup_count(&l), [6, 5, 1]);
    let bytes = constants_bytes(&l);
    assert_eq!(bytes.len(), 16);
    assert_eq!(u32::from_le_bytes(bytes[0..4].try_into().unwrap()), 40);
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 56);
    assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 48);
    assert_eq!(&bytes[12..16], &[0, 0, 0, 0]);

    let l = GemmLayout::for_row(GemmGateRow::Rect4096x1024x4096).expect("layout");
    assert_eq!(workgroup_count(&l), [128, 512, 1]);
}
