//! 下-2 の入口へ渡す入力と出力の列の型。
//!
//! 各型は、ホストのスライスで渡す `Host` の形と、device の buffer で渡す `Device` の形を持つ。
//! `Device` の形は feature `native-blocking-wait` のビルドにだけ在る。1 つの値の中で
//! 2 つの形は混ざらない。

#[cfg(feature = "native-blocking-wait")]
use fandhe_silicon_core::memory::BufferHandle;

/// f16 の融合積和（`a * b + c`）の列。f16 は `u16` のビット列で渡す。
#[derive(Debug)]
pub enum F16FmaOperands<'a> {
    /// ホストのスライスで渡す形。
    Host {
        /// 乗数の列。
        a: &'a [u16],
        /// 乗数の列。
        b: &'a [u16],
        /// 加数の列。
        c: &'a [u16],
        /// 結果の列。
        out: &'a mut [u16],
    },
    /// device の buffer で渡す形。欄は排他の借用なので同じ buffer を 2 欄に渡せない。
    #[cfg(feature = "native-blocking-wait")]
    Device {
        /// 乗数の buffer。
        a: &'a mut BufferHandle,
        /// 乗数の buffer。
        b: &'a mut BufferHandle,
        /// 加数の buffer。
        c: &'a mut BufferHandle,
        /// 結果の buffer。
        out: &'a mut BufferHandle,
    },
}

/// subgroup の u32 の総和の列。
#[derive(Debug)]
pub enum SubgroupSumU32Operands<'a> {
    /// ホストのスライスで渡す形。
    Host {
        /// 入力の列。
        x: &'a [u32],
        /// 結果の列。
        out: &'a mut [u32],
    },
    /// device の buffer で渡す形。欄は排他の借用なので同じ buffer を 2 欄に渡せない。
    #[cfg(feature = "native-blocking-wait")]
    Device {
        /// 入力の buffer。
        x: &'a mut BufferHandle,
        /// 結果の buffer。
        out: &'a mut BufferHandle,
    },
}

/// subgroup の f32 の総和の列。
#[derive(Debug)]
pub enum SubgroupSumF32Operands<'a> {
    /// ホストのスライスで渡す形。
    Host {
        /// 入力の列。
        x: &'a [f32],
        /// 結果の列。
        out: &'a mut [f32],
    },
    /// device の buffer で渡す形。欄は排他の借用なので同じ buffer を 2 欄に渡せない。
    #[cfg(feature = "native-blocking-wait")]
    Device {
        /// 入力の buffer。
        x: &'a mut BufferHandle,
        /// 結果の buffer。
        out: &'a mut BufferHandle,
    },
}

/// atomic の f32 の加算の列。
#[derive(Debug)]
pub enum AtomicAddOperands<'a> {
    /// ホストのスライスで渡す形。
    Host {
        /// 加える値の列。
        x: &'a [f32],
        /// 結果の列。
        out: &'a mut [f32],
    },
    /// device の buffer で渡す形。欄は排他の借用なので同じ buffer を 2 欄に渡せない。
    #[cfg(feature = "native-blocking-wait")]
    Device {
        /// 加える値の buffer。
        x: &'a mut BufferHandle,
        /// 結果の buffer。
        out: &'a mut BufferHandle,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_forms_hold_slices() {
        let (a, b, c) = ([1u16, 2], [3u16, 4], [5u16, 6]);
        let mut out = [0u16; 2];
        let v = F16FmaOperands::Host {
            a: &a,
            b: &b,
            c: &c,
            out: &mut out,
        };
        assert!(
            matches!(&v, F16FmaOperands::Host { a, out, .. } if a.len() == 2 && out.len() == 2)
        );

        let x = [1u32, 2, 3];
        let mut o = [0u32; 1];
        let v = SubgroupSumU32Operands::Host { x: &x, out: &mut o };
        assert!(
            matches!(&v, SubgroupSumU32Operands::Host { x, out } if x[2] == 3 && out.len() == 1)
        );

        let x = [1.0f32, 2.0];
        let mut o = [0.0f32; 1];
        let v = SubgroupSumF32Operands::Host { x: &x, out: &mut o };
        assert!(matches!(&v, SubgroupSumF32Operands::Host { x, .. } if x.len() == 2));

        let mut o = [0.0f32; 2];
        let v = AtomicAddOperands::Host { x: &x, out: &mut o };
        assert!(matches!(&v, AtomicAddOperands::Host { out, .. } if out.len() == 2));
    }

    #[cfg(feature = "native-blocking-wait")]
    #[test]
    fn device_forms_take_exclusive_buffer_borrows() {
        fn f16<'a>(
            a: &'a mut BufferHandle,
            b: &'a mut BufferHandle,
            c: &'a mut BufferHandle,
            out: &'a mut BufferHandle,
        ) -> F16FmaOperands<'a> {
            F16FmaOperands::Device { a, b, c, out }
        }
        fn u32s<'a>(
            x: &'a mut BufferHandle,
            out: &'a mut BufferHandle,
        ) -> SubgroupSumU32Operands<'a> {
            SubgroupSumU32Operands::Device { x, out }
        }
        fn f32s<'a>(
            x: &'a mut BufferHandle,
            out: &'a mut BufferHandle,
        ) -> SubgroupSumF32Operands<'a> {
            SubgroupSumF32Operands::Device { x, out }
        }
        fn atomic<'a>(x: &'a mut BufferHandle, out: &'a mut BufferHandle) -> AtomicAddOperands<'a> {
            AtomicAddOperands::Device { x, out }
        }
        let _ = (f16, u32s, f32s, atomic);
    }
}
