//! CPU の経路が使う、binary16 のビット列と f32 の相互変換（`docs/spec/05-tasks.md` の TASK-930）。
//!
//! 標準ライブラリだけで書く。`cpu` の関数から呼ばれる。

/// binary16 のビット列を f32 へ正確に広げる（非正規化数・±0・±Inf・NaN を含む）。
pub(crate) fn to_f32(bits: u16) -> f32 {
    let sign = u32::from(bits & 0x8000) << 16;
    let exp = u32::from((bits >> 10) & 0x1f);
    let man = u32::from(bits & 0x03ff);
    match exp {
        0 => {
            // 非正規化数と零: 仮数 × 2^-24 は f32 で正確に表せる。
            let mag = man as f32 * f32::from_bits(0x3380_0000);
            f32::from_bits(sign | mag.to_bits())
        }
        31 => f32::from_bits(sign | 0x7f80_0000 | (man << 13)),
        _ => f32::from_bits(sign | ((exp + 112) << 23) | (man << 13)),
    }
}

/// f32 を binary16 のビット列へ最近接偶数で 1 回だけ丸める。
pub(crate) fn from_f32(x: f32) -> u16 {
    let bits = x.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp = ((bits >> 23) & 0xff) as i32;
    let man = bits & 0x007f_ffff;
    if exp == 0xff {
        if man == 0 {
            return sign | 0x7c00;
        }
        return sign | 0x7e00 | (man >> 13) as u16;
    }
    let e = exp - 127 + 15;
    if e >= 31 {
        return sign | 0x7c00;
    }
    if e <= 0 {
        if e < -10 {
            return sign;
        }
        // 非正規化数: 暗黙の 1 を足した仮数を右へずらして丸める。
        let full = man | 0x0080_0000;
        let shift = (14 - e) as u32;
        let half = 1u32 << (shift - 1);
        let rem = full & ((1u32 << shift) - 1);
        let mut m = full >> shift;
        if rem > half || (rem == half && (m & 1) == 1) {
            m += 1;
        }
        return sign | m as u16;
    }
    let m = man >> 13;
    let rem = man & 0x1fff;
    let mut out = ((e as u32) << 10) | m;
    if rem > 0x1000 || (rem == 0x1000 && (m & 1) == 1) {
        // 仮数の繰り上がりは指数へ伝わる（最大値からは無限大になる）。
        out += 1;
    }
    sign | out as u16
}

#[cfg(test)]
mod tests {
    use super::*;
    use fandhe_silicon_core::parity::binary16::{binary16_bits_to_f32, f32_to_binary16_bits};

    #[test]
    fn widening_matches_core_for_all_bit_patterns() {
        for h in 0..=u16::MAX {
            assert_eq!(
                to_f32(h).to_bits(),
                binary16_bits_to_f32(h).to_bits(),
                "h={h:#06x}"
            );
        }
    }

    #[test]
    fn rounding_matches_core() {
        let mut xs: Vec<f32> = vec![
            65504.0,
            65519.0,
            65520.0,
            f32::MAX,
            f32::from_bits(1),
            0.0,
            -0.0,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NAN,
        ];
        let widened: Vec<f32> = (0..=u16::MAX).map(binary16_bits_to_f32).collect();
        for &w in &widened {
            xs.push(w);
            let b = w.to_bits();
            xs.push(f32::from_bits(b.wrapping_sub(1)));
            xs.push(f32::from_bits(b.wrapping_add(1)));
        }
        for pair in widened.windows(2) {
            let (p, q) = (pair[0], pair[1]);
            if p.is_finite() && q.is_finite() {
                xs.push(((f64::from(p) + f64::from(q)) / 2.0) as f32);
            }
        }
        for x in xs {
            assert_eq!(
                from_f32(x),
                f32_to_binary16_bits(x),
                "x bits={:#010x}",
                x.to_bits()
            );
        }
    }
}
