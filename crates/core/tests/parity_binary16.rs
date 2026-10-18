//! binary16 の変換の振る舞いの確認（spec TASK-1005 の 2/6）。
//! feature `test-support` の裏にだけ存在する検証用の道具を対象にする。

use fandhe_silicon_core::parity::binary16::{
    binary16_bits_to_f32, f32_to_binary16_bits, round_slice_to_binary16, widen_binary16_slice,
};

#[test]
fn every_bit_pattern_round_trips() {
    for b in 0..=u16::MAX {
        let wide = binary16_bits_to_f32(b);
        let is_nan = (b & 0x7c00) == 0x7c00 && (b & 0x03ff) != 0;
        if is_nan {
            assert!(wide.is_nan(), "bits {b:#06x}");
            let back = f32_to_binary16_bits(wide);
            assert_eq!(back & 0x7c00, 0x7c00);
            assert_ne!(back & 0x03ff, 0);
        } else {
            assert_eq!(f32_to_binary16_bits(wide), b, "bits {b:#06x}");
        }
    }
}

#[test]
fn ties_round_to_even() {
    let ulp = 2.0_f32.powi(-10);
    // 1.0 と次の値の中間は偶数側（1.0）へ。
    assert_eq!(f32_to_binary16_bits(1.0 + ulp / 2.0), 0x3c00);
    // 中間を 1 ulp（f32）超えると上へ。
    assert_eq!(
        f32_to_binary16_bits(f32::from_bits((1.0 + ulp / 2.0).to_bits() + 1)),
        0x3c01
    );
    // 奇数側（1+ulp）と次の中間は偶数側（1+2ulp）へ。
    assert_eq!(f32_to_binary16_bits(1.0 + ulp * 1.5), 0x3c02);
}

#[test]
fn subnormal_boundaries() {
    let min = 2.0_f32.powi(-24);
    assert_eq!(f32_to_binary16_bits(min), 0x0001);
    assert_eq!(f32_to_binary16_bits(min / 2.0), 0x0000);
    assert_eq!(
        f32_to_binary16_bits(f32::from_bits((min / 2.0).to_bits() + 1)),
        0x0001
    );
    assert_eq!(f32_to_binary16_bits(-min / 2.0), 0x8000);
    assert_eq!(f32_to_binary16_bits(min * 1.5), 0x0002);
    assert_eq!(f32_to_binary16_bits(2.0_f32.powi(-14)), 0x0400);
    assert_eq!(f32_to_binary16_bits(f32::MIN_POSITIVE), 0x0000);
}

#[test]
fn overflow_boundaries() {
    assert_eq!(f32_to_binary16_bits(65504.0), 0x7bff);
    assert_eq!(f32_to_binary16_bits(65519.0), 0x7bff);
    assert_eq!(f32_to_binary16_bits(65520.0), 0x7c00);
    assert_eq!(f32_to_binary16_bits(-65520.0), 0xfc00);
    assert_eq!(f32_to_binary16_bits(f32::MAX), 0x7c00);
    assert_eq!(f32_to_binary16_bits(f32::INFINITY), 0x7c00);
    assert_eq!(f32_to_binary16_bits(f32::NEG_INFINITY), 0xfc00);
}

#[test]
fn zero_sign_is_kept() {
    assert_eq!(f32_to_binary16_bits(0.0), 0x0000);
    assert_eq!(f32_to_binary16_bits(-0.0), 0x8000);
    assert_eq!(binary16_bits_to_f32(0x8000).to_bits(), (-0.0_f32).to_bits());
}

#[test]
fn nan_stays_nan() {
    let b = f32_to_binary16_bits(f32::NAN);
    assert_eq!(b & 0x7c00, 0x7c00);
    assert_ne!(b & 0x03ff, 0);
    // 仮数の上位が 0 の NaN も NaN のまま（静かな NaN の印が立つ）。
    let b = f32_to_binary16_bits(f32::from_bits(0x7f80_0001));
    assert_eq!(b & 0x7c00, 0x7c00);
    assert_ne!(b & 0x03ff, 0);
}

#[test]
fn slice_versions_match_elementwise() {
    let values = [0.0_f32, -0.0, 1.0, 0.1, -3.5, 65520.0, 1e-8, f32::NAN];
    let bits = round_slice_to_binary16(&values).unwrap();
    assert_eq!(bits.len(), values.len());
    for (v, b) in values.iter().zip(&bits) {
        assert_eq!(*b, f32_to_binary16_bits(*v));
    }
    let wide = widen_binary16_slice(&bits).unwrap();
    for (b, w) in bits.iter().zip(&wide) {
        assert_eq!(w.to_bits(), binary16_bits_to_f32(*b).to_bits());
    }
}
