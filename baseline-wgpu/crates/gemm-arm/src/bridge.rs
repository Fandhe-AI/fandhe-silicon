//! 下位の API（wgpu の実験的な機能の同意の印・検査を外したシェーダーの作成）の unsafe な口を呼ぶ橋（spec TASK-1010 のポインタ）。
//!
//! この crate で `unsafe` を持つのはこの module だけ。`device` の経路が device の要求に渡す値をここから受け取る。
//! 外へ出すのは `unsafe fn` の [`run_g2`] と [`run_g3prime`] で、実行時に受けるシェーダーの条件は呼び出し側が保証する。
//! `run_g3prime` の Metal を直接呼ぶ後半と、その部品（それぞれ `unsafe` を 1 つずつ持つ private の関数）もこの module にある。

use fandhe_silicon_core::parity::gemm_gate::{GateInputs, GateOutput};
use fandhe_silicon_core::parity::gemm_population::{GemmGateArm, GemmGateCell};

use crate::device::{ArmError, check_g2_request, run_tiled};
use crate::prime::PrimeJob;

#[cfg(target_os = "macos")]
use std::ffi::c_void;
#[cfg(target_os = "macos")]
use std::ptr::{self, NonNull};

#[cfg(target_os = "macos")]
use objc2::msg_send;
#[cfg(target_os = "macos")]
use objc2::rc::{Allocated, Retained};
#[cfg(target_os = "macos")]
use objc2::runtime::{AnyClass, AnyObject, ProtocolObject};
// コマンドラインのプログラムで `MTLCreateSystemDefaultDevice` が既定の device を返すには、
// Core Graphics がリンクされている必要がある。
#[cfg(target_os = "macos")]
use objc2_core_graphics as _;
#[cfg(target_os = "macos")]
use objc2_metal::{
    MTLBuffer, MTLCommandBuffer, MTLCommandBufferStatus, MTLCommandEncoder, MTLCommandQueue,
    MTLCompileOptions, MTLComputeCommandEncoder, MTLComputePipelineDescriptor,
    MTLComputePipelineState, MTLCreateSystemDefaultDevice, MTLDevice, MTLFunction,
    MTLLanguageVersion, MTLLibrary, MTLMutability, MTLPipelineBufferDescriptorArray,
    MTLPipelineOption, MTLResourceOptions, MTLSize,
};

#[cfg(target_os = "macos")]
use crate::layout::{LayoutError, constants_bytes, crop_c, workgroup_count};
#[cfg(target_os = "macos")]
use crate::msl::MslError;
#[cfg(target_os = "macos")]
use crate::prime::{PaddedInputs, PrimeLanguageVersion, capture_text};
#[cfg(target_os = "macos")]
use crate::{ENTRY_POINT, WORKGROUP_SIZE};

/// `NSString` の UTF-8 を表す encoding の定数。
#[cfg(target_os = "macos")]
const UTF8_ENCODING: usize = 4;

/// 実験的な機能を使う同意の印を作る。
pub(crate) fn experimental_features() -> wgpu::ExperimentalFeatures {
    // SAFETY: この関数が返す値で要求する device（g3 の経路）が実験的な機能として使うのは
    // cooperative matrix の WGSL の作成と dispatch だけで、その device のシェーダーの作成は
    // 検査つきの口だけを使う（`run_g3prime` の pipeline の作成もこの device で、検査つきの口だけを使う）。
    // `create_unchecked_module` は `run_g2` が別に要求する device
    // （実験的な機能を要求しない）のシェーダーの作成だけが使い、この値を受けない。
    unsafe { wgpu::ExperimentalFeatures::enabled() }
}

/// 検査を外してシェーダーを作る。呼び出し元は `run_g2` が渡す共通の本体だけ。
fn create_unchecked_module(device: &wgpu::Device, wgsl: &str) -> wgpu::ShaderModule {
    // SAFETY: 渡すテキストは、実行時に渡された sha256 と一致し、compute の入口の数・workgroup の大きさ・
    // 束縛の宣言の形の照らしを通ったものだけ（共通の本体が `ExternalWgsl` から取る）。束縛する buffer の長さは
    // M×K・K×N・M×N の f32 と 16 バイトの uniform で、uniform の値はその M・N・K。呼ぶのは検証レイヤーが
    // 有効なことを確かめた後だけ。テキストの入口が束縛を宣言どおりにだけ使うことは、`run_g2` の
    // `# Safety` で呼び出し側が保証する。
    unsafe {
        device.create_shader_module_trusted(
            wgpu::ShaderModuleDescriptor {
                label: Some("gemm-arm-external-unchecked"),
                source: wgpu::ShaderSource::Wgsl(wgsl.into()),
            },
            wgpu::ShaderRuntimeChecks::unchecked(),
        )
    }
}

/// 実行時に環境変数で受けた WGSL を、検査を外したシェーダーの作成で device に流し、出力とソースの sha256 を返す。
///
/// 手順は `check_g2_request`（腕・dtype・入力の長さ・検証レイヤー）、ソースの受け取り、device の要求、
/// シェーダー・パイプラインの作成、1 回の投入、読み戻しの順。成果物（パス・バイト列・digest）は引数で受けない。
/// 検証レイヤーが無効、ソースの設定が未設定・不正、照合が通らないときは device を作る前に `Err` を返す。
///
/// # Safety
///
/// 環境変数が指すソースの sha256 の照合と形の照らしは、内容がそのバイト列であることと、入口の数・
/// workgroup の大きさ・束縛の宣言の形しか保証しない。この関数はそのソースのシェーダーを
/// `wgpu::ShaderRuntimeChecks::unchecked()` で作るので、呼び出し側は、そのソースの入口が、`@group(0)` の
/// binding 0・1 を f32 の行優先の列 A・B（読み取りだけ）、binding 2 を f32 の行優先の列 C、binding 3 を
/// M・N・K の定数として宣言どおりにだけ使い、範囲は M・N・K から決まり、範囲外のアクセスや競合する書き込み、
/// 無限ループを持たない監査済みのカーネルであることを保証しなければならない。
pub unsafe fn run_g2(
    cell: GemmGateCell,
    inputs: &GateInputs,
) -> Result<(GateOutput, [u8; 32]), ArmError> {
    check_g2_request(cell, inputs)?;
    run_tiled(GemmGateArm::G2, cell, inputs, create_unchecked_module)
}
/// 受け取ったシェーダーのテキストを Metal で直接コンパイルして 1 回流し、出力と受け取ったテキストの sha256 を返す。
///
/// 手順は `check_g3prime_request`（腕・dtype と入力の種類・検証レイヤー）、G3 と同じ詰め物、
/// pipeline の作成の間の Debug の記録からのテキストの受け取り、言語版と束縛の番号の読み取り、
/// Metal でのコンパイルと pipeline の作成、1 回の投入、読み戻しの順。macOS でない target では
/// 詰め物の後に固定の `Err` を返し、ほかの経路で計算しない。成果物・環境変数は受けない。
///
/// # Safety
///
/// 受け取るテキストは、この crate の WGSL で pipeline を作る間の Debug の記録から、1 行目の入口名と
/// 2 行目の形で選んだもので、言語版・束縛の番号・大きさの表の照らしは形しか保証しない。この関数は
/// そのテキストを Metal で直接コンパイルして投入するので、呼び出し側は、呼び出しの間に同じプロセスの
/// ほかのコードがその形の Debug の記録を出さないことと、そのテキストの入口が、`a`・`b` の番号の
/// バッファを入力の行優先の列（読み取りだけ）、`c` の番号を f32 の出力の行優先の列、`dims` の番号を
/// 16 バイトの定数、`_buffer_sizes` の番号を 3 つのバッファのバイト長として宣言どおりにだけ使い、
/// 範囲は詰めた後の M・N・K から決まり、範囲外のアクセスや競合する書き込み、無限ループを持たない
/// ことを保証しなければならない。
pub unsafe fn run_g3prime(
    cell: GemmGateCell,
    inputs: &GateInputs,
) -> Result<(GateOutput, [u8; 32]), ArmError> {
    crate::prime::check_g3prime_request(cell, inputs)?;
    let job = crate::prime::pad_g3prime(cell, inputs)?;
    run_prepared(&job)
}

/// macOS でない target では、device に触れず固定の `Err` を返す。
#[cfg(not(target_os = "macos"))]
fn run_prepared(job: &PrimeJob) -> Result<(GateOutput, [u8; 32]), ArmError> {
    let _ = job;
    Err(ArmError::UnsupportedTarget)
}

/// 詰めた入力を、受け取ったテキストの Metal の pipeline で 1 回流す。呼び出し元は `run_g3prime` だけ。
#[cfg(target_os = "macos")]
fn run_prepared(job: &PrimeJob) -> Result<(GateOutput, [u8; 32]), ArmError> {
    let captured = capture_text(job.is_f16())?;
    let slots = captured.slots;
    let (a_index, b_index, c_index) = (slot(slots.a())?, slot(slots.b())?, slot(slots.c())?);
    let (dims_index, sizes_index) = (slot(slots.dims())?, slot(slots.sizes())?);

    let device = MTLCreateSystemDefaultDevice().ok_or(ArmError::NoMetalDevice)?;
    let options = MTLCompileOptions::new();
    options.setLanguageVersion(metal_language_version(captured.version));
    options.setPreserveInvariance(true);
    let source = make_nsstring(&captured.text)?;
    let library = new_library(&device, &source, &options)?;
    let name = make_nsstring(ENTRY_POINT)?;
    let function = new_function(&library, &name)?;

    let descriptor = MTLComputePipelineDescriptor::new();
    descriptor.setComputeFunction(Some(&*function));
    let buffers = descriptor.buffers();
    for index in [a_index, b_index, dims_index] {
        set_immutable(&buffers, index);
    }
    let pipeline = device
        .newComputePipelineStateWithDescriptor_options_reflection_error(
            &descriptor,
            MTLPipelineOption::empty(),
            None,
        )
        .map_err(|_| ArmError::Pipeline)?;
    let threads = usize::try_from(WORKGROUP_SIZE[0]).map_err(|_| ArmError::ThreadsExceeded)?;
    if pipeline.maxTotalThreadsPerThreadgroup() < threads {
        return Err(ArmError::ThreadsExceeded);
    }

    let layout = job.layout();
    let c_len = job.c_len();
    let (a_buffer, b_buffer) = match job.padded() {
        PaddedInputs::F32 { a, b } => {
            let a_buffer = new_shared_buffer(&device, a.len(), size_of::<f32>())?;
            let b_buffer = new_shared_buffer(&device, b.len(), size_of::<f32>())?;
            write_buffer(&a_buffer, a)?;
            write_buffer(&b_buffer, b)?;
            (a_buffer, b_buffer)
        }
        PaddedInputs::Binary16 { a, b } => {
            let a_buffer = new_shared_buffer(&device, a.len(), size_of::<u16>())?;
            let b_buffer = new_shared_buffer(&device, b.len(), size_of::<u16>())?;
            write_buffer(&a_buffer, a)?;
            write_buffer(&b_buffer, b)?;
            (a_buffer, b_buffer)
        }
    };
    let c_buffer = new_shared_buffer(&device, c_len, size_of::<f32>())?;
    let mut zeros: Vec<f32> = Vec::new();
    zeros
        .try_reserve_exact(c_len)
        .map_err(|_| ArmError::Layout(LayoutError::AllocationFailed))?;
    zeros.resize(c_len, 0.0);
    write_buffer(&c_buffer, &zeros)?;

    let queue = device.newCommandQueue().ok_or(ArmError::Allocation)?;
    let command_buffer = queue.commandBuffer().ok_or(ArmError::Submit)?;
    let encoder = command_buffer
        .computeCommandEncoder()
        .ok_or(ArmError::Submit)?;
    encoder.setComputePipelineState(&pipeline);
    bind_buffer(&encoder, &a_buffer, a_index);
    bind_buffer(&encoder, &b_buffer, b_index);
    bind_buffer(&encoder, &c_buffer, c_index);
    set_bytes(&encoder, &constants_bytes(layout), dims_index);
    set_bytes(&encoder, job.sizes(), sizes_index);
    let [gx, gy, gz] = workgroup_count(layout);
    let to_size = |x: u32| usize::try_from(x).map_err(|_| ArmError::Submit);
    let [tx, ty, tz] = WORKGROUP_SIZE;
    encoder.dispatchThreadgroups_threadsPerThreadgroup(
        MTLSize {
            width: to_size(gx)?,
            height: to_size(gy)?,
            depth: to_size(gz)?,
        },
        MTLSize {
            width: to_size(tx)?,
            height: to_size(ty)?,
            depth: to_size(tz)?,
        },
    );
    encoder.endEncoding();
    command_buffer.commit();
    command_buffer.waitUntilCompleted();
    if command_buffer.status() != MTLCommandBufferStatus::Completed {
        return Err(ArmError::Submit);
    }

    let c_padded = read_f32(&c_buffer, c_len)?;
    Ok((GateOutput::F32(crop_c(layout, &c_padded)?), captured.sha256))
}

/// 束縛の番号を添字にする。
#[cfg(target_os = "macos")]
fn slot(n: u32) -> Result<usize, ArmError> {
    usize::try_from(n).map_err(|_| ArmError::Text(MslError::SlotNumber))
}

/// 受け付けた言語版を Metal の列挙値に写す。
#[cfg(target_os = "macos")]
fn metal_language_version(v: PrimeLanguageVersion) -> MTLLanguageVersion {
    match v {
        PrimeLanguageVersion::V2_3 => MTLLanguageVersion::Version2_3,
        PrimeLanguageVersion::V2_4 => MTLLanguageVersion::Version2_4,
        PrimeLanguageVersion::V3_0 => MTLLanguageVersion::Version3_0,
        PrimeLanguageVersion::V3_1 => MTLLanguageVersion::Version3_1,
        PrimeLanguageVersion::V3_2 => MTLLanguageVersion::Version3_2,
        PrimeLanguageVersion::V4_0 => MTLLanguageVersion::Version4_0,
    }
}

/// 共有ストレージのバッファを作る。0 長・あふれ・確保の失敗は `Err`。
#[cfg(target_os = "macos")]
fn new_shared_buffer(
    device: &ProtocolObject<dyn MTLDevice>,
    elems: usize,
    elem_size: usize,
) -> Result<Retained<ProtocolObject<dyn MTLBuffer>>, ArmError> {
    let bytes = elems
        .checked_mul(elem_size)
        .ok_or(ArmError::Layout(LayoutError::SizeOverflow))?;
    if bytes == 0 {
        return Err(ArmError::Allocation);
    }
    device
        .newBufferWithLength_options(bytes, MTLResourceOptions::StorageModeShared)
        .ok_or(ArmError::Allocation)
}

/// UTF-8 の文字列から `NSString` を作る。
#[cfg(target_os = "macos")]
fn make_nsstring(text: &str) -> Result<Retained<AnyObject>, ArmError> {
    let class = AnyClass::get(c"NSString").ok_or(ArmError::Allocation)?;
    let bytes = text.as_ptr().cast::<c_void>();
    // SAFETY: `class` は実行時から得た `NSString` のクラスで、`alloc` は引数を取らず `Allocated` を返す。
    // `initWithBytes:length:encoding:` には、`text` の先頭を指す `bytes` と、その実際の長さ `text.len()`、
    // UTF-8 を表す定数 4 を渡す。`text` は `&str` なので有効な UTF-8 で、この呼び出しの間だけ生き、
    // 初期化は内容をコピーする。戻りは失敗で nil になりうるので `Option` で受ける。
    let object: Option<Retained<AnyObject>> = unsafe {
        let allocated: Allocated<AnyObject> = msg_send![class, alloc];
        msg_send![allocated, initWithBytes: bytes, length: text.len(), encoding: UTF8_ENCODING]
    };
    object.ok_or(ArmError::Allocation)
}

/// テキストの `NSString` から library を作る。失敗の中身は読まず固定の `Err` にする。
#[cfg(target_os = "macos")]
fn new_library(
    device: &ProtocolObject<dyn MTLDevice>,
    source: &AnyObject,
    options: &MTLCompileOptions,
) -> Result<Retained<ProtocolObject<dyn MTLLibrary>>, ArmError> {
    let mut error: *mut AnyObject = ptr::null_mut();
    // SAFETY: `device` は生きている Metal の device で、`newLibraryWithSource:options:error:` を受ける。
    // `source` は `make_nsstring` が作った `NSString`、`options` は生きている `MTLCompileOptions` で、
    // どれもこの呼び出しの間だけ参照される。最後の引数は `NSError **` に当たる、局所の
    // `*mut AnyObject` を指す書き込み先で、呼び出しの間だけ生きる。戻りは `new` 系の所有で、
    // 失敗で nil になりうるので `Option` で受ける。コンパイルするテキストが `run_g3prime` の
    // `# Safety` の条件を満たすことは、呼び出し側が保証する。
    let library: Option<Retained<ProtocolObject<dyn MTLLibrary>>> = unsafe {
        msg_send![device, newLibraryWithSource: source, options: options, error: &mut error]
    };
    library.ok_or(ArmError::Compile)
}

/// library から入口の関数を作る。失敗の中身は読まない。
#[cfg(target_os = "macos")]
fn new_function(
    library: &ProtocolObject<dyn MTLLibrary>,
    name: &AnyObject,
) -> Result<Retained<ProtocolObject<dyn MTLFunction>>, ArmError> {
    // SAFETY: `library` は生きている library で、`newFunctionWithName:` を受ける。`name` は
    // `make_nsstring` が作った `NSString` で、この呼び出しの間だけ参照される。戻りは `new` 系の所有で、
    // 名前の関数が無いと nil になりうるので `Option` で受ける。
    let function: Option<Retained<ProtocolObject<dyn MTLFunction>>> =
        unsafe { msg_send![library, newFunctionWithName: name] };
    function.ok_or(ArmError::Function)
}

/// pipeline の `index` 番のバッファを、書き込まないものとして宣言する。
#[cfg(target_os = "macos")]
fn set_immutable(buffers: &MTLPipelineBufferDescriptorArray, index: usize) {
    // SAFETY: `objectAtIndexedSubscript:` は添字を検査しないことがあるが、`index` は
    // `buffer_slots` が 31 未満の 10 進と確かめた束縛の番号で、バッファの添字の範囲内に収まる。
    // 戻りは配列が持つ記述子の保持された参照で、この呼び出しの間だけ使う。
    let descriptor = unsafe { buffers.objectAtIndexedSubscript(index) };
    descriptor.setMutability(MTLMutability::Immutable);
}

/// バッファの先頭へ `data` を書く。バッファの長さを超える書き込みは拒否する。
#[cfg(target_os = "macos")]
fn write_buffer<T: Copy>(
    buffer: &ProtocolObject<dyn MTLBuffer>,
    data: &[T],
) -> Result<(), ArmError> {
    let bytes = data
        .len()
        .checked_mul(size_of::<T>())
        .ok_or(ArmError::Layout(LayoutError::SizeOverflow))?;
    if bytes > buffer.length() {
        return Err(ArmError::Allocation);
    }
    let destination = buffer.contents().cast::<T>();
    // SAFETY: `destination` は共有ストレージのバッファの先頭で、長さは直前の検査で `bytes` 以上と分かり、
    // バッファは CPU から書ける。`T` は `f32` か `u16` で、整列は Metal のバッファの先頭の整列
    // （16 バイト以上）に収まる。`data` は別の確保なので領域は重ならない。この時点でそのバッファを使う
    // GPU の仕事は投入されていない。
    unsafe {
        ptr::copy_nonoverlapping(data.as_ptr(), destination.as_ptr(), data.len());
    }
    Ok(())
}

/// 完了待ちの後のバッファから、先頭の `elems` 個の f32 を読む。バッファの長さを超える読み出しは拒否する。
#[cfg(target_os = "macos")]
fn read_f32(buffer: &ProtocolObject<dyn MTLBuffer>, elems: usize) -> Result<Vec<f32>, ArmError> {
    let bytes = elems
        .checked_mul(size_of::<f32>())
        .ok_or(ArmError::Layout(LayoutError::SizeOverflow))?;
    if bytes > buffer.length() {
        return Err(ArmError::Allocation);
    }
    let source = buffer.contents().cast::<f32>();
    // SAFETY: 呼び出し側は `waitUntilCompleted` と完了の状態の確認の後にだけ呼ぶので、GPU の書き込みは
    // 終わっていて、CPU との読み書きが重ならない。`source` はバッファの先頭で、長さは直前の検査で
    // `elems` 個以上と分かる。バッファは 0 で埋めてから渡したので全要素が初期化済みで、
    // `f32` はどんなビット列も有効な値になる。
    let slice = unsafe { std::slice::from_raw_parts(source.as_ptr(), elems) };
    Ok(slice.to_vec())
}

/// バッファを `index` 番に offset 0 で束縛する。
#[cfg(target_os = "macos")]
fn bind_buffer(
    encoder: &ProtocolObject<dyn MTLComputeCommandEncoder>,
    buffer: &ProtocolObject<dyn MTLBuffer>,
    index: usize,
) {
    // SAFETY: `buffer` は呼び出し側が保持していて、完了待ちが終わるまで生きる（エンコーダは保持しない
    // ため、呼び出し側の保持が生存を保証する）。offset 0 はバッファの範囲に収まり、`index` は
    // `buffer_slots` が確かめた入口の引数の番号。入口がその番号を宣言どおりにだけ使うことは、
    // `run_g3prime` の `# Safety` の契約として呼び出し側が保証する。
    unsafe {
        encoder.setBuffer_offset_atIndex(Some(buffer), 0, index);
    }
}

/// 小さなバイト列を `setBytes` で `index` 番に渡す。
#[cfg(target_os = "macos")]
fn set_bytes(encoder: &ProtocolObject<dyn MTLComputeCommandEncoder>, bytes: &[u8], index: usize) {
    let pointer = NonNull::from(bytes).cast::<c_void>();
    // SAFETY: `pointer` は `bytes` の先頭を指す非 null のポインタで、`bytes.len()` バイトが読める。
    // Metal は呼び出しの間にバイト列をコピーするので、`bytes` の生存はこの呼び出しの間で足りる。
    // 渡す長さは 16 バイトか 12 バイトで 4096 バイト未満。`index` は `buffer_slots` が確かめた入口の
    // 引数の番号で、その使い方の前提は `run_g3prime` の `# Safety` の契約として呼び出し側が保証する。
    unsafe {
        encoder.setBytes_length_atIndex(pointer, bytes.len(), index);
    }
}
