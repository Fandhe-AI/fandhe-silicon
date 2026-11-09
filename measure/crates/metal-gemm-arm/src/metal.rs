//! Metal を直接呼んで GEMM の門の 1 セルを 1 回投入する橋の module（TASK-1008 のポインタ）。macOS 向けだけ。
//!
//! この crate で `unsafe` を書くのはこのファイルだけ（`src/lib.rs` の `mod metal;` の宣言にだけ lint の許可を付ける）。
//! 呼び出し元は腕の試験（門の枠の `run_cell` に渡す腕）で、
//! 外へ出すのは safe な [`run_g6`] と [`run_g7`] だけ。起動の表（[`crate::launch`]）の値を Metal の呼び出しに写し、
//! 完了を待ってからだけ出力を読む。`objc2-foundation` は使わず、`NSString` と 2 つの生成の呼び出しは
//! `msg_send!` で扱う。エラーの中身は読まず、固定の [`ArmError`] にする。

use std::ffi::c_void;
use std::path::Path;
use std::ptr::{self, NonNull};

use fandhe_silicon_core::parity::gemm_gate::{GateInputs, GateOutput};
use fandhe_silicon_core::parity::gemm_population::{GemmGateArm, GemmGateCell};
use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyClass, AnyObject, NSObjectProtocol, ProtocolObject};
use objc2::{msg_send, sel};
// コマンドラインのプログラムで `MTLCreateSystemDefaultDevice` が既定の device を返すには、
// Core Graphics がリンクされている必要がある。参照を置いてリンクを確実にする（シンボルは使わない）。
use objc2_core_graphics as _;
use objc2_metal::{
    MTLBuffer, MTLCommandBuffer, MTLCommandBufferStatus, MTLCommandEncoder, MTLCommandQueue,
    MTLCompileOptions, MTLComputeCommandEncoder, MTLComputePipelineState,
    MTLCreateSystemDefaultDevice, MTLDataType, MTLDevice, MTLFunction, MTLFunctionConstantValues,
    MTLLibrary, MTLMathFloatingPointFunctions, MTLMathMode, MTLResourceOptions, MTLSize,
};

use crate::coopmat::{COOPMAT_ENTRY, COOPMAT_THREADS, coopmat_launch_for};
use crate::launch::{
    BufferLayout, ConstantValue, Launch, OutputKind, crop_matrix, launch_for, pad_matrix, pad8_dim,
};
use crate::{ArmError, MetallibFacts, PipelineFacts};

/// UTF-8 を表す `NSStringEncoding` の値。
const UTF8_ENCODING: usize = 4;

/// `Dims`（buffer 3）。値は詰めた後の M・N・K。
#[repr(C)]
#[derive(Clone, Copy)]
struct Dims {
    m: u32,
    n: u32,
    k: u32,
}

/// `GemmStrides`（f32 の buffer 4）。
#[repr(C)]
#[derive(Clone, Copy)]
struct Strides {
    lda: u32,
    ldb: u32,
    trans_a: u32,
    trans_b: u32,
}

/// `TileClassRegion`（f32 の buffer 5）。
#[repr(C)]
#[derive(Clone, Copy)]
struct Region {
    row_off: u32,
    col_off: u32,
    rows: u32,
    cols: u32,
}

/// `SplitKParams`（f32 の buffer 6）。
#[repr(C)]
#[derive(Clone, Copy)]
struct SplitK {
    partitions: u32,
    k_per_partition: u32,
    reserved0: u32,
    reserved1: u32,
}

/// `CoopmatDims`（cooperative matrix の kernel の buffer 3）。並びは m・k・n で、値は実効の M・K・N。
#[repr(C)]
#[derive(Clone, Copy)]
struct CoopmatDims {
    m: u32,
    k: u32,
    n: u32,
}

/// 門の `G6` のセルを 1 回だけ投入し、出力（M×N、行優先）と pipeline の事実を返す。
///
/// `source` は投入の前に [`crate::source::verify_source`] で固定のハッシュと照らし、
/// 一致しないときは何もコンパイルせず `Err(ArmError::SourceMismatch)` を返す（バッファの束縛の
/// SAFETY が前提とする入口の型・アクセス範囲を、呼び出し側の約束に頼らず保つため）。腕・dtype・入力の長さが
/// セルと食い違うとき、設定の関数が無いとき、スレッド数が pipeline の上限を超えるとき、
/// 完了の状態が完了でないときは、device に出力を読まず `Err` を返す。
pub fn run_g6(
    cell: GemmGateCell,
    inputs: &GateInputs,
    source: &str,
) -> Result<(GateOutput, PipelineFacts), ArmError> {
    if cell.arm() != GemmGateArm::G6 {
        return Err(ArmError::UnsupportedCell);
    }
    let source =
        crate::source::verify_source(source.as_bytes()).map_err(|_| ArmError::SourceMismatch)?;
    let launch = launch_for(cell).ok_or(ArmError::UnsupportedCell)?;
    let (m, n, k) = (cell.row().m(), cell.row().n(), cell.row().k());
    if pad8_dim(m)? != launch.eff_m || pad8_dim(n)? != launch.eff_n || pad8_dim(k)? != launch.eff_k
    {
        return Err(ArmError::LengthMismatch);
    }
    match (launch.output, inputs) {
        (OutputKind::F32, GateInputs::F32(i)) => {
            let (out, facts) = dispatch(&launch, source, (m, n, k), i.a(), i.b(), 0.0_f32)?;
            Ok((GateOutput::F32(out), facts))
        }
        (OutputKind::Binary16, GateInputs::Binary16(i)) => {
            let (out, facts) = dispatch(&launch, source, (m, n, k), i.a(), i.b(), 0_u16)?;
            Ok((GateOutput::Binary16(out), facts))
        }
        _ => Err(ArmError::DtypeMismatch),
    }
}

/// 門の `G7` のセルを、MSL から作った metallib で 1 回だけ投入し、出力（M×N、行優先）と事実を返す。
///
/// 腕・起動の形・dtype・入力の長さの検査は offline compiler も device も呼ぶ前に済ませ、食い違うときは `Err` を返す。
/// 成果物（パス・バイト列）は外から受けず、腕の中で作った metallib だけを読む。作業のディレクトリは
/// 出力を読み終えるまで保持し、どの出口でも消える。
pub fn run_g7(
    cell: GemmGateCell,
    inputs: &GateInputs,
) -> Result<(GateOutput, MetallibFacts), ArmError> {
    if cell.arm() != GemmGateArm::G7 {
        return Err(ArmError::UnsupportedCell);
    }
    let launch = coopmat_launch_for(cell).ok_or(ArmError::UnsupportedCell)?;
    let GateInputs::F32(i) = inputs else {
        return Err(ArmError::DtypeMismatch);
    };
    let (m, n, k) = (cell.row().m(), cell.row().n(), cell.row().k());
    let a_padded = pad_matrix(i.a(), m, k, launch.eff_m, launch.eff_k, 0.0_f32)?;
    let b_padded = pad_matrix(i.b(), k, n, launch.eff_k, launch.eff_n, 0.0_f32)?;
    let c_elems = launch
        .eff_m
        .checked_mul(launch.eff_n)
        .ok_or(ArmError::Overflow)?;

    // 作業のディレクトリは出力を読み終えるまで保持する。
    let built = crate::metallib::build_metallib()?;

    let device = MTLCreateSystemDefaultDevice().ok_or(ArmError::NoDevice)?;
    let url = file_url(built.metallib())?;
    let library = new_library_from_url(&device, &url)?;
    let name_string = make_nsstring(COOPMAT_ENTRY)?;
    let function = new_function_by_name(&library, &name_string)?;
    let pipeline = device
        .newComputePipelineStateWithFunction_error(&function)
        .map_err(|_| ArmError::PipelineFailed)?;
    let max_threads = pipeline.maxTotalThreadsPerThreadgroup();
    if COOPMAT_THREADS > max_threads {
        return Err(ArmError::ThreadsExceeded);
    }

    let a_buffer = new_shared_buffer(&device, a_padded.len(), size_of::<f32>())?;
    let b_buffer = new_shared_buffer(&device, b_padded.len(), size_of::<f32>())?;
    let c_buffer = new_shared_buffer(&device, c_elems, size_of::<f32>())?;
    write_buffer(&a_buffer, &a_padded)?;
    write_buffer(&b_buffer, &b_padded)?;
    zero_buffer(&c_buffer)?;

    let queue = device.newCommandQueue().ok_or(ArmError::AllocationFailed)?;
    let command_buffer = queue.commandBuffer().ok_or(ArmError::SubmitFailed)?;
    let encoder = command_buffer
        .computeCommandEncoder()
        .ok_or(ArmError::SubmitFailed)?;
    encoder.setComputePipelineState(&pipeline);
    bind_buffers(&encoder, &[&a_buffer, &b_buffer, &c_buffer]);
    let dims = CoopmatDims {
        m: to_u32(launch.eff_m)?,
        k: to_u32(launch.eff_k)?,
        n: to_u32(launch.eff_n)?,
    };
    set_bytes(&encoder, &dims, 3);
    encoder.dispatchThreadgroups_threadsPerThreadgroup(
        MTLSize {
            width: launch.groups_width,
            height: launch.groups_height,
            depth: 1,
        },
        MTLSize {
            width: COOPMAT_THREADS,
            height: 1,
            depth: 1,
        },
    );
    encoder.endEncoding();
    command_buffer.commit();
    command_buffer.waitUntilCompleted();
    if command_buffer.status() != MTLCommandBufferStatus::Completed {
        return Err(ArmError::SubmitFailed);
    }

    let padded_out = read_buffer::<f32>(&c_buffer, c_elems)?;
    let out = crop_matrix(&padded_out, launch.eff_m, launch.eff_n, m, n)?;
    let facts = MetallibFacts {
        max_total_threads_per_threadgroup: max_threads,
        metallib_sha256: built.sha256(),
    };
    drop(built);
    Ok((GateOutput::F32(out), facts))
}

/// 詰め物 → コンパイル → pipeline → バッファ → 投入 → 完了待ち → 切り出し、を 1 回だけ行う。
fn dispatch<T: Copy>(
    launch: &Launch,
    source: &str,
    (m, n, k): (usize, usize, usize),
    a: &[T],
    b: &[T],
    zero: T,
) -> Result<(Vec<T>, PipelineFacts), ArmError> {
    let a_padded = pad_matrix(a, m, k, launch.eff_m, launch.eff_k, zero)?;
    let b_padded = pad_matrix(b, k, n, launch.eff_k, launch.eff_n, zero)?;
    let c_elems = launch
        .eff_m
        .checked_mul(launch.eff_n)
        .ok_or(ArmError::Overflow)?;

    let device = MTLCreateSystemDefaultDevice().ok_or(ArmError::NoDevice)?;
    let options = compile_options()?;
    let source_string = make_nsstring(source)?;
    let name_string = make_nsstring(launch.entry.name())?;
    let library = new_library(&device, &source_string, &options)?;
    let values = MTLFunctionConstantValues::new();
    for (index, value) in launch.constants {
        let index = usize::try_from(index).map_err(|_| ArmError::Overflow)?;
        match value {
            ConstantValue::UInt(v) => set_constant(&values, &v, MTLDataType::UInt, index),
            ConstantValue::Bool(v) => set_constant(&values, &v, MTLDataType::Bool, index),
        }
    }
    let function = new_function(&library, &name_string, &values)?;
    let pipeline = device
        .newComputePipelineStateWithFunction_error(&function)
        .map_err(|_| ArmError::PipelineFailed)?;
    let max_threads = pipeline.maxTotalThreadsPerThreadgroup();
    if launch.threads > max_threads {
        return Err(ArmError::ThreadsExceeded);
    }

    let a_buffer = new_shared_buffer(&device, a_padded.len(), size_of::<T>())?;
    let b_buffer = new_shared_buffer(&device, b_padded.len(), size_of::<T>())?;
    let c_buffer = new_shared_buffer(&device, c_elems, size_of::<T>())?;
    write_buffer(&a_buffer, &a_padded)?;
    write_buffer(&b_buffer, &b_padded)?;
    zero_buffer(&c_buffer)?;

    let queue = device.newCommandQueue().ok_or(ArmError::AllocationFailed)?;
    let command_buffer = queue.commandBuffer().ok_or(ArmError::SubmitFailed)?;
    let encoder = command_buffer
        .computeCommandEncoder()
        .ok_or(ArmError::SubmitFailed)?;
    encoder.setComputePipelineState(&pipeline);
    bind_buffers(&encoder, &[&a_buffer, &b_buffer, &c_buffer]);
    let dims = Dims {
        m: to_u32(launch.eff_m)?,
        n: to_u32(launch.eff_n)?,
        k: to_u32(launch.eff_k)?,
    };
    set_bytes(&encoder, &dims, 3);
    if launch.buffers == BufferLayout::F32Seven {
        let strides = Strides {
            lda: dims.k,
            ldb: dims.n,
            trans_a: 0,
            trans_b: 0,
        };
        let region = Region {
            row_off: 0,
            col_off: 0,
            rows: to_u32(launch.groups_height)?,
            cols: to_u32(launch.groups_width)?,
        };
        let split_k = SplitK {
            partitions: 1,
            k_per_partition: dims.k,
            reserved0: 0,
            reserved1: 0,
        };
        set_bytes(&encoder, &strides, 4);
        set_bytes(&encoder, &region, 5);
        set_bytes(&encoder, &split_k, 6);
    }
    set_threadgroup_memory(&encoder, launch.threadgroup_memory);
    encoder.dispatchThreadgroups_threadsPerThreadgroup(
        MTLSize {
            width: launch.groups_width,
            height: launch.groups_height,
            depth: 1,
        },
        MTLSize {
            width: launch.threads,
            height: 1,
            depth: 1,
        },
    );
    encoder.endEncoding();
    command_buffer.commit();
    command_buffer.waitUntilCompleted();
    if command_buffer.status() != MTLCommandBufferStatus::Completed {
        return Err(ArmError::SubmitFailed);
    }

    let padded_out = read_buffer::<T>(&c_buffer, c_elems)?;
    let out = crop_matrix(&padded_out, launch.eff_m, launch.eff_n, m, n)?;
    let facts = PipelineFacts {
        max_total_threads_per_threadgroup: max_threads,
        threadgroup_memory_length: launch.threadgroup_memory,
    };
    Ok((out, facts))
}

fn to_u32(x: usize) -> Result<u32, ArmError> {
    u32::try_from(x).map_err(|_| ArmError::Overflow)
}

/// 数学モードを `Safe`、関数の精度を `Precise` にしたコンパイルのオプションを作る。
///
/// 設定の関数を持たない環境では、設定を呼ぶ前に `Err` を返し、既定のオプションで黙って作らない。
/// 言語版は設定しない。
fn compile_options() -> Result<Retained<MTLCompileOptions>, ArmError> {
    let options = MTLCompileOptions::new();
    if !options.respondsToSelector(sel!(setMathMode:))
        || !options.respondsToSelector(sel!(setMathFloatingPointFunctions:))
    {
        return Err(ArmError::MissingCompileOption);
    }
    options.setMathMode(MTLMathMode::Safe);
    options.setMathFloatingPointFunctions(MTLMathFloatingPointFunctions::Precise);
    Ok(options)
}

/// UTF-8 の文字列から `NSString` を作る。
fn make_nsstring(text: &str) -> Result<Retained<AnyObject>, ArmError> {
    let class = AnyClass::get(c"NSString").ok_or(ArmError::AllocationFailed)?;
    let bytes = text.as_ptr().cast::<c_void>();
    // SAFETY: `class` は実行時から得た `NSString` のクラスで、`alloc` は引数を取らず `Allocated` を返す。
    // `initWithBytes:length:encoding:` には、`text` の先頭を指す `bytes` と、その実際の長さ `text.len()`、
    // UTF-8 を表す定数 4 を渡す。`text` は `&str` なので有効な UTF-8 で、この呼び出しの間だけ生き、
    // 初期化は内容をコピーする。戻りは失敗で nil になりうるので `Option` で受ける。
    let object: Option<Retained<AnyObject>> = unsafe {
        let allocated: Allocated<AnyObject> = msg_send![class, alloc];
        msg_send![allocated, initWithBytes: bytes, length: text.len(), encoding: UTF8_ENCODING]
    };
    object.ok_or(ArmError::AllocationFailed)
}

/// ソースの文字列から library を作る。失敗の中身は読まず固定の `Err` にする。
fn new_library(
    device: &ProtocolObject<dyn MTLDevice>,
    source: &AnyObject,
    options: &MTLCompileOptions,
) -> Result<Retained<ProtocolObject<dyn MTLLibrary>>, ArmError> {
    let mut error: *mut AnyObject = ptr::null_mut();
    // SAFETY: `device` は生きている Metal の device で、`newLibraryWithSource:options:error:` を受ける。
    // `source` は `make_nsstring` が作った `NSString`、`options` は生きている `MTLCompileOptions`で、
    // どれもこの呼び出しの間だけ参照される。最後の引数は `NSError **` に当たる、局所の
    // `*mut AnyObject` を指す書き込み先で、呼び出しの間だけ生きる。戻りは `new` 系の所有で、
    // 失敗で nil になりうるので `Option` で受ける。
    let library: Option<Retained<ProtocolObject<dyn MTLLibrary>>> = unsafe {
        msg_send![device, newLibraryWithSource: source, options: options, error: &mut error]
    };
    library.ok_or(ArmError::CompileFailed)
}

/// library から、function constant を当てた入口の関数を作る。失敗の中身は読まない。
fn new_function(
    library: &ProtocolObject<dyn MTLLibrary>,
    name: &AnyObject,
    values: &MTLFunctionConstantValues,
) -> Result<Retained<ProtocolObject<dyn MTLFunction>>, ArmError> {
    let mut error: *mut AnyObject = ptr::null_mut();
    // SAFETY: `library` は生きている library で、`newFunctionWithName:constantValues:error:` を受ける。
    // `name` は `make_nsstring` が作った `NSString`、`values` は生きている `MTLFunctionConstantValues`で、
    // どれもこの呼び出しの間だけ参照される。最後の引数は `NSError **` に当たる、局所の
    // `*mut AnyObject` を指す書き込み先で、呼び出しの間だけ生きる。戻りは `new` 系の所有で、
    // 失敗で nil になりうるので `Option` で受ける。
    let function: Option<Retained<ProtocolObject<dyn MTLFunction>>> = unsafe {
        msg_send![library, newFunctionWithName: name, constantValues: values, error: &mut error]
    };
    function.ok_or(ArmError::FunctionFailed)
}

/// ファイルのパスから `NSURL` を作る。
fn file_url(path: &Path) -> Result<Retained<AnyObject>, ArmError> {
    let text = path.to_str().ok_or(ArmError::WorkDirFailed)?;
    let path_string = make_nsstring(text)?;
    let class = AnyClass::get(c"NSURL").ok_or(ArmError::AllocationFailed)?;
    // SAFETY: `class` は実行時から得た `NSURL` のクラスで、`fileURLWithPath:` はクラスメソッドとして
    // `NSString` を 1 つ取る。`path_string` は `make_nsstring` が作った生きている `NSString` で、
    // 呼び出しの間だけ参照される。戻りは自動解放のオブジェクトで、`Retained` が保持を取る。
    // 失敗で nil になりうるので `Option` で受ける。
    let url: Option<Retained<AnyObject>> =
        unsafe { msg_send![class, fileURLWithPath: &*path_string] };
    url.ok_or(ArmError::WorkDirFailed)
}

/// metallib のファイルの URL から library を作る。失敗の中身は読まず固定の `Err` にする。
fn new_library_from_url(
    device: &ProtocolObject<dyn MTLDevice>,
    url: &AnyObject,
) -> Result<Retained<ProtocolObject<dyn MTLLibrary>>, ArmError> {
    let mut error: *mut AnyObject = ptr::null_mut();
    // SAFETY: `device` は生きている Metal の device で、`newLibraryWithURL:error:` を受ける。
    // `url` は `file_url` が作った生きている `NSURL` で、この呼び出しの間だけ参照される。
    // 最後の引数は `NSError **` に当たる、局所の `*mut AnyObject` を指す書き込み先で、呼び出しの間だけ
    // 生きる。戻りは `new` 系の所有で、失敗で nil になりうるので `Option` で受ける。
    let library: Option<Retained<ProtocolObject<dyn MTLLibrary>>> =
        unsafe { msg_send![device, newLibraryWithURL: url, error: &mut error] };
    library.ok_or(ArmError::LibraryLoadFailed)
}

/// library から、名前で入口の関数を作る（function constant は使わない）。
fn new_function_by_name(
    library: &ProtocolObject<dyn MTLLibrary>,
    name: &AnyObject,
) -> Result<Retained<ProtocolObject<dyn MTLFunction>>, ArmError> {
    // SAFETY: `library` は生きている library で、`newFunctionWithName:` を受ける。
    // `name` は `make_nsstring` が作った生きている `NSString` で、この呼び出しの間だけ参照される。
    // 戻りは `new` 系の所有で、見つからないと nil になるので `Option` で受ける。
    let function: Option<Retained<ProtocolObject<dyn MTLFunction>>> =
        unsafe { msg_send![library, newFunctionWithName: name] };
    function.ok_or(ArmError::FunctionFailed)
}

/// function constant を 1 つ設定する。`value` の型は `data_type` の大きさと一致させる呼び出しだけがある
/// （`UInt` は `u32`、`Bool` は `bool`）。
fn set_constant<V>(
    values: &MTLFunctionConstantValues,
    value: &V,
    data_type: MTLDataType,
    index: usize,
) {
    let pointer = NonNull::from(value).cast::<c_void>();
    // SAFETY: `pointer` は `value` を指す非 null のポインタで、この呼び出しの間だけ生きる。呼び出し側は
    // `data_type` が `UInt` のとき `V = u32`（4 バイト・4 バイト整列）、`Bool` のとき `V = bool`（1 バイト）
    // だけを渡し、Metal が読む大きさと整列が `V` と一致する。`index` は表の固定の値（0〜17）で、
    // Metal は設定の値をコピーする。
    unsafe {
        values.setConstantValue_type_atIndex(pointer, data_type, index);
    }
}

/// 共有のストレージモードのバッファを `elems × elem_size` バイトで確保する。0 長は拒否する。
fn new_shared_buffer(
    device: &ProtocolObject<dyn MTLDevice>,
    elems: usize,
    elem_size: usize,
) -> Result<Retained<ProtocolObject<dyn MTLBuffer>>, ArmError> {
    let bytes = elems.checked_mul(elem_size).ok_or(ArmError::Overflow)?;
    if bytes == 0 {
        return Err(ArmError::AllocationFailed);
    }
    device
        .newBufferWithLength_options(bytes, MTLResourceOptions::StorageModeShared)
        .ok_or(ArmError::AllocationFailed)
}

/// バッファの先頭へ `data` を書く。バッファの長さを超える書き込みは拒否する。
fn write_buffer<T: Copy>(
    buffer: &ProtocolObject<dyn MTLBuffer>,
    data: &[T],
) -> Result<(), ArmError> {
    let bytes = data
        .len()
        .checked_mul(size_of::<T>())
        .ok_or(ArmError::Overflow)?;
    if bytes > buffer.length() {
        return Err(ArmError::LengthMismatch);
    }
    let destination = buffer.contents().cast::<T>();
    // SAFETY: `destination` は共有ストレージのバッファの先頭で、長さは直前の検査で `bytes` 以上と分かり、
    // バッファは CPU から書ける。`T` の整列は Metal のバッファの先頭の整列（16 バイト以上）に収まる。
    // `data` は別の確保なので領域は重ならない。この時点でそのバッファを使う GPU の仕事は投入されていない。
    unsafe {
        ptr::copy_nonoverlapping(data.as_ptr(), destination.as_ptr(), data.len());
    }
    Ok(())
}

/// バッファの全体を 0 で埋める。
fn zero_buffer(buffer: &ProtocolObject<dyn MTLBuffer>) -> Result<(), ArmError> {
    let destination = buffer.contents().cast::<u8>();
    // SAFETY: `destination` は共有ストレージのバッファの先頭で、`length()` バイトが CPU から書ける。
    // この時点でそのバッファを使う GPU の仕事は投入されていない。
    unsafe {
        ptr::write_bytes(destination.as_ptr(), 0, buffer.length());
    }
    Ok(())
}

/// 完了待ちの後のバッファから、先頭の `elems` 要素を読む。バッファの長さを超える読み出しは拒否する。
fn read_buffer<T: Copy>(
    buffer: &ProtocolObject<dyn MTLBuffer>,
    elems: usize,
) -> Result<Vec<T>, ArmError> {
    let bytes = elems
        .checked_mul(size_of::<T>())
        .ok_or(ArmError::Overflow)?;
    if bytes > buffer.length() {
        return Err(ArmError::LengthMismatch);
    }
    let source = buffer.contents().cast::<T>();
    // SAFETY: 呼び出し側は `waitUntilCompleted` と完了の状態の確認の後にだけ呼ぶので、GPU の書き込みは
    // 終わっていて、CPU との読み書きが重ならない。`source` はバッファの先頭で、長さは直前の検査で
    // `elems` 要素以上と分かる。バッファは 0 で埋めてから渡したので全要素が初期化済みで、
    // `T` は `f32` か `u16` でどんなビット列も有効な値になる。
    let slice = unsafe { std::slice::from_raw_parts(source.as_ptr(), elems) };
    Ok(slice.to_vec())
}

/// A・B・C を buffer 0〜2 に offset 0 で束縛する。
fn bind_buffers(
    encoder: &ProtocolObject<dyn MTLComputeCommandEncoder>,
    buffers: &[&Retained<ProtocolObject<dyn MTLBuffer>>; 3],
) {
    // SAFETY: 3 つのバッファは呼び出し側が保持していて、完了待ちが終わるまで生きる（エンコーダは
    // 保持しないため、呼び出し側の保持が生存を保証する）。offset 0 はどのバッファの範囲にも収まり、
    // index 0〜2 は kernel が宣言する buffer の範囲内。内容は kernel が読む型（f32 か binary16）の
    // 行優先の列で、C だけが書かれる。
    unsafe {
        for (index, buffer) in buffers.iter().enumerate() {
            encoder.setBuffer_offset_atIndex(Some(&***buffer), 0, index);
        }
    }
}

/// `repr(C)` の小さな値を `setBytes` で渡す。
fn set_bytes<V: Copy>(
    encoder: &ProtocolObject<dyn MTLComputeCommandEncoder>,
    value: &V,
    index: usize,
) {
    let pointer = NonNull::from(value).cast::<c_void>();
    // SAFETY: `pointer` は `value` を指す非 null のポインタで、`size_of::<V>()` バイトが読める。`V` は
    // この module の `repr(C)` の `u32` だけの構造体で、padding を持たない。Metal は呼び出しの間に
    // バイト列をコピーするので、`value` の生存はこの呼び出しの間で足りる。大きさは 4096 バイト未満で、
    // `index` は kernel が宣言する buffer の範囲内。
    unsafe {
        encoder.setBytes_length_atIndex(pointer, size_of::<V>(), index);
    }
}

/// threadgroup のメモリ長を index 0 に設定する。
fn set_threadgroup_memory(encoder: &ProtocolObject<dyn MTLComputeCommandEncoder>, length: usize) {
    // SAFETY: index 0 は kernel が宣言する threadgroup のメモリの先頭で、`length` は起動の表の値
    // （16 の倍数で、device の上限より小さい固定値）。設定は呼び出しの間にエンコーダの状態へ書かれるだけ。
    unsafe {
        encoder.setThreadgroupMemoryLength_atIndex(length, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `NSString` を作って UTF-8 のバイト長を読み戻せる（device を作らない）。
    #[test]
    fn nsstring_round_trips_byte_length() {
        let text = "abc あ";
        let object = make_nsstring(text).expect("nsstring");
        // SAFETY: `object` は `make_nsstring` が作った生きている `NSString` で、
        // `lengthOfBytesUsingEncoding:` は `NSStringEncoding` を 1 つ取り `NSUInteger` を返す。
        let length: usize =
            unsafe { msg_send![&*object, lengthOfBytesUsingEncoding: UTF8_ENCODING] };
        assert_eq!(length, text.len());
    }

    /// コンパイルのオプションに設定した 2 つが読み戻せる（device を作らない）。
    #[test]
    fn compile_options_hold_both_settings() {
        let options = compile_options().expect("options");
        assert_eq!(options.mathMode(), MTLMathMode::Safe);
        assert_eq!(
            options.mathFloatingPointFunctions(),
            MTLMathFloatingPointFunctions::Precise
        );
    }

    /// `MTLFunctionConstantValues` に 2 種類の値を設定できる（device を作らない）。
    #[test]
    fn function_constant_values_accept_both_kinds() {
        let values = MTLFunctionConstantValues::new();
        set_constant(&values, &7_u32, MTLDataType::UInt, 0);
        set_constant(&values, &true, MTLDataType::Bool, 5);
    }

    /// 絶対パスから作った `NSURL` がファイルの URL で、パスのバイト長が一致する（device を作らない）。
    #[test]
    fn file_url_round_trips_path() {
        let text = "/tmp/measure-metal-gemm-arm/abc.metallib";
        let url = file_url(Path::new(text)).expect("url");
        // SAFETY: `url` は `file_url` が作った生きている `NSURL` で、`path` は引数なしで `NSString` を返し、
        // `isFileURL` は `BOOL` を返す。`lengthOfBytesUsingEncoding:` は `NSStringEncoding` を 1 つ取り
        // `NSUInteger` を返す。取り出した `NSString` は自動解放で、この関数の間は生きている。
        let (length, is_file): (usize, bool) = unsafe {
            let path_string: Retained<AnyObject> = msg_send![&*url, path];
            let length: usize = msg_send![&*path_string, lengthOfBytesUsingEncoding: UTF8_ENCODING];
            let is_file: bool = msg_send![&*url, isFileURL];
            (length, is_file)
        };
        assert_eq!(length, text.len());
        assert!(is_file);
    }
}
