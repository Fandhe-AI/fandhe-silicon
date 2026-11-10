//! 受け取ったシェーダーのテキストを Metal で直接流す経路（橋の `run_g3prime`）の、`unsafe` を持たない前半（spec TASK-1010 のポインタ）。
//!
//! 橋の `run_g3prime` が最初に [`check_g3prime_request`] を呼び、続けて詰め物（[`pad_g3prime`]）、
//! macOS だけの wgpu の半分（pipeline の作成の間の記録からテキストを 1 つ受け取り、言語版と束縛の番号を読む）を呼ぶ。
//! この module は device の前の検査と純粋な変換だけを持ち、Metal の呼び出しと `unsafe` は橋の module にある。
//! どの失敗も別の経路で計算せず `Err` を返す。

use fandhe_silicon_core::parity::gemm_gate::{GateInputs, ValidationLayerSetting};
use fandhe_silicon_core::parity::gemm_inputs::GemmInputDtype;
use fandhe_silicon_core::parity::gemm_population::{GemmGateArm, GemmGateCell};

use crate::device::ArmError;
use crate::layout::{
    GemmLayout, LayoutError, pad_a_binary16, pad_a_f32, pad_b_binary16, pad_b_f32,
};

/// 受け取ったシェーダーのテキストの 1 行目で受ける言語版。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimeLanguageVersion {
    /// 2.3。
    V2_3,
    /// 2.4。
    V2_4,
    /// 3.0。
    V3_0,
    /// 3.1。
    V3_1,
    /// 3.2。
    V3_2,
    /// 4.0。
    V4_0,
}

/// 言語版 `(major, minor)` を受けるか判定する。6 つの外は `Err(ArmError::LanguageVersion)`。
///
/// OS に依らない純粋関数で、Metal の列挙値への写像は橋の module が持つ。
pub fn accepted_language_version(major: u32, minor: u32) -> Result<PrimeLanguageVersion, ArmError> {
    match (major, minor) {
        (2, 3) => Ok(PrimeLanguageVersion::V2_3),
        (2, 4) => Ok(PrimeLanguageVersion::V2_4),
        (3, 0) => Ok(PrimeLanguageVersion::V3_0),
        (3, 1) => Ok(PrimeLanguageVersion::V3_1),
        (3, 2) => Ok(PrimeLanguageVersion::V3_2),
        (4, 0) => Ok(PrimeLanguageVersion::V4_0),
        _ => Err(ArmError::LanguageVersion),
    }
}

/// `run_g3prime` の device の前の検査。device にもテキストにも触れない。
///
/// 順は、セルの腕が `G3Prime` か（違えば `WrongArm`）、セルの dtype と入力の種類が合うか
/// （f32 は f32、f16acc は binary16。違えば `InputMismatch`）、検証レイヤーが有効か
/// （無効なら `ValidationLayerRequired`）。入力の長さの誤りは詰め物（[`pad_g3prime`]）が `Layout` で返す。
pub fn check_g3prime_request(cell: GemmGateCell, inputs: &GateInputs) -> Result<(), ArmError> {
    if cell.arm() != GemmGateArm::G3Prime {
        return Err(ArmError::WrongArm);
    }
    match (cell.dtype(), inputs) {
        (GemmInputDtype::F32, GateInputs::F32(_))
        | (GemmInputDtype::F16Acc, GateInputs::Binary16(_)) => {}
        _ => return Err(ArmError::InputMismatch),
    }
    if !ValidationLayerSetting::from_env().is_enabled() {
        return Err(ArmError::ValidationLayerRequired);
    }
    Ok(())
}

/// 詰めた A・B。dtype ごとの型を持つ。
pub enum PaddedInputs {
    /// f32 の A・B。
    F32 {
        /// 詰めた A。
        a: Vec<f32>,
        /// 詰めた B。
        b: Vec<f32>,
    },
    /// binary16 の A・B。
    Binary16 {
        /// 詰めた A。
        a: Vec<u16>,
        /// 詰めた B。
        b: Vec<u16>,
    },
}

/// 1 回の投入に要る、詰めた入力と大きさ。
pub struct PrimeJob {
    layout: GemmLayout,
    padded: PaddedInputs,
    c_len: usize,
    sizes: [u8; 12],
}

impl PrimeJob {
    /// 詰め物の配置。
    pub fn layout(&self) -> &GemmLayout {
        &self.layout
    }

    /// 詰めた入力。
    pub fn padded(&self) -> &PaddedInputs {
        &self.padded
    }

    /// 詰めた C の要素数。
    pub fn c_len(&self) -> usize {
        self.c_len
    }

    /// 大きさの表の引数に渡す、A・B・C のバイト長の `u32` × 3（little-endian）。
    pub fn sizes(&self) -> &[u8; 12] {
        &self.sizes
    }

    /// binary16 の入力か。
    pub fn is_f16(&self) -> bool {
        matches!(self.padded, PaddedInputs::Binary16 { .. })
    }
}

/// 入力を G3 と同じ配置に詰め、A・B・C のバイト長を作る。device に触れない。寸法・バイト長のあふれと長さの誤りは `Err`。
pub fn pad_g3prime(cell: GemmGateCell, inputs: &GateInputs) -> Result<PrimeJob, ArmError> {
    let layout = GemmLayout::for_row(cell.row())?;
    let (padded, elem) = match (cell.dtype(), inputs) {
        (GemmInputDtype::F32, GateInputs::F32(i)) => (
            PaddedInputs::F32 {
                a: pad_a_f32(&layout, i.a())?,
                b: pad_b_f32(&layout, i.b())?,
            },
            size_of::<f32>(),
        ),
        (GemmInputDtype::F16Acc, GateInputs::Binary16(i)) => (
            PaddedInputs::Binary16 {
                a: pad_a_binary16(&layout, i.a())?,
                b: pad_b_binary16(&layout, i.b())?,
            },
            size_of::<u16>(),
        ),
        _ => return Err(ArmError::InputMismatch),
    };
    let c_len = layout.c_padded_len()?;
    let to_bytes = |len: usize, elem: usize| -> Result<u32, ArmError> {
        len.checked_mul(elem)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or(ArmError::Layout(LayoutError::SizeOverflow))
    };
    let (a_len, b_len) = match &padded {
        PaddedInputs::F32 { a, b } => (a.len(), b.len()),
        PaddedInputs::Binary16 { a, b } => (a.len(), b.len()),
    };
    let mut sizes = [0u8; 12];
    sizes[0..4].copy_from_slice(&to_bytes(a_len, elem)?.to_le_bytes());
    sizes[4..8].copy_from_slice(&to_bytes(b_len, elem)?.to_le_bytes());
    sizes[8..12].copy_from_slice(&to_bytes(c_len, size_of::<f32>())?.to_le_bytes());
    Ok(PrimeJob {
        layout,
        padded,
        c_len,
        sizes,
    })
}

#[cfg(target_os = "macos")]
pub(crate) use mac::capture_text;

#[cfg(target_os = "macos")]
mod mac {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use fandhe_silicon_core::hash::sha256_bytes;

    use super::{PrimeLanguageVersion, accepted_language_version};
    use crate::device::{ArmError, block_on, create_g3_pipeline, g3_features, open_device};
    use crate::msl::{Slots, buffer_slots, language_version};
    use crate::{COOPMAT_F16ACC_WGSL, COOPMAT_F32_WGSL, ENTRY_POINT, capture};

    /// pipeline の作成の間に受け取ったテキストと、読み取った言語版・束縛の番号・テキストの sha256。
    pub(crate) struct Captured {
        pub(crate) text: String,
        pub(crate) version: PrimeLanguageVersion,
        pub(crate) slots: Slots,
        pub(crate) sha256: [u8; 32],
    }

    /// G3 と同じ device・WGSL で pipeline を作り、その間の記録からシェーダーのテキストを 1 つ受け取って読む。
    ///
    /// wgpu の検証の失敗は error scope で受けて `Err(Validation)` にする。受け取りは `capture_one` の契約どおり
    /// ちょうど 1 つだけを受け、0 件・2 件以上は `Err`。
    pub(crate) fn capture_text(f16: bool) -> Result<Captured, ArmError> {
        let (device, _queue) = open_device(g3_features(f16))?;
        let uncaptured = Arc::new(AtomicBool::new(false));
        {
            let flag = Arc::clone(&uncaptured);
            device.on_uncaptured_error(Arc::new(move |_e: wgpu::Error| {
                flag.store(true, Ordering::SeqCst);
            }));
        }
        let oom_scope = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let validation_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);

        let wgsl = if f16 {
            COOPMAT_F16ACC_WGSL
        } else {
            COOPMAT_F32_WGSL
        };
        let captured = capture::capture_one(ENTRY_POINT, || create_g3_pipeline(&device, wgsl));
        let _ = device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });
        let validation = block_on(validation_scope.pop());
        let oom = block_on(oom_scope.pop());
        if validation.is_some() || oom.is_some() || uncaptured.load(Ordering::SeqCst) {
            return Err(ArmError::Validation);
        }
        let (_pipeline, text) = captured?;

        let (major, minor) = language_version(&text)?;
        let version = accepted_language_version(major, minor)?;
        let slots = buffer_slots(&text, ENTRY_POINT)?;
        let sha256 = sha256_bytes(text.as_bytes());
        Ok(Captured {
            text,
            version,
            slots,
            sha256,
        })
    }
}
