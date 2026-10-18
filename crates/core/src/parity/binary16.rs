//! binary16（IEEE 754 の半精度）と f32 の相互変換（spec TASK-1005 の 2/6）。
//!
//! GEMM の門の run で、入力を f32 から binary16 へ 1 回だけ丸める側と、binary16 の値を f32 へ
//! 正確に広げる側（CPU 参照・出力の比較）から呼ばれる。標準ライブラリだけで書き、外部依存は持たない。
//! TASK-929 の 4/6 で、f64 から 1 回で丸める入口と f64 へ広げる入口を足した。
//! ビット列は `u16` で持つ（型名に裸の半精度のプリミティブ名は使わない）。

use super::cpu_reference::ReferenceError;

/// f32 を binary16 のビット列へ、最近接偶数で 1 回だけ丸める。
///
/// - 零の符号は保つ。非正規化数（最小は 2^-24）も正しく丸める
/// - 桁あふれは符号つきの無限大へ丸める（65520 以上）。無限大は無限大のまま
/// - NaN は NaN のまま。符号と仮数の上位 9 bit を保ち、静かな NaN の印の bit を立てる
pub fn f32_to_binary16_bits(x: f32) -> u16 {
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
        // 非正規化数の範囲。-10 を下回ると最小の非正規化数の半分に届かず、零へ丸まる。
        if e < -10 {
            return sign;
        }
        let full = man | 0x0080_0000;
        let shift = (14 - e) as u32;
        let q = full >> shift;
        let rem = full & ((1u32 << shift) - 1);
        let half = 1u32 << (shift - 1);
        let q = if rem > half || (rem == half && (q & 1) == 1) {
            q + 1
        } else {
            q
        };
        // q が 0x400 に届いたときは最小の正規化数で、そのまま正しいビット列になる。
        return sign | q as u16;
    }

    let q = ((e as u32) << 10) | (man >> 13);
    let rem = man & 0x1fff;
    let q = if rem > 0x1000 || (rem == 0x1000 && (q & 1) == 1) {
        // 仮数の繰り上がりは指数へ伝わる。e == 30 のときは無限大のビット列になる。
        q + 1
    } else {
        q
    };
    sign | q as u16
}

/// binary16 のビット列を f32 へ正確に広げる（非正規化数・無限大・NaN を含む）。
pub fn binary16_bits_to_f32(bits: u16) -> f32 {
    let sign = (u32::from(bits) & 0x8000) << 16;
    let exp = (u32::from(bits) >> 10) & 0x1f;
    let man = u32::from(bits) & 0x3ff;
    match exp {
        0 => {
            // 非正規化数は man * 2^-24（f32 で正確に表せる）。
            let magnitude = (man as f32) * f32::from_bits(0x3380_0000);
            f32::from_bits(magnitude.to_bits() | sign)
        }
        31 => f32::from_bits(sign | 0x7f80_0000 | (man << 13)),
        _ => f32::from_bits(sign | ((exp + 112) << 23) | (man << 13)),
    }
}

/// f64 を binary16 のビット列へ、f32 を経由せず最近接偶数で 1 回だけ丸める。
///
/// 規則は [`f32_to_binary16_bits`] と同じ（零の符号を保つ・非正規化数は最小が 2^-24・65520 以上は
/// 符号つきの無限大・NaN は符号と仮数の上位 bit を保って静かな NaN にする）。
/// f64 の非正規化数は零へ丸まる。f32 を経由すると丸めが 2 回になる値でも、ここでは 1 回で決まる。
/// 代わりの実行の CPU の参照（`fallback_reference`）が、f64 の積和を丸めるときに呼ぶ。
pub fn f64_to_binary16_bits(x: f64) -> u16 {
    let bits = x.to_bits();
    let sign = ((bits >> 48) & 0x8000) as u16;
    let exp = ((bits >> 52) & 0x7ff) as i32;
    let man = bits & 0x000f_ffff_ffff_ffff;

    if exp == 0x7ff {
        if man == 0 {
            return sign | 0x7c00;
        }
        return sign | 0x7e00 | ((man >> 42) as u16 & 0x3ff);
    }

    let e = exp - 1023 + 15;
    if e >= 31 {
        return sign | 0x7c00;
    }
    if e <= 0 {
        // -10 を下回ると最小の非正規化数の半分に届かず零へ丸まる（f64 の非正規化数もここ）。
        if e < -10 {
            return sign;
        }
        let full = man | (1u64 << 52);
        // e は -10..=0 なので shift は 43..=53 で、u64 のシフト量として安全。
        let shift = (43 - e) as u32;
        let q = full >> shift;
        let rem = full & ((1u64 << shift) - 1);
        let half = 1u64 << (shift - 1);
        let q = if rem > half || (rem == half && (q & 1) == 1) {
            q + 1
        } else {
            q
        };
        return sign | q as u16;
    }

    let q = ((e as u64) << 10) | (man >> 42);
    let rem = man & ((1u64 << 42) - 1);
    let half = 1u64 << 41;
    let q = if rem > half || (rem == half && (q & 1) == 1) {
        // 仮数の繰り上がりは指数へ伝わる。e == 30 のときは無限大のビット列になる。
        q + 1
    } else {
        q
    };
    sign | q as u16
}

/// binary16 のビット列を f64 へ正確に広げる（非正規化数・無限大・NaN を含む）。
pub fn binary16_bits_to_f64(bits: u16) -> f64 {
    f64::from(binary16_bits_to_f32(bits))
}

/// `values` の各要素を [`f32_to_binary16_bits`] で 1 回だけ丸めた列を返す。
///
/// 確保に失敗したら [`ReferenceError::AllocationFailed`] を返す（panic しない）。
pub fn round_slice_to_binary16(values: &[f32]) -> Result<Vec<u16>, ReferenceError> {
    let mut out = Vec::new();
    out.try_reserve_exact(values.len())?;
    out.extend(values.iter().map(|&v| f32_to_binary16_bits(v)));
    Ok(out)
}

/// `bits` の各要素を [`binary16_bits_to_f32`] で広げた列を返す。
///
/// 確保に失敗したら [`ReferenceError::AllocationFailed`] を返す（panic しない）。
pub fn widen_binary16_slice(bits: &[u16]) -> Result<Vec<f32>, ReferenceError> {
    let mut out = Vec::new();
    out.try_reserve_exact(bits.len())?;
    out.extend(bits.iter().map(|&b| binary16_bits_to_f32(b)));
    Ok(out)
}
