//! GEMM の門の run の 1 セル（腕 × 行 × dtype）を流す共通の枠（spec TASK-1005 の 5/6）。
//!
//! 各腕の試験（腕の中身は TASK-774・TASK-1008〜TASK-1011 が持つ）から呼ばれる。枠は
//! 入力の生成・腕への投入（1 回）・CPU 参照との比べ（`T1`）・結果の欄の組み立てだけを持ち、
//! device には触れない。採否の集計と機材の組み合わせの判定は範囲の外
//! （後者は各腕の試験が `test_frame` で持つ）。
//!
//! - 1 プロセスで流すセルは環境変数 [`GATE_CELL_ENV`] の識別名で 1 つだけ選ぶ（[`run_selected_from_env`]）。
//!   同じプロセスでの 2 回目の実行は拒否する。この入口を呼ぶ試験は、テストのバイナリにつき 1 つだけにする。
//! - 検証レイヤーの状態は [`DEBUG_LAYER_ENV`]・[`SHADER_VALIDATION_ENV`] が両方ちょうど `"1"` のときだけ
//!   有効として結果に残す。枠は環境変数を設定しない（起動時に設定されている必要がある。設定は手順のスクリプトが持つ）。
//! - 環境変数の値は信頼しない入力として扱い、比べるだけでエラー・表示・記録には出さない。

use std::ffi::OsString;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};

use super::binary16::widen_binary16_slice;
use super::cpu_reference::ReferenceError;
use super::gemm_inputs::{
    GemmGateRow, GemmInputDtype, GemmInputsBinary16, GemmInputsF32, generate_f32, reference_for,
};
use super::gemm_population::GemmGateCell;
use super::tolerance::{CompareError, Comparison, T1, compare_f32};
use crate::hash::sha256_bytes;

/// このプロセスで流すセルの識別名を渡す環境変数の名前。
pub const GATE_CELL_ENV: &str = "FANDHE_SILICON_GEMM_GATE_CELL";
/// 検証レイヤーのうち、API の検査を有効にする環境変数の名前。
pub const DEBUG_LAYER_ENV: &str = "MTL_DEBUG_LAYER";
/// 検証レイヤーのうち、シェーダの検査を有効にする環境変数の名前。
pub const SHADER_VALIDATION_ENV: &str = "MTL_SHADER_VALIDATION";

/// 識別名の値の長さの上限（バイト）。セルの識別名はこれより短い。
const MAX_CELL_VALUE_LEN: usize = 64;

/// 同じプロセスで選択つきの入口が走ったか。
static RAN_IN_PROCESS: AtomicBool = AtomicBool::new(false);

/// 腕へ 1 回だけ渡す入力。`F32` のセルは f32、`F16Acc` のセルは binary16 へ 1 回だけ丸めた値。
#[derive(Debug, Clone, PartialEq)]
pub enum GateInputs {
    /// A・B とも f32。
    F32(GemmInputsF32),
    /// A・B とも binary16 のビット列。
    Binary16(GemmInputsBinary16),
}

/// 腕が返す出力（M×N、行優先）。
#[derive(Debug, Clone, PartialEq)]
pub enum GateOutput {
    /// f32 の出力。
    F32(Vec<f32>),
    /// binary16 のビット列の出力。
    Binary16(Vec<u16>),
}

/// 枠が失敗した理由。`E` は腕のエラー。偽の合格にはしない。
#[derive(Debug)]
pub enum GateError<E> {
    /// 入力・参照の生成に失敗した。
    Reference(ReferenceError),
    /// 列の比べができなかった。
    Compare(CompareError),
    /// 腕が失敗した。
    Arm(E),
    /// 出力の要素数が M×N と違った。
    OutputLength {
        /// M×N。
        expected: usize,
        /// 実際の要素数。
        actual: usize,
    },
    /// `F32` のセルに binary16 の出力が返った。
    OutputKindMismatch,
    /// 環境変数でセルが選ばれていない（腕は呼ばない）。
    NotSelected,
    /// 環境変数の値が不正（空・UTF-8 でない・長すぎる・知らない識別名。値は出さない。腕は呼ばない）。
    InvalidSelection,
    /// このプロセスでは既に 1 回流した（腕は呼ばない）。
    AlreadyRanInProcess,
}

impl<E: fmt::Display> fmt::Display for GateError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(e) => write!(f, "reference failed: {e}"),
            Self::Compare(e) => write!(f, "comparison failed: {e}"),
            Self::Arm(e) => write!(f, "arm failed: {e}"),
            Self::OutputLength { expected, actual } => {
                write!(f, "output has {actual} elements, expected {expected}")
            }
            Self::OutputKindMismatch => write!(f, "binary16 output on an f32 cell"),
            Self::NotSelected => write!(f, "no gate cell is selected"),
            Self::InvalidSelection => write!(f, "the selected gate cell is invalid"),
            Self::AlreadyRanInProcess => write!(f, "a gate cell already ran in this process"),
        }
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for GateError<E> {}

/// 検証レイヤーの状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidationLayerSetting {
    enabled: bool,
}

impl ValidationLayerSetting {
    /// 名前から値を引く関数を受けて作る。2 つとも、ちょうど `"1"` のときだけ有効。
    /// 未設定・`"0"`・UTF-8 でない・別の値はすべて無効。
    pub fn from_lookup<F>(lookup: F) -> Self
    where
        F: Fn(&str) -> Option<OsString>,
    {
        let is_one = |name: &str| lookup(name).is_some_and(|v| v.to_str() == Some("1"));
        Self {
            enabled: is_one(DEBUG_LAYER_ENV) && is_one(SHADER_VALIDATION_ENV),
        }
    }

    /// プロセスの環境変数から作る。`from_lookup` へ `std::env::var_os` を渡すだけ。
    pub fn from_env() -> Self {
        Self::from_lookup(|name| std::env::var_os(name))
    }

    /// 両方の検証が有効か。
    pub fn is_enabled(self) -> bool {
        self.enabled
    }
}

/// 環境変数によるセルの選択の状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateSelection {
    /// 未設定。
    Unset,
    /// 設定はあるが、空・UTF-8 でない・長すぎる・知らない識別名のいずれか。
    Invalid,
    /// 有効なセル。
    Cell(GemmGateCell),
}

impl GateSelection {
    /// 名前から値を引く関数を受けて作る。識別名との完全一致だけを見る。
    pub fn from_lookup<F>(lookup: F) -> Self
    where
        F: Fn(&str) -> Option<OsString>,
    {
        let Some(v) = lookup(GATE_CELL_ENV) else {
            return Self::Unset;
        };
        let Some(s) = v.to_str() else {
            return Self::Invalid;
        };
        if s.len() > MAX_CELL_VALUE_LEN {
            return Self::Invalid;
        }
        match GemmGateCell::from_id(s) {
            Some(c) => Self::Cell(c),
            None => Self::Invalid,
        }
    }
}

/// 1 セルの結果。
#[derive(Debug, Clone, PartialEq)]
pub struct GateCellResult {
    cell: GemmGateCell,
    comparison: Comparison,
    bit_mismatches: usize,
    max_abs_diff: f64,
    output_sha256: [u8; 32],
    validation_layer: bool,
}

impl GateCellResult {
    /// 流したセル。
    pub fn cell(&self) -> GemmGateCell {
        self.cell
    }

    /// `T1` での比べの集計。
    pub fn comparison(&self) -> &Comparison {
        &self.comparison
    }

    /// 丸めた後の参照とビット列が違う要素の数（f32 の出力は f32 のビット、binary16 の出力は u16 のビット）。
    pub fn bit_mismatches(&self) -> usize {
        self.bit_mismatches
    }

    /// 有限な要素どうしの最大の絶対差（f64）。有限な組が無ければ 0。
    pub fn max_abs_diff(&self) -> f64 {
        self.max_abs_diff
    }

    /// 出力の sha256。要素をリトルエンディアンで並べたバイト列（f32 は 4 バイト、binary16 は 2 バイト）の値。
    pub fn output_sha256(&self) -> [u8; 32] {
        self.output_sha256
    }

    /// 検証レイヤー（両方の環境変数が `"1"`）の下で流したか。
    pub fn validation_layer(&self) -> bool {
        self.validation_layer
    }
}

impl fmt::Display for GateCellResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "cell={} compared={} exceeded={} non_finite={} bit_mismatches={} max_abs_diff={:e} sha256=",
            self.cell.id(),
            self.comparison.compared(),
            self.comparison.exceeded(),
            self.comparison.non_finite(),
            self.bit_mismatches,
            self.max_abs_diff,
        )?;
        for b in self.output_sha256 {
            write!(f, "{b:02x}")?;
        }
        write!(
            f,
            " validation={}",
            if self.validation_layer { "on" } else { "off" }
        )
    }
}

/// 出力の種類ごとの、比べる列（f32 へ広げた値）・参照のビット列・出力のバイト列。
struct Prepared {
    actual: Vec<f32>,
    actual_bits: Vec<u32>,
    reference_bits: Vec<u32>,
    reference: Vec<f32>,
    bytes: Vec<u8>,
}

fn to_le_bytes_f32(values: &[f32]) -> Result<Vec<u8>, ReferenceError> {
    let len = values
        .len()
        .checked_mul(4)
        .ok_or(ReferenceError::SizeOverflow)?;
    let mut out = Vec::new();
    out.try_reserve_exact(len)?;
    for v in values {
        out.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    Ok(out)
}

fn to_le_bytes_u16(values: &[u16]) -> Result<Vec<u8>, ReferenceError> {
    let len = values
        .len()
        .checked_mul(2)
        .ok_or(ReferenceError::SizeOverflow)?;
    let mut out = Vec::new();
    out.try_reserve_exact(len)?;
    for v in values {
        out.extend_from_slice(&v.to_le_bytes());
    }
    Ok(out)
}

fn f32_bits(values: &[f32]) -> Result<Vec<u32>, ReferenceError> {
    let mut out = Vec::new();
    out.try_reserve_exact(values.len())?;
    out.extend(values.iter().map(|v| v.to_bits()));
    Ok(out)
}

fn u16_bits(values: &[u16]) -> Result<Vec<u32>, ReferenceError> {
    let mut out = Vec::new();
    out.try_reserve_exact(values.len())?;
    out.extend(values.iter().map(|&v| u32::from(v)));
    Ok(out)
}

/// `cell` を流す。入力と参照はこの呼び出しで 1 回ずつ作り、`submit` は 1 回だけ呼ぶ（`FnOnce`）。
///
/// 出力が `Binary16` の腕は、参照を binary16 へ 1 回だけ丸めた値と比べる。`F16Acc` のセルでは
/// `F32` の出力も受け付け、丸めない参照と比べる。`F32` のセルの `Binary16` の出力は
/// [`GateError::OutputKindMismatch`]。出力の長さが M×N と違えば [`GateError::OutputLength`]。
/// `validation_layer` は結果の欄へそのまま入れる（枠は環境変数を読まない）。
pub fn run_cell<E>(
    cell: GemmGateCell,
    validation_layer: ValidationLayerSetting,
    submit: impl FnOnce(&GateInputs) -> Result<GateOutput, E>,
) -> Result<GateCellResult, GateError<E>> {
    let row: GemmGateRow = cell.row();
    let dtype = cell.dtype();
    let expected = row
        .m()
        .checked_mul(row.n())
        .ok_or(GateError::Reference(ReferenceError::SizeOverflow))?;
    let f32_inputs = generate_f32(row).map_err(GateError::Reference)?;
    let reference = reference_for(row, dtype, &f32_inputs).map_err(GateError::Reference)?;
    let inputs = match dtype {
        GemmInputDtype::F32 => GateInputs::F32(f32_inputs),
        GemmInputDtype::F16Acc => {
            GateInputs::Binary16(f32_inputs.to_binary16().map_err(GateError::Reference)?)
        }
    };
    let output = submit(&inputs).map_err(GateError::Arm)?;
    let actual_len = match &output {
        GateOutput::F32(v) => v.len(),
        GateOutput::Binary16(v) => v.len(),
    };
    if actual_len != expected {
        return Err(GateError::OutputLength {
            expected,
            actual: actual_len,
        });
    }
    let prepared = match &output {
        GateOutput::F32(v) => Prepared {
            actual: v.clone(),
            actual_bits: f32_bits(v).map_err(GateError::Reference)?,
            reference_bits: f32_bits(reference.values()).map_err(GateError::Reference)?,
            reference: reference.values().to_vec(),
            bytes: to_le_bytes_f32(v).map_err(GateError::Reference)?,
        },
        GateOutput::Binary16(v) => {
            if dtype == GemmInputDtype::F32 {
                return Err(GateError::OutputKindMismatch);
            }
            let rounded = reference
                .rounded_to_binary16()
                .map_err(GateError::Reference)?;
            Prepared {
                actual: widen_binary16_slice(v).map_err(GateError::Reference)?,
                actual_bits: u16_bits(v).map_err(GateError::Reference)?,
                reference_bits: u16_bits(&rounded).map_err(GateError::Reference)?,
                reference: widen_binary16_slice(&rounded).map_err(GateError::Reference)?,
                bytes: to_le_bytes_u16(v).map_err(GateError::Reference)?,
            }
        }
    };
    let comparison =
        compare_f32(&prepared.reference, &prepared.actual, T1).map_err(GateError::Compare)?;
    let bit_mismatches = prepared
        .actual_bits
        .iter()
        .zip(&prepared.reference_bits)
        .filter(|(a, r)| a != r)
        .count();
    let max_abs_diff = prepared
        .actual
        .iter()
        .zip(&prepared.reference)
        .filter(|(a, r)| a.is_finite() && r.is_finite())
        .map(|(&a, &r)| (f64::from(a) - f64::from(r)).abs())
        .fold(0.0_f64, f64::max);
    Ok(GateCellResult {
        cell,
        comparison,
        bit_mismatches,
        max_abs_diff,
        output_sha256: sha256_bytes(&prepared.bytes),
        validation_layer: validation_layer.is_enabled(),
    })
}

/// 環境変数で選ばれた 1 セルを、このプロセスで 1 回だけ流す。
///
/// 未設定は [`GateError::NotSelected`]、不正な値は [`GateError::InvalidSelection`] で、どちらも腕を呼ばない
/// （呼び出し側は「流していない」として扱い、暗黙の成功にしない）。2 回目以降は
/// [`GateError::AlreadyRanInProcess`] で腕を呼ばない。この入口を呼ぶ試験は、テストのバイナリにつき 1 つだけにする。
/// `submit` には選ばれたセルも渡す。
pub fn run_selected_from_lookup<E, F>(
    lookup: F,
    submit: impl FnOnce(GemmGateCell, &GateInputs) -> Result<GateOutput, E>,
) -> Result<GateCellResult, GateError<E>>
where
    F: Fn(&str) -> Option<OsString>,
{
    let cell = match GateSelection::from_lookup(&lookup) {
        GateSelection::Unset => return Err(GateError::NotSelected),
        GateSelection::Invalid => return Err(GateError::InvalidSelection),
        GateSelection::Cell(c) => c,
    };
    if RAN_IN_PROCESS.swap(true, Ordering::SeqCst) {
        return Err(GateError::AlreadyRanInProcess);
    }
    let validation = ValidationLayerSetting::from_lookup(&lookup);
    run_cell(cell, validation, |inputs| submit(cell, inputs))
}

/// [`run_selected_from_lookup`] へ `std::env::var_os` を渡すだけの入口。
pub fn run_selected_from_env<E>(
    submit: impl FnOnce(GemmGateCell, &GateInputs) -> Result<GateOutput, E>,
) -> Result<GateCellResult, GateError<E>> {
    run_selected_from_lookup(|name| std::env::var_os(name), submit)
}
