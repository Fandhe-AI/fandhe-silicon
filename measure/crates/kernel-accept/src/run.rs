//! 契約の公開 API で段ごとに投入して待ち、出力を読む口（TASK-485 のポインタ）。
//!
//! 1 段は「パイプラインを作る → 入力と出力のバッファを確保して入力を書く → 記録先を作って dispatch を 1 件積む
//! → 投入して待つ → 完了を確かめる → 出力を読む」の順で、段が終わったらバッファを解放する。
//! 呼び出し元は後続の段の device の試験。入口名とソースは照合済みの [`FlowKernels`] からだけ取る。
//! 製品の Metal の背後実装は、記録のある投入を単体試験のビルド以外では `Unsupported` で拒む。
//! そのため依存 crate としてこの口を呼ぶと、段の投入は固定の種類の失敗で返り、出力は得られない。
//! 拒否の解除は契約側の判断で、この crate は既存の拒否を迂回しない。
//! 投入の前に、カーネルの役割が行・段と合うことを確かめる（不一致は device に触る前に拒む）。
//! Metal に触れるのは macOS の cfg の内側だけで、エラーは固定の種類の文字列に写し、
//! 値・入口名・契約の文言を含めない。

use std::fmt;

use crate::flow::FlowError;

/// 失敗した段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// GEMM の段。
    Gemm,
    /// 加算の段。
    Add,
    /// Relu の段。
    Relu,
}

/// 段の中で失敗した手順。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    /// device の取得。
    Request,
    /// compute の口が無い。
    NoCompute,
    /// パイプラインの作成。
    Pipeline,
    /// バッファの確保。
    Allocate,
    /// 入力の書き込み。
    Upload,
    /// dispatch の記録。
    Dispatch,
    /// 投入。
    Submit,
    /// 完了の待ち。
    Wait,
    /// 待った後に完了になっていない。
    NotComplete,
    /// 出力の読み出し。
    Download,
    /// 値の変換・大きさの計算。
    Convert,
}

/// 口の失敗。`kind` は契約のエラーを写した固定の種類の文字列か、手順の固定の語。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunError {
    /// 失敗した段。段の外（device の取得）は `None`。
    pub stage: Option<Stage>,
    /// 失敗した手順。
    pub step: StepKind,
    /// 固定の種類の文字列。
    pub kind: &'static str,
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "run failed: stage={:?} step={:?} kind={}",
            self.stage, self.step, self.kind
        )
    }
}

impl std::error::Error for RunError {}

impl RunError {
    /// 部品の失敗を、段と固定の種類つきの失敗に写す。
    pub fn from_flow(stage: Stage, e: FlowError) -> Self {
        let kind = match e {
            FlowError::SizeOverflow => "size-overflow",
            FlowError::LengthMismatch => "length-mismatch",
            FlowError::AllocationFailed => "allocation-failed",
            FlowError::RoleMismatch(_) => "role-mismatch",
            _ => "invalid-flow",
        };
        Self {
            stage: Some(stage),
            step: StepKind::Convert,
            kind,
        }
    }
}

/// 3 段の出力。
#[derive(Debug, Clone, PartialEq)]
pub struct FlowOutputs {
    /// GEMM の出力。
    pub gemm: Vec<f32>,
    /// 加算の出力。
    pub add: Vec<f32>,
    /// Relu の出力。
    pub relu: Vec<f32>,
}

#[cfg(target_os = "macos")]
pub use device::{open_device, run_flow, run_gemm};

#[cfg(target_os = "macos")]
mod device {
    use std::time::Duration;

    use fandhe_silicon_contract::backend_metal::MetalDeviceProvider;
    use fandhe_silicon_core::capability::BackendKind;
    use fandhe_silicon_core::device::{
        Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
    };
    use fandhe_silicon_core::error::CompletionState;
    use fandhe_silicon_core::execution::{
        Binding, BindingLayout, ConstantBinding, PipelineInput, ThreadgroupMemory,
    };
    use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};
    use fandhe_silicon_core::parity::gemm_inputs::{GemmGateRow, GemmInputsF32};

    use super::{FlowOutputs, RunError, Stage, StepKind};
    use crate::accept::error_kind;
    use crate::flow::{
        DispatchShape, FlowError, FlowKernel, FlowKernels, Role, add_shape,
        check_gemm_input_lengths, check_gemm_role, check_role, f32_from_ne_bytes, f32_ne_bytes,
        gemm_shape, relu_shape,
    };

    fn err(stage: Stage, step: StepKind, kind: &'static str) -> RunError {
        RunError {
            stage: Some(stage),
            step,
            kind,
        }
    }

    /// Metal の device を 1 つ作る（ComputeOnly・ProductIndependent）。
    pub fn open_device() -> Result<Box<dyn Device>, RunError> {
        MetalDeviceProvider::request(DeviceRequest {
            backend: BackendKind::Metal,
            profile: Profile::ComputeOnly,
            ownership: DeviceOwnership::ProductIndependent,
        })
        .map_err(|e| RunError {
            stage: None,
            step: StepKind::Request,
            kind: error_kind(&e),
        })
    }

    fn pipeline_input<'a>(kernel: &'a FlowKernel, shape: &'a DispatchShape) -> PipelineInput<'a> {
        if kernel.role() == Role::SpecializedGemm {
            PipelineInput::MslSpecialized {
                source: kernel.source(),
                entry_point: kernel.entry(),
                function_constants: &shape.function_constants,
            }
        } else {
            PipelineInput::Msl {
                source: kernel.source(),
                entry_point: kernel.entry(),
            }
        }
    }

    fn alloc_f32(
        device: &dyn Device,
        stage: Stage,
        len: usize,
        held: &mut Vec<BufferHandle>,
    ) -> Result<usize, RunError> {
        let bytes = len
            .checked_mul(4)
            .and_then(|b| u64::try_from(b).ok())
            .ok_or_else(|| err(stage, StepKind::Convert, "size-overflow"))?;
        let usage = BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD;
        let buf = device
            .memory()
            .allocate(bytes, usage)
            .map_err(|e| err(stage, StepKind::Allocate, error_kind(&e)))?;
        held.push(buf);
        Ok(held.len() - 1)
    }

    /// 1 段を流す。確保したバッファは成否に関わらず解放する。
    fn run_stage(
        device: &dyn Device,
        stage: Stage,
        kernel: &FlowKernel,
        shape: &DispatchShape,
        inputs: &[&[f32]],
        out_len: usize,
    ) -> Result<Vec<f32>, RunError> {
        let mut held: Vec<BufferHandle> = Vec::new();
        let result = stage_body(device, stage, kernel, shape, inputs, out_len, &mut held);
        for buf in held {
            device.memory().free(buf);
        }
        result
    }

    fn stage_body(
        device: &dyn Device,
        stage: Stage,
        kernel: &FlowKernel,
        shape: &DispatchShape,
        inputs: &[&[f32]],
        out_len: usize,
        held: &mut Vec<BufferHandle>,
    ) -> Result<Vec<f32>, RunError> {
        let compute = device
            .compute()
            .ok_or_else(|| err(stage, StepKind::NoCompute, "no-compute"))?;
        let pipeline = compute
            .create_pipeline(
                pipeline_input(kernel, shape),
                &BindingLayout::new(shape.layout.clone()),
            )
            .map_err(|e| err(stage, StepKind::Pipeline, error_kind(&e)))?;

        let mut ids = Vec::new();
        for data in inputs {
            let id = alloc_f32(device, stage, data.len(), held)?;
            device
                .memory()
                .upload(&held[id], 0, &f32_ne_bytes(data))
                .map_err(|e| err(stage, StepKind::Upload, error_kind(&e)))?;
            ids.push(id);
        }
        let out_id = alloc_f32(device, stage, out_len, held)?;
        ids.push(out_id);

        let bindings: Vec<Binding<'_>> = ids
            .iter()
            .enumerate()
            .map(|(slot, &id)| Binding::new(slot as u32, &held[id], 0))
            .collect();
        let consts: Vec<ConstantBinding<'_>> = shape
            .constants
            .iter()
            .map(|(slot, bytes)| ConstantBinding::new(*slot, bytes))
            .collect();
        let tg: Vec<ThreadgroupMemory> = shape
            .threadgroup_memory
            .iter()
            .map(|&(index, bytes)| ThreadgroupMemory::new(index, bytes))
            .collect();

        let queue = device.queue();
        let mut encoder = queue.create_encoder();
        queue
            .dispatch_ex(
                &mut encoder,
                &pipeline,
                &bindings,
                &consts,
                shape.groups,
                shape.threads,
                &tg,
            )
            .map_err(|e| err(stage, StepKind::Dispatch, error_kind(&e)))?;
        let token = queue
            .submit(encoder, &[])
            .map_err(|e| err(stage, StepKind::Submit, error_kind(&e)))?;
        queue
            .wait_blocking(token, None::<Duration>)
            .map_err(|e| err(stage, StepKind::Wait, error_kind(&e)))?;
        if queue.poll(token) != CompletionState::Complete {
            return Err(err(stage, StepKind::NotComplete, "not-complete"));
        }

        let out_bytes = out_len
            .checked_mul(4)
            .and_then(|b| u64::try_from(b).ok())
            .ok_or_else(|| err(stage, StepKind::Convert, "size-overflow"))?;
        let bytes = device
            .memory()
            .download_blocking(&held[out_id], 0, out_bytes)
            .map_err(|e| err(stage, StepKind::Download, error_kind(&e)))?;
        f32_from_ne_bytes(&bytes).map_err(|e| RunError::from_flow(stage, e))
    }

    /// GEMM の段だけを 1 回の投入で流し、出力を返す。
    pub fn run_gemm(
        device: &dyn Device,
        row: GemmGateRow,
        kernel: &FlowKernel,
        inputs: &GemmInputsF32,
    ) -> Result<Vec<f32>, RunError> {
        check_gemm_role(row, kernel).map_err(|e| RunError::from_flow(Stage::Gemm, e))?;
        check_gemm_input_lengths(row, inputs).map_err(|e| RunError::from_flow(Stage::Gemm, e))?;
        let out_len = row
            .m()
            .checked_mul(row.n())
            .ok_or_else(|| err(Stage::Gemm, StepKind::Convert, "size-overflow"))?;
        let shape = gemm_shape(row).map_err(|e| RunError::from_flow(Stage::Gemm, e))?;
        run_stage(
            device,
            Stage::Gemm,
            kernel,
            &shape,
            &[inputs.a(), inputs.b()],
            out_len,
        )
    }

    /// GEMM → 加算 → Relu を 3 回の投入で流す。前の段の出力を次の段の入力にする。
    pub fn run_flow(
        device: &dyn Device,
        row: GemmGateRow,
        kernels: &FlowKernels,
        inputs: &GemmInputsF32,
        addend: &[f32],
    ) -> Result<FlowOutputs, RunError> {
        // 先頭の段を流す前に、後続の段の役割も確かめる（途中で投入済みにしない）。
        check_gemm_role(row, &kernels.gemm).map_err(|e| RunError::from_flow(Stage::Gemm, e))?;
        check_role(&kernels.add, Role::Add).map_err(|e| RunError::from_flow(Stage::Add, e))?;
        check_role(&kernels.relu, Role::Relu).map_err(|e| RunError::from_flow(Stage::Relu, e))?;
        let gemm = run_gemm(device, row, &kernels.gemm, inputs)?;
        let len = gemm.len();
        if addend.len() != len {
            return Err(RunError::from_flow(Stage::Add, FlowError::LengthMismatch));
        }
        let add_s = add_shape(len).map_err(|e| RunError::from_flow(Stage::Add, e))?;
        let add = run_stage(
            device,
            Stage::Add,
            &kernels.add,
            &add_s,
            &[&gemm, addend],
            len,
        )?;
        let relu_s = relu_shape(len).map_err(|e| RunError::from_flow(Stage::Relu, e))?;
        let relu = run_stage(device, Stage::Relu, &kernels.relu, &relu_s, &[&add], len)?;
        Ok(FlowOutputs { gemm, add, relu })
    }
}
