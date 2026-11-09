//! MPS の行列積で門の run の 1 セルを 1 回投入する橋の module（TASK-1011 のポインタ）。macOS 向けだけ。
//!
//! この crate で `unsafe` を書くのはこのファイルだけ（`src/lib.rs` の `mod mps;` の宣言にだけ lint の許可を付ける）。
//! 呼び出し元は腕の試験（門の枠の `run_selected_from_env` に渡す腕）で、外へ出すのは safe な [`run_matmul`] だけ。
//! 計画（[`crate::MpsPlan`]）の大きさと要素の型を MPS の呼び出しに写し、完了を待ってからだけ出力を読む。
//! クラスの引きとセレクタの呼び出しは `objc2` の `msg_send!` で行い、MPS の framework は
//! `MPSSupportsMTLDevice` の宣言のリンクで読み込む。エラーの中身は読まず、固定の [`MpsError`] にする。

use std::ptr::NonNull;

use fandhe_silicon_core::parity::gemm_gate::{GateInputs, GateOutput};
use objc2::msg_send;
use objc2::rc::{Allocated, Retained, autoreleasepool};
use objc2::runtime::{AnyClass, AnyObject, ProtocolObject};
// コマンドラインのプログラムで `MTLCreateSystemDefaultDevice` が既定の device を返すには、
// Core Graphics がリンクされている必要がある。参照を置いてリンクを確実にする（シンボルは使わない）。
use objc2_core_graphics as _;
use objc2_metal::{
    MTLBuffer, MTLCommandBuffer, MTLCommandBufferStatus, MTLCommandQueue,
    MTLCreateSystemDefaultDevice, MTLDevice, MTLResourceOptions,
};

use crate::{
    ElementType, MpsError, MpsPlan, check_row_bytes, pack_f16, pack_f32, unpack_f16, unpack_f32,
};

/// `MPSDataTypeFloat32`。
const MPS_DATA_TYPE_FLOAT32: u32 = 0x1000_0020;
/// `MPSDataTypeFloat16`。
const MPS_DATA_TYPE_FLOAT16: u32 = 0x1000_0010;

#[link(name = "MetalPerformanceShaders", kind = "framework")]
unsafe extern "C" {
    /// device が MPS に対応するか。
    fn MPSSupportsMTLDevice(device: &AnyObject) -> objc2::runtime::Bool;
}

/// 要素の型を `MPSDataType` の値に写す。
fn data_type(element: ElementType) -> u32 {
    match element {
        ElementType::F32 => MPS_DATA_TYPE_FLOAT32,
        ElementType::F16 => MPS_DATA_TYPE_FLOAT16,
    }
}

/// 計画に従って `C = A × B` を MPS で 1 回だけ投入し、出力（M×N、行優先）を返す。
///
/// 入力の種類が計画の入力の型と合わないときは device に触れず `Err(InputMismatch)`。device・クラス・初期化が
/// MPS に対応しないときは `Err(Unsupported)` で止め、別の経路には回さない。完了の状態が完了でないときは
/// 出力を読まず `Err(Execution)`。
pub fn run_matmul(plan: &MpsPlan, inputs: &GateInputs) -> Result<GateOutput, MpsError> {
    autoreleasepool(|_| run_inner(plan, inputs))
}

fn run_inner(plan: &MpsPlan, inputs: &GateInputs) -> Result<GateOutput, MpsError> {
    let input_matches = matches!(
        (plan.input(), inputs),
        (ElementType::F32, GateInputs::F32(_)) | (ElementType::F16, GateInputs::Binary16(_))
    );
    if !input_matches {
        return Err(MpsError::InputMismatch);
    }
    let (m, n, k) = (plan.m(), plan.n(), plan.k());
    let (in_type, out_type) = (data_type(plan.input()), data_type(plan.output()));

    let device = MTLCreateSystemDefaultDevice().ok_or(MpsError::NoDevice)?;
    let device_object: &AnyObject = AsRef::<AnyObject>::as_ref(&*device);
    // SAFETY: `device_object` は生きている Metal の device で、`MPSSupportsMTLDevice` は
    // `id<MTLDevice>` を 1 つ受けて `BOOL` を返す C の関数。引数は呼び出しの間だけ参照される。
    let supported = unsafe { MPSSupportsMTLDevice(device_object) }.as_bool();
    if !supported {
        return Err(MpsError::Unsupported);
    }

    let descriptor_class = class(c"MPSMatrixDescriptor")?;
    let matrix_class = class(c"MPSMatrix")?;
    let kernel_class = class(c"MPSMatrixMultiplication")?;

    // A は M×K、B は K×N、C は M×N。行の間隔は MPS に尋ね、照らしてからバッファの長さに使う。
    let row_a = row_bytes(descriptor_class, k, in_type);
    let row_b = row_bytes(descriptor_class, n, in_type);
    let row_c = row_bytes(descriptor_class, n, out_type);
    let len_a = check_row_bytes(k, plan.input(), row_a, m).map_err(MpsError::Plan)?;
    let len_b = check_row_bytes(n, plan.input(), row_b, k).map_err(MpsError::Plan)?;
    let len_c = check_row_bytes(n, plan.output(), row_c, m).map_err(MpsError::Plan)?;

    let (bytes_a, bytes_b) = match inputs {
        GateInputs::F32(i) => (
            pack_f32(i.a(), m, k, row_a).map_err(MpsError::Plan)?,
            pack_f32(i.b(), k, n, row_b).map_err(MpsError::Plan)?,
        ),
        GateInputs::Binary16(i) => (
            pack_f16(i.a(), m, k, row_a).map_err(MpsError::Plan)?,
            pack_f16(i.b(), k, n, row_b).map_err(MpsError::Plan)?,
        ),
    };
    let buffer_a = buffer_with_bytes(&device, &bytes_a, len_a)?;
    let buffer_b = buffer_with_bytes(&device, &bytes_b, len_b)?;
    if len_c == 0 {
        return Err(MpsError::AllocationFailed);
    }
    let buffer_c = device
        .newBufferWithLength_options(len_c, MTLResourceOptions::StorageModeShared)
        .ok_or(MpsError::AllocationFailed)?;

    let matrix_a = new_matrix(
        matrix_class,
        descriptor_class,
        &buffer_a,
        (m, k, row_a),
        in_type,
    )?;
    let matrix_b = new_matrix(
        matrix_class,
        descriptor_class,
        &buffer_b,
        (k, n, row_b),
        in_type,
    )?;
    let matrix_c = new_matrix(
        matrix_class,
        descriptor_class,
        &buffer_c,
        (m, n, row_c),
        out_type,
    )?;
    let kernel = new_kernel(kernel_class, &device, (m, n, k))?;

    let queue = device.newCommandQueue().ok_or(MpsError::AllocationFailed)?;
    let command_buffer = queue.commandBuffer().ok_or(MpsError::AllocationFailed)?;
    encode(&kernel, &command_buffer, &matrix_a, &matrix_b, &matrix_c);
    command_buffer.commit();
    command_buffer.waitUntilCompleted();
    if command_buffer.status() != MTLCommandBufferStatus::Completed {
        return Err(MpsError::Execution);
    }

    let raw = read_output(&buffer_c, len_c)?;
    match plan.output() {
        ElementType::F32 => unpack_f32(&raw, m, n, row_c)
            .map(GateOutput::F32)
            .map_err(MpsError::Plan),
        ElementType::F16 => unpack_f16(&raw, m, n, row_c)
            .map(GateOutput::Binary16)
            .map_err(MpsError::Plan),
    }
}

/// 実行時にクラスを引く。無ければ `Unsupported`。
fn class(name: &std::ffi::CStr) -> Result<&'static AnyClass, MpsError> {
    AnyClass::get(name).ok_or(MpsError::Unsupported)
}

/// `+[MPSMatrixDescriptor rowBytesFromColumns:dataType:]` の値を返す。
fn row_bytes(descriptor_class: &AnyClass, columns: usize, data_type: u32) -> usize {
    // SAFETY: `descriptor_class` は実行時から得た `MPSMatrixDescriptor` のクラスで、このクラスメソッドは
    // `NSUInteger`（`usize`）と `MPSDataType`（`u32`）を受けて `NSUInteger` を返す。
    unsafe { msg_send![descriptor_class, rowBytesFromColumns: columns, dataType: data_type] }
}

/// 詰めたバイト列をコピーした共有ストレージのバッファを作る。長さ 0 とバイト列の長さの不一致は拒否する。
fn buffer_with_bytes(
    device: &ProtocolObject<dyn MTLDevice>,
    bytes: &[u8],
    expected: usize,
) -> Result<Retained<ProtocolObject<dyn MTLBuffer>>, MpsError> {
    if bytes.is_empty() || bytes.len() != expected {
        return Err(MpsError::AllocationFailed);
    }
    let pointer = NonNull::from(bytes).cast::<std::ffi::c_void>();
    // SAFETY: `pointer` は `bytes` の先頭を指す非 null のポインタで、`bytes.len()` バイトが読める。
    // `bytes` はこの呼び出しの間だけ生き、Metal は内容をコピーする。
    unsafe {
        device.newBufferWithBytes_length_options(
            pointer,
            bytes.len(),
            MTLResourceOptions::StorageModeShared,
        )
    }
    .ok_or(MpsError::AllocationFailed)
}

/// バッファを包む `MPSMatrix`（`rows × columns`、行の間隔 `row_bytes`）を作る。
fn new_matrix(
    matrix_class: &AnyClass,
    descriptor_class: &AnyClass,
    buffer: &ProtocolObject<dyn MTLBuffer>,
    (rows, columns, row_bytes): (usize, usize, usize),
    data_type: u32,
) -> Result<Retained<AnyObject>, MpsError> {
    // SAFETY: `descriptor_class` は実行時から得た `MPSMatrixDescriptor` のクラスで、このクラスメソッドは
    // `NSUInteger` 3 つと `MPSDataType`（`u32`）を受けて記述子（autorelease）を返す。戻りは nil になりうるので
    // `Option` で受ける。
    let descriptor: Option<Retained<AnyObject>> = unsafe {
        msg_send![
            descriptor_class,
            matrixDescriptorWithRows: rows,
            columns: columns,
            rowBytes: row_bytes,
            dataType: data_type
        ]
    };
    let descriptor = descriptor.ok_or(MpsError::Unsupported)?;
    // SAFETY: `matrix_class` は実行時から得た `MPSMatrix` のクラスで、`alloc` は引数を取らず `Allocated` を返す。
    // `initWithBuffer:descriptor:` は `id<MTLBuffer>` と `MPSMatrixDescriptor` を受ける。`buffer` と `descriptor` は
    // 生きていて、行列が保持する。初期化は失敗で nil になりうるので `Option` で受ける。
    let matrix: Option<Retained<AnyObject>> = unsafe {
        let allocated: Allocated<AnyObject> = msg_send![matrix_class, alloc];
        msg_send![allocated, initWithBuffer: buffer, descriptor: &*descriptor]
    };
    matrix.ok_or(MpsError::Unsupported)
}

/// 転置なし・`alpha = 1`・`beta = 0` の `MPSMatrixMultiplication` を作る。
fn new_kernel(
    kernel_class: &AnyClass,
    device: &ProtocolObject<dyn MTLDevice>,
    (m, n, k): (usize, usize, usize),
) -> Result<Retained<AnyObject>, MpsError> {
    // SAFETY: `kernel_class` は実行時から得た `MPSMatrixMultiplication` のクラスで、`alloc` は引数を取らず
    // `Allocated` を返す。`initWithDevice:transposeLeft:transposeRight:resultRows:resultColumns:interiorColumns:alpha:beta:`
    // は `id<MTLDevice>`、`BOOL` 2 つ、`NSUInteger` 3 つ、`double` 2 つを受ける。`device` は生きている。
    // 初期化は失敗で nil になりうるので `Option` で受ける。
    let kernel: Option<Retained<AnyObject>> = unsafe {
        let allocated: Allocated<AnyObject> = msg_send![kernel_class, alloc];
        msg_send![
            allocated,
            initWithDevice: device,
            transposeLeft: false,
            transposeRight: false,
            resultRows: m,
            resultColumns: n,
            interiorColumns: k,
            alpha: 1.0_f64,
            beta: 0.0_f64
        ]
    };
    kernel.ok_or(MpsError::Unsupported)
}

/// 行列積をコマンドバッファに積む。
fn encode(
    kernel: &AnyObject,
    command_buffer: &ProtocolObject<dyn MTLCommandBuffer>,
    left: &AnyObject,
    right: &AnyObject,
    result: &AnyObject,
) {
    // SAFETY: `kernel` は `MPSMatrixMultiplication` で、`encodeToCommandBuffer:leftMatrix:rightMatrix:resultMatrix:`
    // は `id<MTLCommandBuffer>` と `MPSMatrix` 3 つを受けて何も返さない。引数はどれも生きていて、
    // 積んだ仕事が保持する。行列の大きさは `kernel` の初期化に渡した M・N・K と一致する。
    unsafe {
        let _: () = msg_send![
            kernel,
            encodeToCommandBuffer: command_buffer,
            leftMatrix: left,
            rightMatrix: right,
            resultMatrix: result
        ];
    }
}

/// 完了待ちの後のバッファから、先頭の `len` バイトを読む。バッファの長さを超える読み出しは拒否する。
fn read_output(buffer: &ProtocolObject<dyn MTLBuffer>, len: usize) -> Result<Vec<u8>, MpsError> {
    if len > buffer.length() {
        return Err(MpsError::Execution);
    }
    let source = buffer.contents().cast::<u8>();
    // SAFETY: 呼び出し側は `waitUntilCompleted` と完了の状態の確認の後にだけ呼ぶので、GPU の書き込みは
    // 終わっていて、CPU との読み書きが重ならない。`source` は共有ストレージのバッファの先頭で、
    // 長さは直前の検査で `len` バイト以上と分かる。バッファは `newBufferWithLength` で作られ、
    // 内容は 0 で初期化されているので、全バイトが初期化済みで `u8` はどんなビット列も有効。
    let slice = unsafe { std::slice::from_raw_parts(source.as_ptr(), len) };
    Ok(slice.to_vec())
}
