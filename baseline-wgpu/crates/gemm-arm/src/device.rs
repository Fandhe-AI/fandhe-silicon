//! g3 のセル（cooperative matrix の WGSL）を wgpu の device で 1 つ流す経路。
//!
//! 門の run の枠 `fandhe_silicon_core::parity::gemm_gate` が渡す入力を詰めて GPU へ投入し、
//! 読み戻した出力を M×N に切り出して返す。枠は結果の比べだけを持ち、device にはこの module が触れる。
//! 要る機能が無いアダプタでは device を要求する前に `Err` を返す（別の経路で計算しない）。
//! wgpu の検証の失敗は error scope で受けて `Err` にする（panic しない）。

use std::future::Future;
use std::pin::pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::task::{Context, Poll, Waker};

use fandhe_silicon_core::parity::gemm_gate::{GateInputs, GateOutput};
use fandhe_silicon_core::parity::gemm_inputs::GemmInputDtype;
use fandhe_silicon_core::parity::gemm_population::{GemmGateArm, GemmGateCell};

use crate::layout::{
    GemmLayout, LayoutError, constants_bytes, crop_c, pad_a_binary16, pad_a_f32, pad_b_binary16,
    pad_b_f32, workgroup_count,
};
use crate::{COOPMAT_F16ACC_WGSL, COOPMAT_F32_WGSL, ENTRY_POINT};

/// device の経路の失敗。文言は固定の英語で、wgpu のエラーの中身・アダプタの名前・入力の値を含まない。
#[derive(Debug)]
#[non_exhaustive]
pub enum ArmError {
    /// 使えるアダプタが無い。
    NoAdapter,
    /// アダプタが要る機能を持たない。
    MissingFeatures,
    /// device の要求に失敗した。
    RequestDevice,
    /// 作成または投入が wgpu の検証・確保で拒否された。
    Validation,
    /// 出力の読み戻しに失敗した。
    Readback,
    /// 流せる腕は g3 だけ。
    WrongArm,
    /// セルの dtype と入力の種類が合わない。
    InputMismatch,
    /// 詰め物・切り出しに失敗した。
    Layout(LayoutError),
}

impl std::fmt::Display for ArmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::NoAdapter => "no adapter is available",
            Self::MissingFeatures => "the adapter lacks a required feature",
            Self::RequestDevice => "device request failed",
            Self::Validation => "wgpu rejected a creation or submission",
            Self::Readback => "output readback failed",
            Self::WrongArm => "the cell does not belong to this arm",
            Self::InputMismatch => "the inputs do not match the cell dtype",
            Self::Layout(_) => "padding or cropping failed",
        };
        f.write_str(text)
    }
}

impl std::error::Error for ArmError {}

impl From<LayoutError> for ArmError {
    fn from(e: LayoutError) -> Self {
        Self::Layout(e)
    }
}

/// `Future` を noop の `Waker` で完了まで poll し続けて結果を返す（外部 executor を使わない）。
fn block_on<F: Future>(fut: F) -> F::Output {
    let mut fut = pin!(fut);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(out) = fut.as_mut().poll(&mut cx) {
            return out;
        }
        std::thread::yield_now();
    }
}

/// 値の列を little-endian のバイト列へ並べる。長さのあふれと確保の失敗は `Err`。
fn le_bytes<T: Copy, const N: usize>(
    values: &[T],
    to: impl Fn(T) -> [u8; N],
) -> Result<Vec<u8>, ArmError> {
    let len = values
        .len()
        .checked_mul(N)
        .ok_or(ArmError::Layout(LayoutError::SizeOverflow))?;
    let mut out = Vec::new();
    out.try_reserve_exact(len)
        .map_err(|_| ArmError::Layout(LayoutError::AllocationFailed))?;
    for &v in values {
        out.extend_from_slice(&to(v));
    }
    Ok(out)
}

/// g3 のセルを device で流す。門の run の枠の `run_cell` / `run_selected_from_env` の腕として渡す。
///
/// g3 以外のセル・dtype と合わない入力は device に触れず `Err`。出力は常に f32（M×N、行優先）。
pub fn run_g3(cell: GemmGateCell, inputs: &GateInputs) -> Result<GateOutput, ArmError> {
    if cell.arm() != GemmGateArm::G3 {
        return Err(ArmError::WrongArm);
    }
    let layout = GemmLayout::for_row(cell.row())?;
    let (a_bytes, b_bytes, f16) = match (cell.dtype(), inputs) {
        (GemmInputDtype::F32, GateInputs::F32(i)) => (
            le_bytes(&pad_a_f32(&layout, i.a())?, f32::to_le_bytes)?,
            le_bytes(&pad_b_f32(&layout, i.b())?, f32::to_le_bytes)?,
            false,
        ),
        (GemmInputDtype::F16Acc, GateInputs::Binary16(i)) => (
            le_bytes(&pad_a_binary16(&layout, i.a())?, u16::to_le_bytes)?,
            le_bytes(&pad_b_binary16(&layout, i.b())?, u16::to_le_bytes)?,
            true,
        ),
        _ => return Err(ArmError::InputMismatch),
    };
    let c_len = layout.c_padded_len()?;
    let c_bytes = c_len
        .checked_mul(4)
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(ArmError::Layout(LayoutError::SizeOverflow))?;

    let mut features = wgpu::Features::EXPERIMENTAL_COOPERATIVE_MATRIX;
    if f16 {
        features |= wgpu::Features::SHADER_F16;
    }
    let (device, queue) = open_device(features)?;

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
    let readback = build_and_submit(&device, &queue, &layout, &a_bytes, &b_bytes, c_bytes, wgsl);

    let validation = block_on(validation_scope.pop());
    let oom = block_on(oom_scope.pop());
    if validation.is_some() || oom.is_some() || uncaptured.load(Ordering::SeqCst) {
        return Err(ArmError::Validation);
    }

    let c_padded = read_back(&device, &readback, c_bytes, c_len)?;
    Ok(GateOutput::F32(crop_c(&layout, &c_padded)?))
}

/// アダプタを選び、要る機能が無ければ device を要求する前に失敗する。
fn open_device(features: wgpu::Features) -> Result<(wgpu::Device, wgpu::Queue), ArmError> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .map_err(|_| ArmError::NoAdapter)?;
    if !adapter.features().contains(features) {
        return Err(ArmError::MissingFeatures);
    }
    block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("gemm-arm"),
        required_features: features,
        required_limits: adapter.limits(),
        experimental_features: crate::bridge::experimental_features(),
        memory_hints: wgpu::MemoryHints::Performance,
        trace: wgpu::Trace::Off,
    }))
    .map_err(|_| ArmError::RequestDevice)
}

/// 入力の buffer・パイプライン・束縛を作って 1 回だけ投入し、読み戻し用の buffer を返す。
fn build_and_submit(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &GemmLayout,
    a_bytes: &[u8],
    b_bytes: &[u8],
    c_bytes: u64,
    wgsl: &str,
) -> wgpu::Buffer {
    let buffer = |label: &'static str, size: u64, usage: wgpu::BufferUsages| {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage,
            mapped_at_creation: false,
        })
    };
    let a = buffer(
        "a",
        a_bytes.len() as u64,
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    );
    let b = buffer(
        "b",
        b_bytes.len() as u64,
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    );
    let c = buffer(
        "c",
        c_bytes,
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
    );
    let dims_bytes = constants_bytes(layout);
    let dims = buffer(
        "dims",
        dims_bytes.len() as u64,
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    );
    let readback = buffer(
        "readback",
        c_bytes,
        wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
    );
    queue.write_buffer(&a, 0, a_bytes);
    queue.write_buffer(&b, 0, b_bytes);
    queue.write_buffer(&dims, 0, &dims_bytes);

    let entry = |binding: u32, ty: wgpu::BufferBindingType| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    };
    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("gemm-arm"),
        entries: &[
            entry(0, wgpu::BufferBindingType::Storage { read_only: true }),
            entry(1, wgpu::BufferBindingType::Storage { read_only: true }),
            entry(2, wgpu::BufferBindingType::Storage { read_only: false }),
            entry(3, wgpu::BufferBindingType::Uniform),
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("gemm-arm"),
        bind_group_layouts: &[Some(&bgl)],
        immediate_size: 0,
    });
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("gemm-arm"),
        source: wgpu::ShaderSource::Wgsl(wgsl.into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("gemm-arm"),
        layout: Some(&pipeline_layout),
        module: &module,
        entry_point: Some(ENTRY_POINT),
        compilation_options: Default::default(),
        cache: None,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("gemm-arm"),
        layout: &bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: a.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: b.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: c.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: dims.as_entire_binding(),
            },
        ],
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("gemm-arm"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("gemm-arm"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let [x, y, z] = workgroup_count(layout);
        pass.dispatch_workgroups(x, y, z);
    }
    encoder.copy_buffer_to_buffer(&c, 0, &readback, 0, c_bytes);
    queue.submit([encoder.finish()]);
    readback
}

/// 読み戻し用の buffer を map して f32 の列にする。
fn read_back(
    device: &wgpu::Device,
    readback: &wgpu::Buffer,
    c_bytes: u64,
    c_len: usize,
) -> Result<Vec<f32>, ArmError> {
    let (tx, rx) = mpsc::channel();
    readback
        .slice(..c_bytes)
        .map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })
        .map_err(|_| ArmError::Readback)?;
    match rx.try_recv() {
        Ok(Ok(())) => {}
        _ => return Err(ArmError::Readback),
    }
    let view = readback
        .slice(..c_bytes)
        .get_mapped_range()
        .map_err(|_| ArmError::Readback)?;
    let (chunks, rest) = view.as_chunks::<4>();
    if !rest.is_empty() || chunks.len() != c_len {
        return Err(ArmError::Readback);
    }
    let mut out = Vec::new();
    out.try_reserve_exact(c_len)
        .map_err(|_| ArmError::Layout(LayoutError::AllocationFailed))?;
    out.extend(chunks.iter().map(|c| f32::from_le_bytes(*c)));
    Ok(out)
}
