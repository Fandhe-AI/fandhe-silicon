//! Metal の背後実装（下-1）。`backend-metal` feature で有効になる。
//!
//! この背後実装の `unsafe` / FFI はこのモジュールに閉じ込め、外へは safe な API だけを出す。
//! macOS のときだけ、既定の device の取得（`DeviceProvider::request`）・診断・バッファの確保と読み書き・
//! 投入の窓口（`Device::queue`）・compute の口（`Device::compute()`。`ComputeOps` の `create_pipeline` へ届く）を持つ
//! （TASK-158、DEV-1・DEV-16・DEV-67・DEV-68・DEV-206・DEV-210・DEV-211・TASK-265、TASK-592 のポインタ）。
//! macOS 以外（Linux・wasm32）では、feature を付けても実装は空のままで、公開の項目を持たない
//! （device に触らない純粋な検査の関数だけが、この段の試験のために置かれている）。
//! Objective-C のバインディングは `objc2` 系の 4 crate（macOS 向けにだけ宣言。承認の記録は `deps-approved.toml`）。
//!
//! 公開するのは `MetalDeviceProvider` だけ。device の型 `MetalDevice` は非公開で、上・下-2 は
//! `request` が返す `Box<dyn Device>` 越しに使う。
//!
//! `dispatch_ex` は検査に通った引数を、記録先ごとの表に plain な値として積む。投入（`submit`・`submit_many`）は、
//! 束縛したバッファが投入の時にまだ有効なことを全件確かめ、コマンドバッファを 1 つ作り、記録のある記録先ごとに
//! compute encoder を 1 つ開いて記録の順に積み、確定する。束縛した各バッファの最後に使った投入は、払い出した
//! 通し番号になる（確かめと登録は同じ錠の中）。コピー・クリア・待ちセマフォ・実際のロストの検出は持たない。
//! 読み書きの `unsafe` ブロックの同期の根拠は、最後に使った投入が完了していることを確かめてからコピーすることに置く。
//! ロストは、feature `fault-injection` の模擬（`FaultHooks::simulate_device_lost`）と、device の型の内側の
//! コールバックの登録（`DeviceLostNotify`）だけがある。
//!
//! 製品のビルドでは、記録のある投入は `Unsupported` で拒む（`DISPATCH_EXECUTION_ENABLED`）。カーネルが触る範囲の保証が
//! 入るまで、safe な経路から任意の MSL を走らせない。積み込みの形は単体試験のビルドだけが確かめる。
//!
//! `free` は、表から外したバッファを最後に使った投入が終わっていなければ解放の保留の一覧に置き、
//! ゼロ化も再利用の一覧（pool）への回しもしない。終わったら（完了か失敗）、全バイトを 0 にしてから
//! 同じ `byte_len` の pool へ回す（上限を超える分は解放する）。再利用された `allocate` の利用者に以前の内容は見えない。
//! pool に入ったバッファは、device の Drop まで保持される（合計の上限は 64 MiB）。
//! 保留の一覧のバッファは、device の Drop ではゼロ化せずに落とす。
//!
//! `MetalDevice` は Objective-C のオブジェクトを持つので `Send`・`Sync` ではない（`unsafe impl` は足さない）。
//! `create_pipeline`（TASK-773 のポインタ。`Device::compute()` から届く）は MSL のソースから compute パイプライン状態を作り、パイプラインの表に
//! `layout` の写しと一緒に置く。コンパイルのオプション付きの 2 つの入力も受け付ける（TASK-774 のポインタ）。function constant の設定の `unsafe` ブロックは 1 つで、記録とは関係しない。
//! `create_pipeline` の MSL のコンパイルの手順は `compile_function` にあり、`reflect_function` は同じ手順で作った関数から reflection 付きでパイプライン状態を作って、引数の数とバッファの引数ごとの slot・バイト長・型を取り出す（TASK-493 のポインタ）。
//! キャッシュキーの口（`pipeline_cache_key`。TASK-501 のポインタ）は MSL の 4 つの入力でキーを返し、パイプラインを作らない。
//! 投入済みの表は device の錠の中に置き、記録先の後始末の hook は記録先の表だけを握る。
//! `on_complete` の登録の表は投入済みの表と同じ錠の中に置き、callback は錠を放してから呼ぶ。
//! `dispatch_checked`（TASK-436 のポインタ）は、束縛のハンドルを表の項目と照らし、バッファの実際の長さと `limits()` の上限で引数の宣言を照らしてから `dispatch_ex` に委ねる。
//! 駆動は `submit`・`submit_many`・`poll`・`poll_ex`・`wait_blocking`・`allocate` の中で行い、待ちのスレッドは持たない。
//! feature `artifact-inspection` のビルドでは `Device::artifact_hooks()` が `ArtifactHooks` の実装（binary archive の書き出しと読み込み、metallib からのパイプラインの生成）を返す（TASK-492 のポインタ）。

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use fandhe_silicon_core::capability::BackendKind;
#[cfg(feature = "fault-injection")]
use fandhe_silicon_core::device_lost::LostCallback;
#[cfg(feature = "fault-injection")]
use fandhe_silicon_core::error::DeviceLostReason;
use fandhe_silicon_core::error::{CompletionState, CompletionStateEx, ContractError};
use fandhe_silicon_core::execution::{
    __BackendSharedEncoderCloser, CacheKey, DType, FunctionConstant, FunctionConstantValue,
    GpuTiming, ImportedSemaphoreHandle, MslFloatFunctions, MslMathMode, OperandBinding,
    OperandConstant, OperandSpec, PipelineInput, SubmitToken, validate_dispatch_operands,
};
use fandhe_silicon_core::hash::sha256_u64;
use fandhe_silicon_core::memory::validate_buffer_range;

/// 部分範囲 `[offset, offset + len)` を確保長 `allocation_len` に照らし、ホストの `usize` の組にして返す。
///
/// `upload` の宛先と `download_*` の元が共有する。終端の `u64` のあふれと確保長の超過は共通の
/// `validate_buffer_range` が `InvalidArgument` で拒む。検査を通った後でも `usize` に収まらない値は、
/// 切り詰めずに `InvalidArgument` で返す。device に触らず、バッファの内容も変えない。
/// `usize` が 64 bit 以上の環境では `u64` の値は必ず収まるので、この分岐は試験では通らない。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn host_range(offset: u64, len: u64, allocation_len: u64) -> Result<(usize, usize), ContractError> {
    validate_buffer_range(offset, len, allocation_len)?;
    match (usize::try_from(offset), usize::try_from(len)) {
        (Ok(offset), Ok(len)) => Ok((offset, len)),
        _ => Err(ContractError::InvalidArgument(
            "buffer range does not fit usize".into(),
        )),
    }
}

/// 源から次の値を取る。あふれたら `None` を返し、源は動かさない。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn take_next(counter: &AtomicU64) -> Option<u64> {
    // toolchain の版に依らず警告の出ない、比較交換のループで払い出す。
    let mut current = counter.load(Ordering::Relaxed);
    loop {
        let next = current.checked_add(1)?;
        match counter.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return Some(current),
            Err(actual) => current = actual,
        }
    }
}

/// `create_pipeline` が受け付ける入力から取り出した、ソース・入口名・function constant の並び・
/// 特殊化の入力（`MslSpecialized`）だったか。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
type AcceptedPipelineInput<'a> = (&'a str, &'a str, &'a [FunctionConstant], bool);

/// `create_pipeline` の入力を分類する。device にも Metal の API にも触らない。
///
/// `Msl`・`MslSpecialized`・`MslWithOptions`・`MslSpecializedWithOptions` は取り出して返す
/// （`Msl`・`MslWithOptions` の function constant は空の並びで、末尾は `false`。
/// `MslSpecialized`・`MslSpecializedWithOptions` は並びが空でも末尾が `true`）。
/// WGSL は、この背後実装が提供しない操作として `Unsupported` にする。
/// ほかの言語の成果物は `ArtifactRejected { kind: "unsupported-input", .. }` にし、理由に受け取った
/// variant 名と受け付ける variant を英語で載せる。`PipelineInput` は `#[non_exhaustive]` なので、
/// 知らない variant も `Unsupported` にする（fail-closed）。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn classify_pipeline_input<'a>(
    input: &PipelineInput<'a>,
) -> Result<AcceptedPipelineInput<'a>, ContractError> {
    let unsupported = || ContractError::Unsupported {
        op: "create_pipeline",
        backend: BackendKind::Metal,
    };
    let rejected = |variant: &str| ContractError::ArtifactRejected {
        kind: "unsupported-input",
        reason: format!(
            "pipeline input {variant} is not accepted by the Metal backend; accepted inputs are Msl, MslSpecialized, MslWithOptions and MslSpecializedWithOptions"
        ),
    };
    match input {
        PipelineInput::Msl {
            source,
            entry_point,
        } => Ok((source, entry_point, &[], false)),
        PipelineInput::MslSpecialized {
            source,
            entry_point,
            function_constants,
        } => Ok((source, entry_point, function_constants, true)),
        PipelineInput::MslWithOptions {
            source,
            entry_point,
            ..
        } => Ok((source, entry_point, &[], false)),
        PipelineInput::MslSpecializedWithOptions {
            source,
            entry_point,
            function_constants,
            ..
        } => Ok((source, entry_point, function_constants, true)),
        PipelineInput::Wgsl { .. } => Err(unsupported()),
        PipelineInput::SpirV { .. } => Err(rejected("SpirV")),
        PipelineInput::CudaC { .. } => Err(rejected("CudaC")),
        PipelineInput::Ptx { .. } => Err(rejected("Ptx")),
        PipelineInput::Dxil { .. } => Err(rejected("Dxil")),
        _ => Err(unsupported()),
    }
}

/// `create_pipeline` の入力から読んだ、ライブラリのコンパイルのオプションに設定するもの。
///
/// 既定値（どちらも `false`）は「オプションを設定せず既定のオプションでコンパイルする」を表す。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
struct CompilePlan {
    /// 数学モードを `Safe` に設定する。
    safe_math: bool,
    /// 関数の精度を `Precise` に設定する。
    precise_functions: bool,
}

/// 入力から [`CompilePlan`] を読む。device にも Metal の API にも触らない。
///
/// `MslWithOptions` は数学モードだけ、`MslSpecializedWithOptions` は数学モードと関数の精度を読む。
/// ほかの入力は既定値を返す。`MslMathMode`・`MslFloatFunctions` の知らない値は `Unsupported`
/// （fail-closed。外の crate から知らない値は作れないので試験では通らない）。
/// `create_pipeline` から、入力の分類の次に呼ばれる。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn compile_plan(input: &PipelineInput<'_>) -> Result<CompilePlan, ContractError> {
    let unsupported = || ContractError::Unsupported {
        op: "create_pipeline",
        backend: BackendKind::Metal,
    };
    let safe = |mode: &MslMathMode| match mode {
        MslMathMode::Safe => Ok(true),
        MslMathMode::Fast => Ok(false),
        _ => Err(unsupported()),
    };
    match input {
        PipelineInput::MslWithOptions { math_mode, .. } => Ok(CompilePlan {
            safe_math: safe(math_mode)?,
            precise_functions: false,
        }),
        PipelineInput::MslSpecializedWithOptions {
            math_mode,
            float_functions,
            ..
        } => Ok(CompilePlan {
            safe_math: safe(math_mode)?,
            precise_functions: match float_functions {
                MslFloatFunctions::Precise => true,
                MslFloatFunctions::Fast => false,
                _ => return Err(unsupported()),
            },
        }),
        _ => Ok(CompilePlan::default()),
    }
}

/// `pipeline_cache_key` の `compiler_version`・`driver_version` に入れる値。
///
/// OS の版の文字列と device 名を NUL（`\0`）でつないだ文字列の `sha256_u64`。device にも Metal の API にも触らない。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn cache_key_version(os_version: &str, device_name: &str) -> u64 {
    sha256_u64(format!("{os_version}\0{device_name}").as_bytes())
}

/// 数学モードの語。知らない値（`#[non_exhaustive]`）は `None`。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn math_mode_word(mode: MslMathMode) -> Option<&'static str> {
    match mode {
        MslMathMode::Safe => Some("Safe"),
        MslMathMode::Fast => Some("Fast"),
        _ => None,
    }
}

/// 浮動小数点の関数の精度の語。知らない値（`#[non_exhaustive]`）は `None`。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn float_functions_word(functions: MslFloatFunctions) -> Option<&'static str> {
    match functions {
        MslFloatFunctions::Fast => Some("Fast"),
        MslFloatFunctions::Precise => Some("Precise"),
        _ => None,
    }
}

/// `entry=<入口名>` に、function constant ごとに `;idx=<番号>:U32(<値>)` か `;idx=<番号>:Bool(<値>)` を
/// 渡された順に続けた文字列。知らない値の型（`#[non_exhaustive]`）を含むと `None`。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn specialization_tag(entry_point: &str, constants: &[FunctionConstant]) -> Option<String> {
    let mut tag = format!("entry={entry_point}");
    for c in constants {
        let value = match c.value {
            FunctionConstantValue::U32(v) => format!("U32({v})"),
            FunctionConstantValue::Bool(b) => format!("Bool({b})"),
            _ => return None,
        };
        tag.push_str(&format!(";idx={}:{value}", c.index));
    }
    Some(tag)
}

/// `pipeline_cache_key` のキーを組み立てる。device にも Metal の API にも触らない。
///
/// MSL の 4 つの入力（`Msl`・`MslSpecialized`・`MslWithOptions`・`MslSpecializedWithOptions`）で `Some` を返し、
/// ほかの入力と、知らない数学モード・関数の精度・定数の値の型を含む入力は `None` を返す。
/// `compiler_id` は `"metal"`、`target` は `"metal-msl"`、`source_hash` はソースの `sha256_u64`。
/// `options_hash` は、`Msl`・`MslSpecialized` が `fast-math(default)`、`MslWithOptions` が `math_mode=<語>`、
/// `MslSpecializedWithOptions` が `math_mode=<語>;float_functions=<語>` の `sha256_u64`。
/// `specialization_hash` は `specialization_tag` の `sha256_u64`。`compiler_version` と `driver_version` は
/// どちらも `version`（`cache_key_version` の値）。`bounds_checked` は引数の値。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn metal_cache_key(
    input: &PipelineInput<'_>,
    bounds_checked: bool,
    version: u64,
) -> Option<CacheKey> {
    let (source, options, specialization) = match input {
        PipelineInput::Msl {
            source,
            entry_point,
        } => (
            *source,
            "fast-math(default)".to_string(),
            specialization_tag(entry_point, &[])?,
        ),
        PipelineInput::MslSpecialized {
            source,
            entry_point,
            function_constants,
        } => (
            *source,
            "fast-math(default)".to_string(),
            specialization_tag(entry_point, function_constants)?,
        ),
        PipelineInput::MslWithOptions {
            source,
            entry_point,
            math_mode,
        } => (
            *source,
            format!("math_mode={}", math_mode_word(*math_mode)?),
            specialization_tag(entry_point, &[])?,
        ),
        PipelineInput::MslSpecializedWithOptions {
            source,
            entry_point,
            function_constants,
            math_mode,
            float_functions,
        } => (
            *source,
            format!(
                "math_mode={};float_functions={}",
                math_mode_word(*math_mode)?,
                float_functions_word(*float_functions)?
            ),
            specialization_tag(entry_point, function_constants)?,
        ),
        _ => return None,
    };
    Some(CacheKey {
        source_hash: sha256_u64(source.as_bytes()),
        compiler_id: "metal",
        compiler_version: version,
        target: "metal-msl",
        options_hash: sha256_u64(options.as_bytes()),
        bounds_checked,
        specialization_hash: sha256_u64(specialization.as_bytes()),
        driver_version: Some(version),
    })
}

/// `layout.slots` に同じ番号が 2 つ以上あれば `InvalidArgument`（重複した最初の番号を載せる）。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn check_unique_slots(slots: &[u32]) -> Result<(), ContractError> {
    let mut seen = HashSet::new();
    for slot in slots {
        if !seen.insert(*slot) {
            return Err(ContractError::InvalidArgument(format!(
                "duplicate slot in binding layout: {slot}"
            )));
        }
    }
    Ok(())
}

/// dispatch の束縛・定数の slot と、共有メモリの index の上限（含む）。これを超える番号は拒否する。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const MAX_DISPATCH_SLOT: u32 = 30;

/// 定数 1 件の bytes の長さの上限（含む）。これを超えるものは拒否する。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const MAX_CONSTANT_BYTES: usize = 4096;

/// 共有メモリ 1 件の長さの倍数。この倍数でない長さは拒否する。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const THREADGROUP_MEMORY_ALIGN: u32 = 16;

/// 束縛のバッファの offset の倍数。`setBuffer_offset_atIndex` の offset の整列条件で、この倍数でない
/// offset は記録の前に拒否する。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const BUFFER_OFFSET_ALIGN: u64 = 4;

/// `check_dispatch_args` に渡す束縛 1 件の値（slot・offset・バッファの確保の長さ）。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[derive(Debug, Clone, Copy)]
struct BindingArg {
    slot: u32,
    offset: u64,
    buffer_len: u64,
}

/// `check_dispatch_args` に渡す定数 1 件の値（slot・bytes の長さ）。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[derive(Debug, Clone, Copy)]
struct ConstantArg {
    slot: u32,
    len: usize,
}

/// `check_dispatch_args` に渡す共有メモリ 1 件の値（index・bytes）。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[derive(Debug, Clone, Copy)]
struct ThreadgroupArg {
    index: u32,
    bytes: u32,
}

/// `check_dispatch_args` の入力。device とパイプラインから読んだ値を plain な型で持つ。
///
/// `MetalDevice::dispatch_ex` が表の照合の後に組み立てる。Objective-C の型を持たないので、どの OS でも試験できる。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[derive(Debug, Clone, Copy)]
struct DispatchArgs<'a> {
    dispatch_dims: [u32; 3],
    threads_per_threadgroup: [u32; 3],
    bindings: &'a [BindingArg],
    constants: &'a [ConstantArg],
    threadgroup_memory: &'a [ThreadgroupArg],
    layout_slots: &'a [u32],
    pipeline_max_total_threads: u64,
    pipeline_static_threadgroup_memory: u64,
    device_max_threadgroup_memory: u64,
    device_max_threads_per_threadgroup: [u64; 3],
    max_dispatch_dims: [u32; 3],
}

/// dispatch の引数を記録の前に検査する。最初に当たった項目を `InvalidArgument` で返し、副作用は持たない。
///
/// 順は、`dispatch_dims`、`threads_per_threadgroup`、slot、束縛の offset（長さ、4 の倍数の順）、定数、共有メモリ。
/// 足し算・掛け算は `checked_*` で行い、あふれも拒否する。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn check_dispatch_args(args: &DispatchArgs<'_>) -> Result<(), ContractError> {
    let invalid = |msg: String| Err(ContractError::InvalidArgument(msg));

    if args.dispatch_dims.contains(&0) {
        return invalid("dispatch_dims contains zero".into());
    }
    if args
        .dispatch_dims
        .iter()
        .zip(args.max_dispatch_dims)
        .any(|(dim, max)| *dim > max)
    {
        return invalid("dispatch_dims exceeds the limit".into());
    }

    if args.threads_per_threadgroup.contains(&0) {
        return invalid("threads_per_threadgroup contains zero".into());
    }
    if args
        .threads_per_threadgroup
        .iter()
        .zip(args.device_max_threads_per_threadgroup)
        .any(|(threads, max)| u64::from(*threads) > max)
    {
        return invalid("threads_per_threadgroup exceeds the device limit".into());
    }
    let total = args
        .threads_per_threadgroup
        .iter()
        .try_fold(1_u64, |acc, t| acc.checked_mul(u64::from(*t)));
    match total {
        Some(total) if total <= args.pipeline_max_total_threads => {}
        _ => return invalid("threads_per_threadgroup exceeds the pipeline thread limit".into()),
    }

    let mut used = HashSet::new();
    let slots = args
        .bindings
        .iter()
        .map(|b| b.slot)
        .chain(args.constants.iter().map(|c| c.slot));
    for slot in slots {
        if slot > MAX_DISPATCH_SLOT {
            return invalid(format!("slot {slot} exceeds the maximum slot"));
        }
        if !used.insert(slot) {
            return invalid(format!("duplicate slot in dispatch: {slot}"));
        }
        if !args.layout_slots.contains(&slot) {
            return invalid(format!("slot {slot} is not in the binding layout"));
        }
    }
    if let Some(slot) = args.layout_slots.iter().find(|s| !used.contains(s)) {
        return invalid(format!("slot {slot} of the binding layout is not provided"));
    }

    if let Some(b) = args.bindings.iter().find(|b| b.offset > b.buffer_len) {
        return invalid(format!(
            "binding slot {} offset exceeds the buffer length",
            b.slot
        ));
    }
    if let Some(b) = args
        .bindings
        .iter()
        .find(|b| b.offset % BUFFER_OFFSET_ALIGN != 0)
    {
        return invalid(format!(
            "binding slot {} offset is not a multiple of {BUFFER_OFFSET_ALIGN}",
            b.slot
        ));
    }

    for c in args.constants {
        if c.len == 0 {
            return invalid(format!("constant slot {} is empty", c.slot));
        }
        if c.len > MAX_CONSTANT_BYTES {
            return invalid(format!(
                "constant slot {} exceeds the maximum length",
                c.slot
            ));
        }
    }

    let mut seen = HashSet::new();
    let mut total = args.pipeline_static_threadgroup_memory;
    for t in args.threadgroup_memory {
        if t.index > MAX_DISPATCH_SLOT {
            return invalid(format!(
                "threadgroup memory index {} exceeds the maximum index",
                t.index
            ));
        }
        if !seen.insert(t.index) {
            return invalid(format!("duplicate threadgroup memory index: {}", t.index));
        }
        if t.bytes % THREADGROUP_MEMORY_ALIGN != 0 {
            return invalid(format!(
                "threadgroup memory index {} length is not a multiple of {THREADGROUP_MEMORY_ALIGN}",
                t.index
            ));
        }
        total = match total.checked_add(u64::from(t.bytes)) {
            Some(total) => total,
            None => return invalid("threadgroup memory total overflows".into()),
        };
    }
    if total > args.device_max_threadgroup_memory {
        return invalid("threadgroup memory exceeds the device limit".into());
    }
    Ok(())
}

/// 記録した束縛 1 件。バッファは識別子と払い出しの世代で指し、Objective-C の型を持たない。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
struct RecordedBinding {
    slot: u32,
    buffer_id: u64,
    generation: u64,
    offset: u64,
}

/// dispatch の実行を許すか。`check_dispatch_args` は引数の数値と宣言の配置だけを見て、カーネルが実際に触る範囲は
/// 保証しない。その保証（シェーダーの検証または実行時の範囲の制限）が入るまで、製品のビルドでは false にして
/// 記録を含む投入を `Unsupported` で拒む。この crate の単体試験のビルドだけが、積み込みの形を確かめるために true にする。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const DISPATCH_EXECUTION_ENABLED: bool = cfg!(test);

/// 記録した dispatch 1 件。`dispatch_ex` が検査の後に積み、投入の時に取り出して compute encoder に積む。
///
/// 定数の bytes は写しを持つ。Objective-C の型を持たないので、記録先の表は `Send + Sync` のままになる。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
struct DispatchRecord {
    pipeline: u64,
    bindings: Vec<RecordedBinding>,
    constants: Vec<(u32, Vec<u8>)>,
    dispatch_dims: [u32; 3],
    threads_per_threadgroup: [u32; 3],
    threadgroup_memory: Vec<(u32, u32)>,
}

/// 投入の時の束縛のバッファの確かめ。全記録の全束縛を並びの順に見て、最初に外れたものを `InvalidArgument` で返す。
///
/// 外れるのは、識別子が表に無い（`lookup` が `None`）・表の項目の世代が記録の世代と違う・記録の世代が
/// 今の世代と違うとき（COMP-39）。メッセージに束縛の slot の番号を載せる。副作用は持たない。
/// `submit`・`submit_many` が積む前に呼び、1 件でも外れれば何も積まない。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn check_recorded_buffers(
    records: &[Vec<DispatchRecord>],
    lookup: impl Fn(u64) -> Option<u64>,
    current_generation: u64,
) -> Result<(), ContractError> {
    for binding in records.iter().flatten().flat_map(|r| r.bindings.iter()) {
        let alive = binding.generation == current_generation
            && lookup(binding.buffer_id) == Some(binding.generation);
        if !alive {
            return Err(ContractError::InvalidArgument(format!(
                "a bound buffer was freed before submit (binding slot {})",
                binding.slot
            )));
        }
    }
    Ok(())
}

/// 記録先ごとの記録の列を、compute encoder 1 つ分の並び（run）に分ける純粋な計画。
///
/// 各要素は（記録, `setComputePipelineState` を呼ぶか）。空の記録先は run を作らず、記録先どうしは run を共有しない。
/// run の中で直前の記録とパイプラインが同じなら呼ばない。それまでの記録が設定した共有メモリの index の
/// どれかを次の記録が持たないときは、その記録の前で run を切り、新しい run の先頭として呼ぶ。
/// 記録を run に移して返すので、実行する側は run ごとに開く・各記録を積む・閉じるを順に回すだけでよい。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn encode_plan<T>(
    groups: Vec<Vec<T>>,
    pipeline_of: impl Fn(&T) -> u64,
    threadgroup_indices_of: impl Fn(&T) -> &[u32],
) -> Vec<Vec<(T, bool)>> {
    let mut runs = Vec::new();
    for group in groups {
        let mut run: Vec<(T, bool)> = Vec::new();
        let mut last_pipeline = 0;
        let mut set_indices: HashSet<u32> = HashSet::new();
        for record in group {
            let indices = threadgroup_indices_of(&record);
            let covers = set_indices.iter().all(|i| indices.contains(i));
            let pipeline = pipeline_of(&record);
            let set_pipeline = if run.is_empty() {
                true
            } else if covers {
                pipeline != last_pipeline
            } else {
                runs.push(std::mem::take(&mut run));
                set_indices.clear();
                true
            };
            set_indices.extend(indices.iter().copied());
            last_pipeline = pipeline;
            run.push((record, set_pipeline));
        }
        if !run.is_empty() {
            runs.push(run);
        }
    }
    runs
}

/// function constant の `index` の上限（含む）。MSL 4.1 仕様の 5.8 節（Program Scope Function Constants）が
/// 許す範囲 0〜65535 の上端で、Apple の公開文書に基づく。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const MAX_FUNCTION_CONSTANT_INDEX: u32 = 65535;

/// function constant の `index` が Metal の許す範囲（0〜`MAX_FUNCTION_CONSTANT_INDEX`）を超えるものがあれば
/// `InvalidArgument`（最初の範囲外の番号を載せる）。Metal の API を呼ぶ前に、重複の検査より先に行う。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn check_constant_index_range(constants: &[FunctionConstant]) -> Result<(), ContractError> {
    match constants
        .iter()
        .find(|c| c.index > MAX_FUNCTION_CONSTANT_INDEX)
    {
        Some(c) => Err(ContractError::InvalidArgument(format!(
            "function constant index out of range: {} (max {MAX_FUNCTION_CONSTANT_INDEX})",
            c.index
        ))),
        None => Ok(()),
    }
}

/// function constant の `index` に重複があれば `InvalidArgument`（重複した最初の番号を載せる）。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn check_unique_constants(constants: &[FunctionConstant]) -> Result<(), ContractError> {
    let mut seen = HashSet::new();
    for constant in constants {
        if !seen.insert(constant.index) {
            return Err(ContractError::InvalidArgument(format!(
                "duplicate function constant index: {}",
                constant.index
            )));
        }
    }
    Ok(())
}

/// function constant の値の種別。Metal の `MTLDataType` の `UInt`・`Bool` に対応する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
enum ConstantKind {
    /// 32 ビット符号なし整数（`MTLDataType::UInt`。4 バイト）。
    UInt,
    /// 真偽値（`MTLDataType::Bool`。1 バイト）。
    Bool,
}

impl ConstantKind {
    /// この種別の値が Metal に渡るときのバイト数。
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    const fn byte_len(self) -> usize {
        match self {
            Self::UInt => 4,
            Self::Bool => 1,
        }
    }
}

/// function constant の値を（種別, ネイティブエンディアンのバイト列）に写す。
///
/// `FunctionConstantValue` は `#[non_exhaustive]` なので、知らない型は `Unsupported`（fail-closed）。
/// 返すバイト列の長さは種別の `byte_len` と一致する（`Bool` は偽が 0、真が 1）。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn constant_wire(value: &FunctionConstantValue) -> Result<(ConstantKind, Vec<u8>), ContractError> {
    match value {
        FunctionConstantValue::U32(v) => Ok((ConstantKind::UInt, v.to_ne_bytes().to_vec())),
        FunctionConstantValue::Bool(v) => Ok((ConstantKind::Bool, vec![u8::from(*v)])),
        _ => Err(ContractError::Unsupported {
            op: "create_pipeline",
            backend: BackendKind::Metal,
        }),
    }
}

/// function constant の並びを、重複の検査と値の写しを済ませた（番号, 種別, バイト列）の並びにする。
/// Metal の API を呼ぶ前に、`create_pipeline` から呼ばれる。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn prepare_constants(
    constants: &[FunctionConstant],
) -> Result<Vec<(u32, ConstantKind, Vec<u8>)>, ContractError> {
    check_constant_index_range(constants)?;
    check_unique_constants(constants)?;
    constants
        .iter()
        .map(|c| constant_wire(&c.value).map(|(kind, bytes)| (c.index, kind, bytes)))
        .collect()
}

/// 関数が宣言する function constant の（番号, 必須か）の並びと、渡した番号から、足りない必須の番号を
/// 小さい順に重複なしで返す。Metal に触らない純粋な関数で、`create_pipeline` が特殊化の前に呼ぶ。
///
/// 必須でない宣言は渡されなくてよい。宣言に無い番号を渡しても咎めない（Metal はそれを無視する）。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn missing_required_constants(declared: &[(usize, bool)], provided: &[u32]) -> Vec<usize> {
    let mut missing: Vec<usize> = declared
        .iter()
        .filter(|(index, required)| {
            *required
                && !provided
                    .iter()
                    .any(|p| usize::try_from(*p).is_ok_and(|p| p == *index))
        })
        .map(|(index, _)| *index)
        .collect();
    missing.sort_unstable();
    missing.dedup();
    missing
}

/// 足りない必須の番号（空でない）から、`ArtifactRejected { kind: "msl" }` の理由の文を作る。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn missing_required_reason(missing: &[usize]) -> String {
    let list = missing
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    format!("required function constant at index {list} was not provided")
}

/// 世代の源が尽きて、ロストの世代を取り直せなかったことを表す。状態は変わっていない。
#[cfg(feature = "fault-injection")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
struct GenerationExhausted;

/// 模擬のロストの状態。最初のロストの理由と、登録されたコールバック 1 つを錠の中に持つ。
///
/// device に触らない純粋な型で、Metal の device の型 `MetalDevice` が 1 つ持つ（TASK-2932、DEV-80、TASK-271）。
/// 世代の格納先と取り直しの源は呼び出し側が渡すので、device が無い環境でも単体で確かめられる。
/// `Drop` ではコールバックを呼ばない。
#[cfg(feature = "fault-injection")]
#[derive(Default)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
struct LostState {
    inner: Mutex<LostInner>,
}

#[cfg(feature = "fault-injection")]
#[derive(Default)]
struct LostInner {
    /// 最初のロストの理由。`Some` になったら 2 回目以降のロストは無視する。
    reason: Option<DeviceLostReason>,
    /// 登録されたコールバック。ロストで `take` されるので、ロストの後に登録したものは呼ばれない。
    callback: Option<LostCallback>,
}

#[cfg(feature = "fault-injection")]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
impl LostState {
    /// 毒化していても中身を取り出す。panic しない。
    fn lock(&self) -> MutexGuard<'_, LostInner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// コールバックを登録する。登録済みなら上書きし、ロスト済みなら保持するだけで呼ばない。
    fn set_callback(&self, callback: LostCallback) {
        // 上書きされた古いコールバックの `Drop` が再入しても詰まらないよう、錠を放してから捨てる。
        let old = self.lock().callback.replace(callback);
        drop(old);
    }

    /// 模擬のロストが起きたか。単体の試験だけが読む。
    #[cfg(test)]
    fn is_lost(&self) -> bool {
        self.lock().reason.is_some()
    }

    /// 1 回目だけ、理由の記録・世代の取り直し・コールバックの取り出しを錠の中で行い、錠を放してから
    /// 指定の理由でコールバックを 1 回呼ぶ。1 回目なら `Ok(true)`、2 回目以降は何もせず `Ok(false)`。
    ///
    /// 世代は原子の値として読まれる。理由とコールバックは錠の中だけで読む。
    ///
    /// 世代は `source` から 1 つ取って `generation` へ書く。`source` が尽きていたら、理由も世代も
    /// コールバックも変えずに `Err` を返す。コールバックの中から同じ状態を呼んでも詰まらない。
    fn lose(
        &self,
        reason: DeviceLostReason,
        generation: &AtomicU64,
        source: &AtomicU64,
    ) -> Result<bool, GenerationExhausted> {
        let callback = {
            let mut inner = self.lock();
            if inner.reason.is_some() {
                return Ok(false);
            }
            let next = take_next(source).ok_or(GenerationExhausted)?;
            generation.store(next, Ordering::Release);
            inner.reason = Some(reason.clone());
            inner.callback.take()
        };
        if let Some(callback) = callback {
            callback(reason);
        }
        Ok(true)
    }
}

/// 駆動してから、照合と払い出しに使う世代を読み直して返す。
///
/// 駆動は利用者の callback を同期で呼ぶので、その中から `simulate_device_lost` へ再入すると世代が変わり得る。
/// 駆動の前に読んだ値で照合や払い出しをすると、成功して返したハンドルや印が直後の操作で世代不一致になる。
/// そこで、駆動には駆動の前に読んだ世代 `g0` を渡し、照合と払い出しには駆動の後の値を使う
/// （1 つの操作で世代を読むのは、駆動に 1 回、照合と払い出しに 1 回）。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn driven_generation(drive: impl FnOnce(u64), read: impl Fn() -> u64) -> u64 {
    drive(read());
    read()
}

/// 表から、識別子が `id` で払い出しの世代が `generation` の項目だけを外して返す。
///
/// 識別子は払い出しのたびに新しいので、一致する項目はそのハンドルを払い出したときの項目だけ。
/// 世代が違う・表に無い識別子は何も外さない。`free` が使う（ロストの後でも、ロスト前のハンドルで外せる）。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn remove_issued<T>(
    map: &mut HashMap<u64, T>,
    id: u64,
    generation: u64,
    issued_generation: impl Fn(&T) -> u64,
) -> Option<T> {
    match map.get(&id) {
        Some(entry) if issued_generation(entry) == generation => map.remove(&id),
        _ => None,
    }
}

/// コマンドバッファ 1 つの状態。`MTLCommandBufferStatus` を写す。試験では偽の値を使う。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
enum BufState {
    /// 完了した。
    Done,
    /// `Error` で終わった。
    Failed,
    /// 未完了。
    Running,
}

/// 1 回の投入（コマンドバッファの列）の状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
enum GroupState {
    Pending,
    Completed,
    Failed,
}

/// 列のどれかが `Failed` なら失敗、全部が `Done` なら完了、他は未完了とする。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn group_state_of(states: impl IntoIterator<Item = BufState>) -> GroupState {
    let mut all_done = true;
    for state in states {
        match state {
            BufState::Failed => return GroupState::Failed,
            BufState::Running => all_done = false,
            BufState::Done => {}
        }
    }
    if all_done {
        GroupState::Completed
    } else {
        GroupState::Pending
    }
}

/// 時刻の表が持つ件数の上限。超えたら通し番号の小さいものから捨てる。
#[cfg(any(test, feature = "native-blocking-wait"))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const TIMING_TABLE_MAX: usize = 1024;

/// コマンドバッファの列の (開始, 終了)（秒）から、投入全体の (最小の開始, 最大の終了) を求める。
///
/// 空の列・有限でない値・0 以下の値（Metal は時刻を得られないとき 0 を返す）・終了が開始より前の組があれば
/// `None`（時刻として残さない）。
#[cfg(any(test, feature = "native-blocking-wait"))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn span_of(times: impl IntoIterator<Item = (f64, f64)>) -> Option<(f64, f64)> {
    let mut span: Option<(f64, f64)> = None;
    for (start, end) in times {
        if !start.is_finite() || !end.is_finite() || start <= 0.0 || end <= 0.0 || end < start {
            return None;
        }
        span = Some(match span {
            None => (start, end),
            Some((s, e)) => (s.min(start), e.max(end)),
        });
    }
    span
}

/// `on_complete` に渡された callback。native 版の `CommandQueue::on_complete` の引数と同じ型。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
type Callback = Box<dyn FnOnce(CompletionState) + Send>;

/// 投入済みの表と、`on_complete` の登録の表。投入の通し番号は device ごとに 1 から払い出す。
///
/// `G` は 1 回の投入が持つ列の型で、本物では Objective-C のコマンドバッファの列、試験では偽の状態の列。
/// `C` は登録された callback の型。
///
/// 登録の表と投入済みの表は同じ錠の中に置く。device のほかの錠（`buffers`・`pipelines`・`open`）とは、
/// バッファ -> パイプライン -> 記録先 -> 投入済みの順にだけ入れ子で取る（逆の向きでは取らない）。callback はどの錠も握らずに呼ぶ。
/// 登録の判定は `state_of` が行い、`pending` から回収で外れた番号でも watermark と失敗の集合から
/// 終端の値が決まるので、登録の表は `pending` と独立に持てる。
/// callback は必ず表から外して錠を放してから呼ぶ。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
struct Submissions<G, C = Callback> {
    /// 次に払い出す通し番号。
    next_sequence: u64,
    /// この番号以下は回収済み（失敗の集合にあるものを除き完了）。
    watermark: u64,
    /// 通し番号から列への表。回収されるまで保持する。
    pending: BTreeMap<u64, G>,
    /// 失敗として回収した通し番号。watermark 以下の失敗の番号だけを持つので、増えるのは失敗の件数だけ。
    failed: HashSet<u64>,
    /// 通し番号から (開始, 終了)（秒）への表。書くのは完了で起きる待ち（`wait_blocking(None)`）が完了を検出した時だけ。
    /// `TIMING_TABLE_MAX` 件を超えたら通し番号の最も小さいものから捨てる。
    timings: BTreeMap<u64, (f64, f64)>,
    /// 未完了のまま登録された callback。(通し番号, 登録したときの印の世代, callback) を登録の順に持つ。
    /// 呼ぶときに表から外すので、どれも 1 回しか呼ばれない。
    /// 世代が device の今の世代と違う項目は、Metal の完了状態に関わらず `Lost` で呼ぶ（`poll` と同じ値）。
    callbacks: Vec<(u64, u64, C)>,
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
impl<G, C> Submissions<G, C> {
    /// 何も投入していない表。
    fn new() -> Self {
        Self {
            next_sequence: 1,
            watermark: 0,
            pending: BTreeMap::new(),
            failed: HashSet::new(),
            timings: BTreeMap::new(),
            callbacks: Vec::new(),
        }
    }

    /// 時刻を表に書く。上限を超えた分は通し番号の最も小さいものから捨てる。
    #[cfg(any(test, feature = "native-blocking-wait"))]
    fn record_timing(&mut self, sequence: u64, start_seconds: f64, end_seconds: f64) {
        self.timings.insert(sequence, (start_seconds, end_seconds));
        while self.timings.len() > TIMING_TABLE_MAX {
            self.timings.pop_first();
        }
    }

    /// 完了を待ち終えた時の時刻の扱い。`record` が真で `span` が `Some` の時だけ表へ書き、書いたかを返す。
    ///
    /// `wait_blocking(None)` だけが `record = true` で呼ぶ。期限付きの待ちと `on_complete` は呼ばない。
    /// `span` は完了を確かめた列の `span_of` の値で、未完了・不正な時刻なら `None`。
    #[cfg(any(test, feature = "native-blocking-wait"))]
    fn complete_wait(&mut self, sequence: u64, span: Option<(f64, f64)>, record: bool) -> bool {
        match (record, span) {
            (true, Some((start, end))) => {
                self.record_timing(sequence, start, end);
                true
            }
            _ => false,
        }
    }

    /// 通し番号の GPU 側の時刻の問合せ。表と投入済みの表を見るだけで、回収も状態の変更もしない。
    ///
    /// 時刻の表にあれば `Available`。無ければ、投入済みの表にあるものは未完了が `Pending`、
    /// 時刻を残さずに完了したものが `Reclaimed`、失敗が `Unavailable`。表から外れた番号は、失敗の集合にあれば
    /// `Unavailable`、watermark 以下の他は（時刻を残さずに完了した・表から捨てられたものとして）`Reclaimed`。
    /// 0 と払い出していない番号は `Unavailable`。
    fn timing_of(&self, sequence: u64, classify: impl Fn(&G) -> GroupState) -> GpuTiming {
        if let Some(&(start_seconds, end_seconds)) = self.timings.get(&sequence) {
            return GpuTiming::Available {
                start_seconds,
                end_seconds,
            };
        }
        if let Some(group) = self.pending.get(&sequence) {
            return match classify(group) {
                GroupState::Pending => GpuTiming::Pending,
                GroupState::Completed => GpuTiming::Reclaimed,
                GroupState::Failed => GpuTiming::Unavailable,
            };
        }
        if self.failed.contains(&sequence) {
            return GpuTiming::Unavailable;
        }
        if sequence >= 1 && sequence <= self.watermark {
            GpuTiming::Reclaimed
        } else {
            GpuTiming::Unavailable
        }
    }

    /// callback を登録する。回収はしない（登録は駆動の呼び出しではない）。
    ///
    /// 番号の状態が終端なら登録せず、callback と状態を返す。呼ぶのは錠を放してから。
    /// `Pending` なら表に積んで `None`。
    fn register(
        &mut self,
        sequence: u64,
        generation: u64,
        current_generation: u64,
        callback: C,
        classify: impl Fn(&G) -> GroupState,
    ) -> Option<(C, CompletionState)> {
        // 登録の世代が今より小さければ、ロストの掃き出しがすでに済んでいる。積まずにその場で `Lost` を返す。
        if generation < current_generation {
            return Some((callback, CompletionState::Lost));
        }
        match self.state_of(sequence, classify) {
            CompletionState::Pending => {
                self.callbacks.push((sequence, generation, callback));
                None
            }
            done => Some((callback, done)),
        }
    }

    /// 回収してから、終端になった登録を表から外して返す。`Pending` のものは残す。
    ///
    /// 先頭が未完了で回収が止まっていても、後ろの完了済みの列の登録は外れる。呼ぶのは錠を放してから。
    /// 登録の件数に比例して走査する。判定（`classify`）をすべて済ませてから外すので、判定が panic しても登録は失われない。
    fn take_terminal(
        &mut self,
        current_generation: u64,
        classify: impl Fn(&G) -> GroupState + Copy,
    ) -> Vec<(C, u64, CompletionState)> {
        // 回収が止まった先頭は再分類せず `Pending` に残す（間に完了が挟まっても、表に残したまま配らない）。
        let stalled = self.reclaim_observing(classify);
        let states: Vec<CompletionState> = self
            .callbacks
            .iter()
            .map(|(sequence, generation, _)| {
                if stalled == Some(*sequence) && *generation == current_generation {
                    return CompletionState::Pending;
                }
                self.delivery_state(*sequence, *generation, current_generation, classify)
            })
            .collect();
        let mut ready = Vec::new();
        let mut kept = Vec::new();
        for ((sequence, generation, callback), state) in
            std::mem::take(&mut self.callbacks).into_iter().zip(states)
        {
            match state {
                CompletionState::Pending => kept.push((sequence, generation, callback)),
                done => ready.push((callback, generation, done)),
            }
        }
        self.callbacks = kept;
        ready
    }

    /// 登録の配る値。世代の源は単調に増えるので、古い扱いは登録の世代が比べる世代より小さいときだけ。
    ///
    /// - 登録の世代 < 比べる世代: 列の状態に関わらず `Lost`（その token の `poll` と同じ値）
    /// - 登録の世代 > 比べる世代: 読んだ世代のほうが古い。触れずに `Pending`（次の駆動に残す）
    /// - 等しい: 列の状態
    fn delivery_state(
        &self,
        sequence: u64,
        generation: u64,
        current_generation: u64,
        classify: impl Fn(&G) -> GroupState,
    ) -> CompletionState {
        if generation < current_generation {
            return CompletionState::Lost;
        }
        if generation > current_generation {
            return CompletionState::Pending;
        }
        self.state_of(sequence, classify)
    }

    /// 世代が今より小さい登録を全部外し、`Lost` を付けて返す。同じ世代と新しい世代の登録は残す。呼ぶのは錠を放してから。
    ///
    /// ロストの後に、まだ呼ばれていない完了の callback を `poll` と同じ値で呼ぶために使う。
    #[cfg(any(test, feature = "fault-injection"))]
    fn take_stale(&mut self, current_generation: u64) -> Vec<(C, u64, CompletionState)> {
        let mut stale = Vec::new();
        let mut kept = Vec::new();
        for (sequence, generation, callback) in std::mem::take(&mut self.callbacks) {
            if generation < current_generation {
                stale.push((callback, generation, CompletionState::Lost));
            } else {
                kept.push((sequence, generation, callback));
            }
        }
        self.callbacks = kept;
        stale
    }

    /// 登録されている番号の列の完了を `wait` で待つ。`Drop` が残りを呼び切る前に使う。
    fn wait_registered(&self, wait: impl Fn(&G)) {
        for (sequence, _, _) in &self.callbacks {
            if let Some(group) = self.pending.get(sequence) {
                wait(group);
            }
        }
    }

    /// 登録を全部外して返す。待ったあとでも `Pending` の番号は `Lost`（起きないはずの経路の fail-closed）。
    ///
    /// 判定を済ませてから外すので、判定が panic しても登録は失われない。
    fn take_all(
        &mut self,
        current_generation: u64,
        classify: impl Fn(&G) -> GroupState + Copy,
    ) -> Vec<(C, u64, CompletionState)> {
        self.reclaim(classify);
        let states: Vec<CompletionState> = self
            .callbacks
            .iter()
            .map(|(sequence, generation, _)| {
                // 最後の掃き出しなので、新しい世代の登録も今の世代として通常の判定にする。
                let compared = (*generation).max(current_generation);
                terminal_state(self.delivery_state(*sequence, *generation, compared, classify))
            })
            .collect();
        std::mem::take(&mut self.callbacks)
            .into_iter()
            .zip(states)
            .map(|((_, generation, callback), state)| (callback, generation, state))
            .collect()
    }

    /// 通し番号の昇順に、決着した先頭から表を外して watermark を進める。未完了で止まる。
    ///
    /// 先頭が未完了なら、後ろが完了していても回収しない。失敗は失敗の集合に移す（完了として数えない）。
    fn reclaim(&mut self, classify: impl Fn(&G) -> GroupState) {
        let _ = self.reclaim_observing(classify);
    }

    /// `reclaim` と同じ。回収が止まった先頭の通し番号（そのとき `Pending` と分類した列）を返す。
    ///
    /// 呼び出しの中で返す状態は、この分類から決める。Metal のコマンドバッファの状態は呼ぶたびに読み直すので、
    /// 回収のあとに同じ列を分類し直すと、間に完了が挟まって「`Complete` を返したのに表に残る」ことが起きる。
    /// 返した番号の列は `Pending` として答えれば、非 `Pending` を返した番号（先頭まで決着済み）は必ず表から外れている。
    /// 先頭より後ろの列は回収しない設計のままで、後ろの列の状態は答える側で分類する。
    fn reclaim_observing(&mut self, classify: impl Fn(&G) -> GroupState) -> Option<u64> {
        while let Some(entry) = self.pending.first_entry() {
            let sequence = *entry.key();
            match classify(entry.get()) {
                GroupState::Pending => return Some(sequence),
                GroupState::Completed => {
                    entry.remove();
                }
                GroupState::Failed => {
                    entry.remove();
                    self.failed.insert(sequence);
                }
            }
            self.watermark = sequence;
        }
        None
    }

    /// 回収してから通し番号の完了状態を答える（`poll` の錠の中の本体）。
    ///
    /// 回収が `Pending` で止まった先頭の番号は、再分類せずに `Pending` と答える。
    fn reclaim_and_state_of(
        &mut self,
        sequence: u64,
        classify: impl Fn(&G) -> GroupState,
    ) -> CompletionState {
        if self.reclaim_observing(&classify) == Some(sequence) {
            return CompletionState::Pending;
        }
        self.state_of(sequence, classify)
    }

    /// 回収してから通し番号の拡張した完了状態を答える（`poll_ex` の錠の中の本体）。`reclaim_and_state_of` と同じ規則。
    fn reclaim_and_state_ex_of(
        &mut self,
        sequence: u64,
        classify: impl Fn(&G) -> GroupState,
    ) -> CompletionStateEx {
        if self.reclaim_observing(&classify) == Some(sequence) {
            return CompletionStateEx::Pending;
        }
        self.state_ex_of(sequence, classify)
    }

    /// 通し番号の投入が終わっているか（完了か失敗。走っているあいだは偽）。
    ///
    /// 読み取りだけで、回収も世代の照合もしない。答えは表の列の状態・watermark・失敗の集合で決まる。
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    fn is_finished(&self, sequence: u64, classify: impl Fn(&G) -> GroupState) -> bool {
        self.state_of(sequence, classify) != CompletionState::Pending
    }

    /// 通し番号の完了状態。呼ぶ前に `reclaim` しておく。
    ///
    /// 表にあれば列の状態、無ければ失敗の集合にあるものと未払い出しの番号（0 を含む）は `Lost`、
    /// watermark 以下の他は `Complete`。
    fn state_of(&self, sequence: u64, classify: impl Fn(&G) -> GroupState) -> CompletionState {
        if let Some(group) = self.pending.get(&sequence) {
            return match classify(group) {
                GroupState::Pending => CompletionState::Pending,
                GroupState::Completed => CompletionState::Complete,
                GroupState::Failed => CompletionState::Lost,
            };
        }
        if self.failed.contains(&sequence) {
            return CompletionState::Lost;
        }
        if sequence >= 1 && sequence <= self.watermark {
            CompletionState::Complete
        } else {
            CompletionState::Lost
        }
    }

    /// 通し番号の拡張した完了状態。呼ぶ前に `reclaim` しておく。
    ///
    /// 表にあれば列の状態（失敗は `RuntimeError`）、失敗の集合にあるものは `RuntimeError`、
    /// watermark 以下の他は `Complete`。表にも集合にも無く watermark を超える番号（0 と未払い出しを含む）は
    /// `Unknown` で、`Complete` とは返さない。
    fn state_ex_of(&self, sequence: u64, classify: impl Fn(&G) -> GroupState) -> CompletionStateEx {
        if let Some(group) = self.pending.get(&sequence) {
            return match classify(group) {
                GroupState::Pending => CompletionStateEx::Pending,
                GroupState::Completed => CompletionStateEx::Complete,
                GroupState::Failed => CompletionStateEx::RuntimeError,
            };
        }
        if self.failed.contains(&sequence) {
            return CompletionStateEx::RuntimeError;
        }
        if sequence >= 1 && sequence <= self.watermark {
            CompletionStateEx::Complete
        } else {
            CompletionStateEx::Unknown
        }
    }
}

/// 開いている記録先の識別子と、その記録先に積んだ dispatch の記録の表。`CommandEncoder` の後始末の hook を兼ねる。
///
/// 記録は Objective-C の型を持たない plain な値なので `Send + Sync` で、記録先が device より長く生きても安全に使える。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
struct OpenEncoders {
    ids: Mutex<HashMap<u64, Vec<DispatchRecord>>>,
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
impl OpenEncoders {
    fn new() -> Self {
        Self {
            ids: Mutex::new(HashMap::new()),
        }
    }

    /// 表の錠を取る。更新は挿入・積み・削除だけなので、汚れていても中身を使う。
    fn lock(&self) -> MutexGuard<'_, HashMap<u64, Vec<DispatchRecord>>> {
        self.ids.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 渡された識別子が開いている表にあるかを、表を変えずに返す（0 は表に入らないので偽）。
    fn is_open(&self, id: u64) -> bool {
        self.lock().contains_key(&id)
    }

    /// 開いている記録先の記録の列の末尾に `record` を積む。
    ///
    /// 開いていなければ（0 を含む）`InvalidArgument` で、表は変えない（記録先の項目は作らない）。
    fn record(&self, id: u64, record: DispatchRecord) -> Result<(), ContractError> {
        match self.lock().get_mut(&id) {
            Some(records) => {
                records.push(record);
                Ok(())
            }
            None => Err(ContractError::InvalidArgument(
                "encoder is not open on this device".into(),
            )),
        }
    }

    /// 渡された識別子が全部開いている表にあることを確かめ、あれば表から外して記録の列を渡した順に返す。
    ///
    /// 1 つでも無い（0 を含む）・同じ識別子が重複していれば `InvalidArgument` で、表は変えない。
    fn claim(&self, ids: &[u64]) -> Result<Vec<Vec<DispatchRecord>>, ContractError> {
        let mut open = self.lock();
        let mut seen = HashSet::with_capacity(ids.len());
        for id in ids {
            if !open.contains_key(id) || !seen.insert(*id) {
                return Err(ContractError::InvalidArgument(
                    "encoder is not open on this device".into(),
                ));
            }
        }
        Ok(ids
            .iter()
            .map(|id| open.remove(id).unwrap_or_default())
            .collect())
    }
}

impl __BackendSharedEncoderCloser for OpenEncoders {
    /// 投入されずに破棄された記録先を、積んだ記録ごと表から外す。無ければ何もしない。
    fn close(&self, id: u64) {
        self.lock().remove(&id);
    }
}

/// 使わない panic の中身を捨てる。中身の `Drop` が panic しても、そこから巻き戻らない。
///
/// 破棄で出た panic の中身も `forget` で捨てる（その破棄から呼び出し元へ巻き戻らないため。漏れるのは panic の中身だけで、
/// メモリ安全には触れない）。残りの callback を呼び切る経路と `Drop` の経路が、破棄で途切れないために通す。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn discard_panic(payload: Box<dyn std::any::Any + Send>) {
    if let Err(inner) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(payload))) {
        std::mem::forget(inner);
    }
}

/// 外した callback を全部呼ぶ。1 つが panic しても残りを呼び切り、最初の panic の中身を返す。
///
/// 各 callback を呼ぶ直前に `read_generation` で今の世代を読み直し、登録の世代がそれより小さければ、
/// 先に決めた状態に関わらず `Lost` を渡す。先の callback からロストに再入しても、表の外に出た後ろの登録が
/// 古い判定の `Complete` を受け取らないため。世代が等しいか大きければ、決めた状態のまま渡す。
///
/// 2 個目以降の panic の中身は `discard_panic` で捨てる（その破棄の panic でも残りを呼び切る）。
///
/// 錠を放してから呼ぶこと。呼ぶ側が駆動の呼び出しの最後に `resume_unwind` で再送出する
/// （`Drop` では捨てる）。取りこぼしと二重呼び出しを作らないため、各 callback は呼ぶ前にすでに表から外れている。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn run_callbacks<C>(
    ready: Vec<(C, u64, CompletionState)>,
    read_generation: impl Fn() -> u64,
    call: impl Fn(C, CompletionState),
) -> Option<Box<dyn std::any::Any + Send>> {
    let mut first_panic = None;
    for (callback, generation, state) in ready {
        let state = if generation < read_generation() {
            CompletionState::Lost
        } else {
            state
        };
        let outcome =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| call(callback, state)));
        if let Err(payload) = outcome {
            if first_panic.is_none() {
                first_panic = Some(payload);
            } else {
                discard_panic(payload);
            }
        }
    }
    first_panic
}

/// 表の錠を取る。毒化していても中身を使う（表は壊れない更新だけを行う）。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn lock_table<G, C>(table: &Mutex<Submissions<G, C>>) -> MutexGuard<'_, Submissions<G, C>> {
    table.lock().unwrap_or_else(|e| e.into_inner())
}

/// 駆動の本体。回収して終端の登録を外すまでを錠の中で行い、錠を放してから `call` で 1 回ずつ呼ぶ。
///
/// 登録が空なら回収もしない。callback が panic しても残りを呼び切り、最後に最初の panic を
/// `resume_unwind` で伝える（利用者の callback の panic を伝えるもので、この関数自身は panic しない）。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn drive_table<G, C>(
    table: &Mutex<Submissions<G, C>>,
    current_generation: u64,
    read_generation: impl Fn() -> u64,
    classify: impl Fn(&G) -> GroupState + Copy,
    call: impl Fn(C, CompletionState),
) {
    let ready = {
        let mut subs = lock_table(table);
        if subs.callbacks.is_empty() {
            return;
        }
        subs.take_terminal(current_generation, classify)
    };
    if let Some(payload) = run_callbacks(ready, read_generation, call) {
        std::panic::resume_unwind(payload);
    }
}

/// ロストの後の掃き出し。世代が今と違う登録を外し、錠を放してから `Lost` で 1 回ずつ呼ぶ。
///
/// `drive_table` と同じく、callback が panic しても残りを呼び切り、最後に最初の panic を伝える。
/// 外した登録は表に残らないので、`drive_table` や `Drop` と二重に呼ばれない。
#[cfg(any(test, feature = "fault-injection"))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn sweep_stale_table<G, C>(
    table: &Mutex<Submissions<G, C>>,
    current_generation: u64,
    call: impl Fn(C, CompletionState),
) {
    let ready = lock_table(table).take_stale(current_generation);
    if let Some(payload) = run_callbacks(ready, || current_generation, call) {
        std::panic::resume_unwind(payload);
    }
}

/// 登録の本体。終端ならその場で（錠を放してから）`call` で 1 回呼び、未完了なら表に積む。回収はしない。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn register_table<G, C>(
    table: &Mutex<Submissions<G, C>>,
    sequence: u64,
    generation: u64,
    read_generation: impl Fn() -> u64,
    callback: C,
    classify: impl Fn(&G) -> GroupState,
    call: impl Fn(C, CompletionState),
) {
    // 世代の読みの例外: 呼び出した操作の読みではなく、表の錠の中で読み直す。ロストの掃き出しとすれ違うと、
    // 掃き出しの済んだ表へ古い世代の登録が残るため。ロストは世代を書いてから掃き出すので、錠の中の読みで抜けが無い。
    let ready = {
        let mut subs = lock_table(table);
        let current = read_generation();
        subs.register(sequence, generation, current, callback, classify)
    };
    if let Some((callback, state)) = ready {
        // 錠を放した後に世代が進んでいたら、決めた状態に関わらず `Lost` を渡す。
        let state = if generation < read_generation() {
            CompletionState::Lost
        } else {
            state
        };
        call(callback, state);
    }
}

/// `Drop` の本体。登録が残っていれば、その列を `wait` で待ってから、残りを `call` で 1 回ずつ呼ぶ。
///
/// `&mut` なので錠は取らない。callback の panic は捨てる（`Drop` から伝えない）。捨てる中身の破棄の panic も止める。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn finish_table<G, C>(
    subs: &mut Submissions<G, C>,
    current_generation: u64,
    wait: impl Fn(&G),
    classify: impl Fn(&G) -> GroupState + Copy,
    call: impl Fn(C, CompletionState),
) {
    if subs.callbacks.is_empty() {
        return;
    }
    subs.wait_registered(wait);
    let ready = subs.take_all(current_generation, classify);
    if let Some(payload) = run_callbacks(ready, || current_generation, call) {
        discard_panic(payload);
    }
}

/// 終端の状態に寄せる。待ったあとでも `Pending` なら（起きないはずだが）失敗側の `Lost` にする。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn terminal_state(state: CompletionState) -> CompletionState {
    match state {
        CompletionState::Pending => CompletionState::Lost,
        done => done,
    }
}

/// `wait_blocking` が待ったあとの `poll` の値を結果に写す。
///
/// 失敗した投入は `DeviceLost` ではなく `BackendFailure`（この背後実装は device のロストを返さない）。
#[cfg(any(test, feature = "native-blocking-wait"))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn wait_result(state: CompletionState) -> Result<(), ContractError> {
    match state {
        CompletionState::Complete => Ok(()),
        CompletionState::Lost => Err(ContractError::BackendFailure {
            op: "wait_blocking",
            backend: BackendKind::Metal,
            detail: "the submission ended with an error".into(),
        }),
        CompletionState::Pending => Err(ContractError::BackendFailure {
            op: "wait_blocking",
            backend: BackendKind::Metal,
            detail: "submission is still pending after the wait".into(),
        }),
    }
}

/// 空でない `wait` を、何も投入せずに `Unsupported` で拒む。device に触らない。
///
/// この段の Metal の背後実装は取り込み済みセマフォを待てない。`submit`・`submit_ex`・`submit_many` が共有する。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn reject_wait(op: &'static str, wait: &[ImportedSemaphoreHandle]) -> Result<(), ContractError> {
    if wait.is_empty() {
        Ok(())
    } else {
        Err(ContractError::Unsupported {
            op,
            backend: BackendKind::Metal,
        })
    }
}

/// 投入の印が、この device・この世代のものか照らす。device に触らない。
///
/// 別の device の印は `DeviceMismatch`、同じ device で世代が違う印は `DeviceLost`。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn check_token_owner(
    token: &SubmitToken,
    device_id: u64,
    generation: u64,
) -> Result<(), ContractError> {
    if token.__backend_device_id() != device_id {
        return Err(ContractError::DeviceMismatch);
    }
    if token.generation != generation {
        return Err(ContractError::DeviceLost {
            generation: token.generation,
        });
    }
    Ok(())
}

/// `submit` が device に触る前に通す検査。`wait` だけを見る。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn precheck_submit(wait: &[ImportedSemaphoreHandle]) -> Result<(), ContractError> {
    reject_wait("submit(wait)", wait)
}

/// `submit_many` が device に触る前に通す検査。順は、`after` の各印の照合、空の列、`wait`。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn precheck_submit_many(
    command_count: usize,
    after: &[SubmitToken],
    wait: &[ImportedSemaphoreHandle],
    device_id: u64,
    generation: u64,
) -> Result<(), ContractError> {
    for token in after {
        check_token_owner(token, device_id, generation)?;
    }
    if command_count == 0 {
        return Err(ContractError::InvalidArgument(
            "submit_many requires at least one encoder".into(),
        ));
    }
    reject_wait("submit_many(wait)", wait)
}

/// 再利用の一覧に溜めるバッファの件数の上限（全 `byte_len` の合計）。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const POOL_MAX_ENTRIES: usize = 16;

/// 再利用の一覧に溜めるバッファの大きさの合計（バイト）の上限。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const POOL_MAX_BYTES: u64 = 64 * 1024 * 1024;

/// 再利用の一覧に入るバッファ 1 本。`slot` は Metal のバッファごとに 1 度だけ払い出す識別子で、
/// 一覧を出入りしても変わらない（ハンドルの識別子とは別で、再利用のたびに新しいハンドルの識別子を払い出す）。
/// `G` は本物では Metal のバッファで、試験では偽の値を使う。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
struct PooledSlot<G> {
    slot: u64,
    buffer: G,
}

/// 同じ `byte_len` ごとの再利用の一覧。`device` の表の錠の中に置く。
///
/// 入れるバッファは呼び出し側が先に全バイトを 0 にしたものに限る（この部品は中身を見ない）。
/// 件数と大きさの合計に上限があり、超える分は入れずに呼び出し側へ返す（呼び出し側が解放する）。
/// 再利用の回数と、取り出した `slot` が直前に入れた `slot` と一致した回数を数える（試験が読む）。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
struct SizePool<G> {
    by_len: BTreeMap<u64, Vec<PooledSlot<G>>>,
    entries: usize,
    bytes: u64,
    last_put_slot: Option<u64>,
    reuse_count: u64,
    slot_match_count: u64,
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
impl<G> SizePool<G> {
    fn new() -> Self {
        Self {
            by_len: BTreeMap::new(),
            entries: 0,
            bytes: 0,
            last_put_slot: None,
            reuse_count: 0,
            slot_match_count: 0,
        }
    }

    /// 入れた後の件数と大きさの合計。上限を超える・計算があふれるときは `None`。
    fn next_totals(&self, byte_len: u64) -> Option<(usize, u64)> {
        let entries = self.entries.checked_add(1)?;
        let bytes = self.bytes.checked_add(byte_len)?;
        (entries <= POOL_MAX_ENTRIES && bytes <= POOL_MAX_BYTES).then_some((entries, bytes))
    }

    /// いま `byte_len` のバッファを入れられるか。呼び出し側がゼロ化の前に確かめる。
    /// 錠の外でゼロ化する間に状況が変わりうるので、`put` が改めて検査する。
    fn would_accept(&self, byte_len: u64) -> bool {
        self.next_totals(byte_len).is_some()
    }

    /// 入れる。上限に当たる（件数・大きさの合計・計算のあふれ）と、一覧を変えずに渡された値を返す。
    fn put(&mut self, byte_len: u64, slot: u64, buffer: G) -> Option<G> {
        let Some((next_entries, next_bytes)) = self.next_totals(byte_len) else {
            return Some(buffer);
        };
        self.entries = next_entries;
        self.bytes = next_bytes;
        self.last_put_slot = Some(slot);
        self.by_len
            .entry(byte_len)
            .or_default()
            .push(PooledSlot { slot, buffer });
        None
    }

    /// 同じ `byte_len` の一覧から 1 本取り出す。別の大きさは見ない。取り出せたら再利用の回数を数える。
    fn take(&mut self, byte_len: u64) -> Option<(u64, G)> {
        let list = self.by_len.get_mut(&byte_len)?;
        let PooledSlot { slot, buffer } = list.pop()?;
        if list.is_empty() {
            self.by_len.remove(&byte_len);
        }
        // 入れた分だけ引くので、あふれ・負にはならない。
        self.entries = self.entries.saturating_sub(1);
        self.bytes = self.bytes.saturating_sub(byte_len);
        self.reuse_count = self.reuse_count.saturating_add(1);
        if self.last_put_slot == Some(slot) {
            self.slot_match_count = self.slot_match_count.saturating_add(1);
        }
        Some((slot, buffer))
    }
}

/// 表から外したが、最後に使った投入が終わっていないバッファの一覧。`device` の表の錠の中に置く。
///
/// 項目は通し番号と一緒に積んだ順に持つ。ゼロ化も再利用の一覧への回しも、この部品は行わない
/// （終わった項目を外して返し、続きは呼び出し側が錠の外で行う）。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
struct PendingReleases<B> {
    items: Vec<(u64, B)>,
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
impl<B> PendingReleases<B> {
    fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// 通し番号つきで末尾へ積む。
    fn push(&mut self, sequence: u64, item: B) {
        self.items.push((sequence, item));
    }

    /// `is_finished` が真の項目を積んだ順に外して返す。偽の項目は相対の順を保って残す（先頭で止めない）。
    fn take_finished(&mut self, is_finished: impl Fn(u64) -> bool) -> Vec<B> {
        let mut finished = Vec::new();
        let mut kept = Vec::new();
        for (sequence, item) in self.items.drain(..) {
            if is_finished(sequence) {
                finished.push(item);
            } else {
                kept.push((sequence, item));
            }
        }
        self.items = kept;
        finished
    }

    /// 試験専用。積んである件数。
    #[cfg(test)]
    fn len(&self) -> usize {
        self.items.len()
    }
}

/// 解放されたバッファを、ゼロ化してから再利用の一覧へ回す流れ。受け入れられなかったバッファを返す
/// （呼び出し側が解放する）。device に触らず、錠の取り方は呼び出し側の閉包に任せる。
///
/// 順は、`accepts(byte_len)` で一覧に入れられるか確かめる（入れられないならゼロ化せずに返す）、
/// `zero` でゼロ化する（失敗したら入れずに返す）、`put` で入れる（検査に当たれば返す）。
/// 一覧に入るのはゼロ化に成功したバッファだけになる。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn retire<B, E>(
    byte_len: u64,
    mut buffer: B,
    accepts: impl FnOnce(u64) -> bool,
    zero: impl FnOnce(&mut B) -> Result<(), E>,
    put: impl FnOnce(B) -> Option<B>,
) -> Option<B> {
    if !accepts(byte_len) {
        return Some(buffer);
    }
    if zero(&mut buffer).is_err() {
        return Some(buffer);
    }
    put(buffer)
}

/// 束縛のハンドルを表の項目と照らし、通ればバッファの実際の長さを返す。
///
/// ハンドルの世代が項目の世代と違うとき、またはハンドルの `size()` が `buffer_len` と違うときは `InvalidArgument`。
/// 世代、大きさの順に見る。device に触らない。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn check_handle_record(
    handle_generation: u64,
    handle_size: u64,
    entry_generation: u64,
    buffer_len: u64,
) -> Result<u64, ContractError> {
    if handle_generation != entry_generation {
        return Err(ContractError::InvalidArgument(
            "buffer handle generation does not match the issued record".into(),
        ));
    }
    if handle_size != buffer_len {
        return Err(ContractError::InvalidArgument(
            "buffer handle size does not match the buffer length".into(),
        ));
    }
    Ok(buffer_len)
}

/// 引数の宣言を C8、C7、C4、C5、C6 の順に照らす。
///
/// `capacity(i)` は `bindings` の `i` 番目の束縛の容量（ハンドルの照合を含む）を返す。C4 の段で
/// 束縛の並びの順に引かれ、同じ位置は 2 回引かない。`capacity` が `Err` を返した束縛は、
/// その `Err` をそのまま返す。照らしが通ったときは、まだ引いていない位置もすべて引いてから `Ok(())` を返す。
/// C8・C7 に当たるときは `capacity` を 1 回も呼ばない。device に触らない。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn check_declared_operands(
    declared: &[OperandSpec<'_>],
    bindings: &[OperandBinding],
    consts: &[OperandConstant<'_>],
    capacity: impl Fn(usize) -> Result<u64, ContractError>,
    max_buffer_bytes: u64,
) -> Result<(), ContractError> {
    let lens: RefCell<Vec<Option<u64>>> = RefCell::new(vec![None; bindings.len()]);
    let first_err: RefCell<Option<ContractError>> = RefCell::new(None);
    // 引いた結果を位置ごとに保持する。`capacity` を呼んでいる間は借用を持たない。
    let fetch = |pos: usize| -> Option<u64> {
        let known = lens.borrow().get(pos).copied().flatten();
        if known.is_some() {
            return known;
        }
        match capacity(pos) {
            Ok(len) => {
                if let Some(entry) = lens.borrow_mut().get_mut(pos) {
                    *entry = Some(len);
                }
                Some(len)
            }
            Err(e) => {
                let mut first = first_err.borrow_mut();
                if first.is_none() {
                    *first = Some(e);
                }
                None
            }
        }
    };
    let checked = validate_dispatch_operands(
        declared,
        bindings,
        consts,
        |b| {
            let Some(pos) = bindings.iter().position(|x| x.slot == b.slot) else {
                return Some(0);
            };
            // 照合に外れた束縛は容量 0 として C4 を落とし、下で照合のエラーを優先する。
            Some(fetch(pos).unwrap_or(0))
        },
        max_buffer_bytes,
    );
    let lookup_err = first_err.borrow_mut().take();
    if let Some(e) = lookup_err {
        return Err(e);
    }
    checked?;
    for pos in 0..bindings.len() {
        if fetch(pos).is_none() {
            break;
        }
    }
    let lookup_err = first_err.borrow_mut().take();
    match lookup_err {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// reflection が返すバッファ引数の要素の型。
///
/// `Struct` は構造体の引数、`Other` は上の 5 つにも `Struct` にも当たらない型の `MTLDataType` の値をそのまま持つ。
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReflectedDataType {
    Float,
    Half,
    UInt,
    Int,
    Bool,
    Struct,
    Other(u64),
}

/// reflection が返すバッファ引数のバイト長。0 は `Unchecked`、0 でなければ `Bytes`。
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReflectedSize {
    Bytes(u64),
    Unchecked,
}

/// reflection から読んだバッファ引数 1 つ（slot・バイト長・要素の型）。
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReflectedBuffer {
    slot: u32,
    size: ReflectedSize,
    data_type: ReflectedDataType,
}

/// パイプラインの reflection から読んだ値。
///
/// `argument_count` はバッファ以外も含む引数の数、`buffers` はバッファ引数の並び（slot の昇順）。
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
struct PipelineReflection {
    argument_count: usize,
    buffers: Vec<ReflectedBuffer>,
}

/// `DType` に対応する reflection の要素の型を返す。
///
/// `F32`→`Float`・`F16`→`Half`・`U32`→`UInt`・`I32`→`Int`・`Bool`→`Bool`。表に無い `DType` は `None`。
#[cfg_attr(not(test), allow(dead_code))]
fn expected_data_type(dtype: DType) -> Option<ReflectedDataType> {
    match dtype {
        DType::F32 => Some(ReflectedDataType::Float),
        DType::F16 => Some(ReflectedDataType::Half),
        DType::U32 => Some(ReflectedDataType::UInt),
        DType::I32 => Some(ReflectedDataType::Int),
        DType::Bool => Some(ReflectedDataType::Bool),
        _ => None,
    }
}

/// reflection の引数 1 つの index・バイト長・型から `ReflectedBuffer` を作る。
///
/// index が `u32`、バイト長が `u64` に収まらないときは `BackendFailure` を返す。
/// バイト長 0 は `ReflectedSize::Unchecked`、型は変えずに持つ。
#[cfg_attr(not(test), allow(dead_code))]
fn classify_reflected_buffer(
    index: usize,
    data_size: usize,
    data_type: ReflectedDataType,
) -> Result<ReflectedBuffer, ContractError> {
    let slot = u32::try_from(index).map_err(|_| ContractError::BackendFailure {
        op: "create_pipeline",
        backend: BackendKind::Metal,
        detail: "reflected argument index does not fit u32".into(),
    })?;
    let bytes = u64::try_from(data_size).map_err(|_| ContractError::BackendFailure {
        op: "create_pipeline",
        backend: BackendKind::Metal,
        detail: "reflected buffer size does not fit u64".into(),
    })?;
    let size = if bytes == 0 {
        ReflectedSize::Unchecked
    } else {
        ReflectedSize::Bytes(bytes)
    };
    Ok(ReflectedBuffer {
        slot,
        size,
        data_type,
    })
}

/// 書き出したファイルを読むときの上限のバイト数。
#[cfg(feature = "artifact-inspection")]
#[cfg_attr(not(test), allow(dead_code))]
const ARTIFACT_READ_LIMIT: u64 = 1 << 28;

/// 絶対・UTF-8・空でないパスを `&str` にして返す。外れれば `InvalidArgument`。
#[cfg(feature = "artifact-inspection")]
#[cfg_attr(not(test), allow(dead_code))]
fn artifact_path_str(path: &std::path::Path) -> Result<&str, ContractError> {
    if path.as_os_str().is_empty() {
        return Err(ContractError::InvalidArgument(
            "artifact path is empty".into(),
        ));
    }
    if !path.is_absolute() {
        return Err(ContractError::InvalidArgument(
            "artifact path must be absolute".into(),
        ));
    }
    path.to_str()
        .ok_or_else(|| ContractError::InvalidArgument("artifact path is not valid UTF-8".into()))
}

/// 通常のファイル（シンボリックリンクでない）で、長さが 1 以上 `limit` 以下のときだけ中身を返す。
/// 外れれば `ArtifactRejected { kind: "binary-archive" }`。
#[cfg(feature = "artifact-inspection")]
#[cfg_attr(not(test), allow(dead_code))]
fn read_written_artifact(path: &std::path::Path, limit: u64) -> Result<Vec<u8>, ContractError> {
    use std::io::Read as _;

    let rejected = |reason: String| ContractError::ArtifactRejected {
        kind: "binary-archive",
        reason,
    };
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|e| rejected(format!("cannot inspect written artifact: {e}")))?;
    if !metadata.file_type().is_file() {
        return Err(rejected("written artifact is not a regular file".into()));
    }
    if metadata.len() == 0 {
        return Err(rejected("written artifact is empty".into()));
    }
    if metadata.len() > limit {
        return Err(rejected(format!("written artifact exceeds {limit} bytes")));
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        // FIFO への差し替えで open が停止しないよう O_NONBLOCK で、シンボリックリンクを
        // open と不可分に拒否するよう O_NOFOLLOW で開く。値は libc の定数（外部依存を足さないため直書き）。
        #[cfg(all(target_os = "linux", any(target_arch = "arm", target_arch = "aarch64")))]
        options.custom_flags(0o4000 | 0o100_000);
        #[cfg(all(
            target_os = "linux",
            not(any(target_arch = "arm", target_arch = "aarch64"))
        ))]
        options.custom_flags(0o4000 | 0o400_000);
        #[cfg(target_os = "macos")]
        options.custom_flags(0x4 | 0x100);
    }
    let file = options
        .open(path)
        .map_err(|e| rejected(format!("cannot open written artifact: {e}")))?;
    let opened = file
        .metadata()
        .map_err(|e| rejected(format!("cannot inspect opened artifact: {e}")))?;
    if !opened.file_type().is_file() {
        return Err(rejected("written artifact is not a regular file".into()));
    }
    // 検査の後に open までの間でシンボリックリンク等へ差し替えられていないことを、
    // 開いたハンドルと開いた直後のパス（リンクを辿らない）の同一性で確かめる。
    let after = std::fs::symlink_metadata(path)
        .map_err(|e| rejected(format!("cannot re-inspect written artifact: {e}")))?;
    if !after.file_type().is_file() {
        return Err(rejected("written artifact is not a regular file".into()));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        if opened.dev() != after.dev() || opened.ino() != after.ino() {
            return Err(rejected("written artifact changed while opening".into()));
        }
    }
    if opened.len() == 0 || opened.len() > limit {
        return Err(rejected(
            "written artifact size changed while opening".into(),
        ));
    }
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| rejected(format!("cannot read written artifact: {e}")))?;
    match u64::try_from(bytes.len()) {
        Ok(0) => Err(rejected("written artifact is empty".into())),
        Ok(n) if n <= limit => Ok(bytes),
        _ => Err(rejected(format!("written artifact exceeds {limit} bytes"))),
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use std::collections::HashMap;
    use std::ffi::c_void;
    use std::ptr::NonNull;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex, MutexGuard};

    #[cfg(feature = "native-blocking-wait")]
    use super::span_of;
    #[cfg(feature = "fault-injection")]
    use super::sweep_stale_table;
    #[cfg(feature = "native-blocking-wait")]
    use super::wait_result;
    use super::{BindingArg, ConstantArg, DispatchArgs, ThreadgroupArg};
    use super::{
        BufState, Callback, CompilePlan, ConstantKind, DispatchRecord, GroupState, OpenEncoders,
        PendingReleases, PipelineReflection, RecordedBinding, ReflectedDataType, SizePool,
        Submissions, check_declared_operands, check_dispatch_args, check_handle_record,
        check_recorded_buffers, check_token_owner, check_unique_slots, classify_pipeline_input,
        classify_reflected_buffer, compile_plan, drive_table, driven_generation, encode_plan,
        finish_table, group_state_of, missing_required_constants, missing_required_reason,
        precheck_submit, precheck_submit_many, prepare_constants, register_table, remove_issued,
        retire, take_next,
    };
    use super::{
        DISPATCH_EXECUTION_ENABLED, MAX_FUNCTION_CONSTANT_INDEX, cache_key_version, host_range,
        metal_cache_key,
    };

    #[cfg(feature = "artifact-inspection")]
    use super::{ARTIFACT_READ_LIMIT, artifact_path_str, read_written_artifact};
    #[cfg(feature = "artifact-inspection")]
    use fandhe_silicon_core::artifact::ArtifactHooks;
    use fandhe_silicon_core::capability::{
        BackendKind, Capabilities, CapabilityLevel, CapabilityQuery, FeatureId, LimitsTable,
    };
    use fandhe_silicon_core::device::{
        __backend_next_device_id, Device, DeviceDiagnostics, DeviceOwnership, DeviceProvider,
        DeviceRequest, Profile,
    };
    #[cfg(feature = "fault-injection")]
    use fandhe_silicon_core::device_lost::{DeviceLostNotify, LostCallback};
    #[cfg(feature = "fault-injection")]
    use fandhe_silicon_core::error::DeviceLostReason;
    use fandhe_silicon_core::error::{
        CompletionState, CompletionStateEx, ContractError, UnsupportedReason,
    };
    use fandhe_silicon_core::execution::{
        __BackendSharedEncoderCloser, __backend_next_encoder_id, BindingLayout, CacheKey,
        CommandEncoder, CommandQueue, ComputeOps, GpuTiming, ImportedSemaphoreHandle,
        OperandBinding, OperandConstant, OperandSpec, PipelineHandle, PipelineInput, SubmitToken,
    };
    #[cfg(feature = "artifact-inspection")]
    use fandhe_silicon_core::execution::{MslMathMode, validate_declared_operands};
    #[cfg(feature = "fault-injection")]
    use fandhe_silicon_core::fault::FaultHooks;
    use fandhe_silicon_core::memory::{
        BufferHandle, BufferUsage, MemoryOps, validate_allocation_size,
    };
    use objc2::rc::Retained;
    use objc2::runtime::{NSObjectProtocol, ProtocolObject};
    use objc2::sel;
    // コマンドラインのプログラムで `MTLCreateSystemDefaultDevice` が既定の device を返すには、
    // Core Graphics がリンクされている必要がある。参照を置いてリンクを確実にする（シンボルは使わない）。
    use objc2_core_graphics as _;
    #[cfg(feature = "artifact-inspection")]
    use objc2_foundation::{NSArray, NSURL};
    use objc2_foundation::{NSProcessInfo, NSString};
    #[cfg(feature = "artifact-inspection")]
    use objc2_metal::{MTLBinaryArchive, MTLBinaryArchiveDescriptor};
    use objc2_metal::{
        MTLBuffer, MTLCommandBuffer, MTLCommandBufferStatus, MTLCommandEncoder, MTLCommandQueue,
        MTLCompileOptions, MTLComputeCommandEncoder, MTLComputePipelineDescriptor,
        MTLComputePipelineReflection, MTLComputePipelineState, MTLCreateSystemDefaultDevice,
        MTLDataType, MTLDevice, MTLFunction, MTLFunctionConstantValues, MTLLibrary,
        MTLMathFloatingPointFunctions, MTLMathMode, MTLPipelineOption, MTLResourceOptions, MTLSize,
    };

    /// 計画をライブラリのコンパイルのオプションにする。`create_pipeline` から呼ばれる。
    ///
    /// 既定の計画は `None`（既定のオプション）を返す。そうでなければ新しいオプションに、計画が求める
    /// 数学モード・関数の精度だけを設定する（言語版などほかの設定は呼ばない）。設定の関数を持たない
    /// 環境では、Metal の設定の API を呼ぶ前に `Unsupported` を返し、既定のオプションで黙って作らない。
    fn compile_options(
        plan: CompilePlan,
    ) -> Result<Option<Retained<MTLCompileOptions>>, ContractError> {
        if plan == CompilePlan::default() {
            return Ok(None);
        }
        let unsupported = || ContractError::Unsupported {
            op: "create_pipeline",
            backend: BACKEND,
        };
        let options = MTLCompileOptions::new();
        if plan.safe_math {
            if !options.respondsToSelector(sel!(setMathMode:)) {
                return Err(unsupported());
            }
            options.setMathMode(MTLMathMode::Safe);
        }
        if plan.precise_functions {
            if !options.respondsToSelector(sel!(setMathFloatingPointFunctions:)) {
                return Err(unsupported());
            }
            options.setMathFloatingPointFunctions(MTLMathFloatingPointFunctions::Precise);
        }
        Ok(Some(options))
    }

    /// この背後実装の種類。エラーの `backend` と `Device::backend` の値。
    const BACKEND: BackendKind = BackendKind::Metal;

    /// 診断の `backend_name`。
    const BACKEND_NAME: &str = "metal";

    /// Metal が扱うテクスチャの 1 辺の上限として `LimitsTable` に載せる値。
    const MAX_TEXTURE_SIZE: u32 = 16384;

    /// この背後実装のプロセスの中で、device を作るごとに 1 つ増える世代の源。1 から始まる。
    static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

    /// `request` の中の呼び出しの結果を `None` に差し替える指示。
    ///
    /// 本物の `request` は `Faults::default()`（何も差し替えない）で同じ関数を通る。
    /// feature `fault-injection` の提供型だけが、どちらかを `true` にして呼ぶ。
    #[derive(Clone, Copy, Default)]
    struct Faults {
        /// `MTLCreateSystemDefaultDevice` を呼ばずに、結果を `None` として扱う。
        default_device_none: bool,
        /// `newCommandQueue` の結果を `None` として扱う（device は本物を取る）。
        command_queue_none: bool,
    }

    /// 要求の組み合わせを照らし、device を作って返す。本物の `request` と差し込みの型が共有する。
    ///
    /// 照らす順は、背後実装、プロファイル（`Graphics` の feature、`CpuOnly`、未知の値）、所有形態。
    /// 最初に当たったものを返し、そのあいだは Metal の呼び出しをしない。
    fn request_with(req: DeviceRequest, faults: Faults) -> Result<Box<dyn Device>, ContractError> {
        if req.backend != BACKEND {
            return Err(ContractError::InvalidCombination(format!(
                "Metal provider cannot serve backend {:?}",
                req.backend
            )));
        }
        match req.profile {
            Profile::ComputeOnly => {}
            Profile::Graphics => {
                #[cfg(not(feature = "graphics"))]
                return Err(ContractError::InvalidCombination(
                    "profile Graphics requires the `graphics` feature of the contract crate".into(),
                ));
            }
            Profile::CpuOnly => {
                return Err(ContractError::InvalidCombination(format!(
                    "backend {BACKEND:?} does not serve profile {:?}",
                    req.profile
                )));
            }
            _ => {
                return Err(ContractError::InvalidCombination(
                    "unknown device profile".into(),
                ));
            }
        }
        match req.ownership {
            DeviceOwnership::ProductIndependent => {}
            DeviceOwnership::Shared => {
                return Err(ContractError::Unsupported {
                    op: "request(DeviceOwnership::Shared)",
                    backend: BACKEND,
                });
            }
            DeviceOwnership::Imported { .. } => {
                return Err(ContractError::Unsupported {
                    op: "request(DeviceOwnership::Imported)",
                    backend: BACKEND,
                });
            }
            _ => {
                return Err(ContractError::InvalidCombination(
                    "unknown device ownership".into(),
                ));
            }
        }

        let injected = if faults.default_device_none {
            " (injected)"
        } else {
            ""
        };
        let device = if faults.default_device_none {
            None
        } else {
            MTLCreateSystemDefaultDevice()
        }
        .ok_or_else(|| {
            ContractError::DeviceUnavailable(format!(
                "MTLCreateSystemDefaultDevice returned None{injected}"
            ))
        })?;

        let queue_injected = if faults.command_queue_none {
            " (injected)"
        } else {
            ""
        };
        let queue = if faults.command_queue_none {
            None
        } else {
            device.newCommandQueue()
        }
        .ok_or_else(|| {
            ContractError::DeviceUnavailable(format!(
                "newCommandQueue returned None{queue_injected}"
            ))
        })?;

        let exhausted =
            || ContractError::DeviceUnavailable("device counter space is exhausted".into());
        let id = __backend_next_device_id().ok_or_else(exhausted)?;
        let generation = take_next(&NEXT_GENERATION).ok_or_else(exhausted)?;

        let limits = LimitsTable {
            max_buffer_bytes: u64::try_from(device.maxBufferLength()).unwrap_or(u64::MAX),
            max_dispatch_dims: [u32::MAX; 3],
            max_texture_size: MAX_TEXTURE_SIZE,
        };
        let adapter_name = device.name().to_string();

        Ok(Box::new(MetalDevice::new(
            id,
            generation,
            adapter_name,
            limits,
            device,
            queue,
        )))
    }

    /// Metal の背後実装で device を作る入口。
    ///
    /// `request` は `Metal` の背後実装・`ComputeOnly`（`graphics` feature が有効なら `Graphics` も）・
    /// `ProductIndependent` の組だけを受ける。`ProductIndependent` では、呼ぶたびに新しい device と
    /// コマンドキューを作り、共有しない。
    #[derive(Debug, Clone, Copy, Default)]
    pub struct MetalDeviceProvider;

    impl DeviceProvider for MetalDeviceProvider {
        /// 要求を照らしてから Metal の既定の device を取り、device を作る。
        ///
        /// - 背後実装が Metal でない、`CpuOnly`、`graphics` が無効での `Graphics`:
        ///   `InvalidCombination`（device を作る前）。
        /// - `Shared`・`Imported`: `Unsupported`（device を作る前）。
        /// - 既定の device またはコマンドキューが得られない: `DeviceUnavailable`。
        ///
        /// 別の背後実装へは切り替えない。
        fn request(req: DeviceRequest) -> Result<Box<dyn Device>, ContractError> {
            request_with(req, Faults::default())
        }
    }

    /// 既定の device の取得が `None` を返した場合を作る差し込み用の入口。実際の故障ではない。
    ///
    /// 要求の検査は `MetalDeviceProvider` と同じで、検査を通ったら `MTLCreateSystemDefaultDevice` を
    /// 呼ばずに、その結果を `None` として扱い、`DeviceUnavailable` を返す（device に触らない）。
    /// 初期化失敗の経路を、実機の故障なしで通すための試験・診断用で、feature `fault-injection` の裏にある。
    #[cfg(feature = "fault-injection")]
    #[derive(Debug, Clone, Copy, Default)]
    pub struct MetalDefaultDeviceNoneProvider;

    #[cfg(feature = "fault-injection")]
    impl DeviceProvider for MetalDefaultDeviceNoneProvider {
        /// 検査を通ったら、既定の device の取得の結果を `None` に差し替えて `DeviceUnavailable` を返す。
        fn request(req: DeviceRequest) -> Result<Box<dyn Device>, ContractError> {
            request_with(
                req,
                Faults {
                    default_device_none: true,
                    ..Faults::default()
                },
            )
        }
    }

    /// コマンドキューの作成が `None` を返した場合を作る差し込み用の入口。実際の故障ではない。
    ///
    /// 要求の検査は `MetalDeviceProvider` と同じで、検査を通ったら本物の既定の device を取り、
    /// `newCommandQueue` の結果を `None` として扱い、`DeviceUnavailable` を返す（device に触る）。
    /// 初期化失敗の経路を通すための試験・診断用で、feature `fault-injection` の裏にある。
    #[cfg(feature = "fault-injection")]
    #[derive(Debug, Clone, Copy, Default)]
    pub struct MetalCommandQueueNoneProvider;

    #[cfg(feature = "fault-injection")]
    impl DeviceProvider for MetalCommandQueueNoneProvider {
        /// 検査を通ったら、コマンドキューの作成の結果を `None` に差し替えて `DeviceUnavailable` を返す。
        fn request(req: DeviceRequest) -> Result<Box<dyn Device>, ContractError> {
            request_with(
                req,
                Faults {
                    command_queue_none: true,
                    ..Faults::default()
                },
            )
        }
    }

    /// device が持つバッファの 1 本。
    struct BufferEntry {
        /// Metal のバッファ。`free` で表から外すまで device が保持する。
        buffer: Retained<ProtocolObject<dyn MTLBuffer>>,
        /// このバッファの識別子（slot）。Metal のバッファを作るときに 1 度だけ払い出し、再利用の一覧を出入りしても変わらない。
        slot: u64,
        /// 払い出したときの世代。`free` はロストの後でも、これと一致するハンドルなら外す。
        generation: u64,
        /// このバッファを最後に使った投入の通し番号。使った投入が無ければ `None`。
        last_submission: Option<u64>,
    }

    /// device のバッファの表。識別子は device ごとに 1 から払い出す。
    struct Buffers {
        /// 次に払い出すバッファの識別子。
        next_id: u64,
        /// 識別子からバッファへの表。
        map: HashMap<u64, BufferEntry>,
        /// 次に払い出す slot。
        next_slot: u64,
        /// 解放されたバッファの再利用の一覧。入っているのは全バイトが 0 のバッファだけ。
        pool: SizePool<Retained<ProtocolObject<dyn MTLBuffer>>>,
        /// 表から外したが、最後に使った投入が終わっていないバッファの一覧。ゼロ化も再利用の一覧への回しもまだしていない。
        /// 終わったものは回収の点で外してゼロ化し、再利用の一覧へ回す。device の Drop ではゼロ化せずに落とす。
        pending: PendingReleases<BufferEntry>,
    }

    /// device のパイプラインの表。識別子は device ごとに 1 から払い出す。
    struct Pipelines {
        /// 次に払い出すパイプラインの識別子の源。`take_next` で払い出す。
        next_id: AtomicU64,
        /// 識別子から（パイプライン状態, 作成時に渡された `BindingLayout` の写し）への表。
        /// パイプラインの破棄の口は無く、作った分だけ件数が増え続ける（上限も無い）。
        map: HashMap<
            u64,
            (
                Retained<ProtocolObject<dyn MTLComputePipelineState>>,
                BindingLayout,
            ),
        >,
    }

    /// ゼロ化に使う 0 の並び。`free` の中で大きな領域を確保しないよう、固定長を繰り返し書く。
    const ZEROS: [u8; 4096] = [0; 4096];

    /// Metal の device 1 つ。`DeviceProvider::request` が作る。
    ///
    /// device・コマンドキュー・バッファは、この値が持つ（Drop で解放される）。
    /// バッファは shared の storage で確保し、読み書きは CPU からの `contents()` 越しのコピーで行う。
    struct MetalDevice {
        /// この背後実装の中で device ごとに別の識別子。ハンドルの払い出し元の照合に使う。
        id: u64,
        /// 世代。device を作るときに取り、模擬のロストの 1 回目で取り直す。読むのは `current_generation`。
        generation: AtomicU64,
        /// 模擬のロストの状態（理由とコールバック）。
        #[cfg(feature = "fault-injection")]
        lost: super::LostState,
        /// 作るときに指定された所有形態。
        ownership: DeviceOwnership,
        /// `MTLDevice::name` の文字列。
        adapter_name: String,
        /// 作るときに計算した上限の表。
        limits: LimitsTable,
        /// 既定の device。
        device: Retained<ProtocolObject<dyn MTLDevice>>,
        /// コマンドキュー。`submit`・`submit_many` がコマンドバッファをここから 1 つ作る。
        queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
        /// バッファの表。
        buffers: Mutex<Buffers>,
        /// パイプラインの表。作成した状態は device が持ち、破棄の口は持たない。
        pipelines: Mutex<Pipelines>,
        /// 開いている（まだ投入も破棄もされていない）記録先の識別子と、積んだ dispatch の記録の表。hook と共有する。
        open: Arc<OpenEncoders>,
        /// 投入済みの表。Objective-C の型を持つので、この錠の中だけに置く。
        submissions: Mutex<Submissions<Vec<CommandBuffer>>>,
        /// 試験専用。`Some` の間は `commit_records` が確定を遅らせ、ここに溜める（`None` なら従来どおり即確定）。
        /// 確定していないコマンドバッファは完了しないので、登録の時点で未完了の状態を決定的に作れる。
        /// 製品のビルドには存在しない。
        #[cfg(test)]
        commit_hold: Mutex<Option<Vec<CommandBuffer>>>,
    }

    /// 投入した 1 つのコマンドバッファ。
    type CommandBuffer = Retained<ProtocolObject<dyn MTLCommandBuffer>>;

    /// 列の各コマンドバッファの `status()` を写して、列の状態を求める。
    fn group_state(group: &Vec<CommandBuffer>) -> GroupState {
        group_state_of(group.iter().map(|buffer| {
            let status = buffer.status();
            if status == MTLCommandBufferStatus::Error {
                BufState::Failed
            } else if status == MTLCommandBufferStatus::Completed {
                BufState::Done
            } else {
                BufState::Running
            }
        }))
    }

    /// Metal のバッファ（参照を保持する）。
    type MetalBuffer = Retained<ProtocolObject<dyn MTLBuffer>>;

    /// 投入の時に compute encoder へ積む 1 件。記録を、Metal の呼び出しにそのまま渡せる値へ変換したもの。
    ///
    /// 変換で失敗しうる処理（`usize` への変換・表の引き直し）は `prepare_groups` が済ませる。
    /// 積む側（`encode_run`）は、この値を順に渡すだけで失敗しない。
    struct PreparedDispatch {
        pipeline_id: u64,
        state: Retained<ProtocolObject<dyn MTLComputePipelineState>>,
        /// （バッファ, offset, index）。バッファは `Retained` で持つ。
        bindings: Vec<(MetalBuffer, usize, usize)>,
        /// （bytes の写し, index）。
        constants: Vec<(Vec<u8>, usize)>,
        /// （長さ, index）。
        threadgroup_memory: Vec<(usize, usize)>,
        /// `threadgroup_memory` の index（積み方の計画が見る）。
        threadgroup_indices: Vec<u32>,
        grid: MTLSize,
        threads: MTLSize,
    }

    /// `NSUInteger` に収まらない値は、切り詰めずに `InvalidArgument` にする。
    fn to_usize(value: impl TryInto<usize>) -> Result<usize, ContractError> {
        value
            .try_into()
            .map_err(|_| ContractError::InvalidArgument("value does not fit usize".into()))
    }

    /// 記録先ごとの記録を `PreparedDispatch` にする。バッファ・パイプラインは表から `Retained` を写す。
    ///
    /// 表に無いもの・`usize` に収まらない値は `InvalidArgument`（積み始める前に返る）。
    fn prepare_groups(
        buffers: &Buffers,
        pipelines: &Pipelines,
        records: Vec<Vec<DispatchRecord>>,
    ) -> Result<Vec<Vec<PreparedDispatch>>, ContractError> {
        let size = |dims: [u32; 3]| -> Result<MTLSize, ContractError> {
            Ok(MTLSize {
                width: to_usize(dims[0])?,
                height: to_usize(dims[1])?,
                depth: to_usize(dims[2])?,
            })
        };
        let mut groups = Vec::with_capacity(records.len());
        for group in records {
            let mut prepared = Vec::with_capacity(group.len());
            for record in group {
                let Some((state, _)) = pipelines.map.get(&record.pipeline) else {
                    return Err(ContractError::InvalidArgument(
                        "pipeline handle is not in this device's table".into(),
                    ));
                };
                let mut bindings = Vec::with_capacity(record.bindings.len());
                for binding in &record.bindings {
                    let Some(entry) = buffers.map.get(&binding.buffer_id) else {
                        return Err(ContractError::InvalidArgument(format!(
                            "a bound buffer was freed before submit (binding slot {})",
                            binding.slot
                        )));
                    };
                    bindings.push((
                        entry.buffer.clone(),
                        to_usize(binding.offset)?,
                        to_usize(binding.slot)?,
                    ));
                }
                let mut constants = Vec::with_capacity(record.constants.len());
                for (slot, bytes) in record.constants {
                    constants.push((bytes, to_usize(slot)?));
                }
                let mut threadgroup_memory = Vec::with_capacity(record.threadgroup_memory.len());
                for (index, bytes) in &record.threadgroup_memory {
                    threadgroup_memory.push((to_usize(*bytes)?, to_usize(*index)?));
                }
                prepared.push(PreparedDispatch {
                    pipeline_id: record.pipeline,
                    state: state.clone(),
                    bindings,
                    constants,
                    threadgroup_memory,
                    threadgroup_indices: record
                        .threadgroup_memory
                        .iter()
                        .map(|(index, _)| *index)
                        .collect(),
                    grid: size(record.dispatch_dims)?,
                    threads: size(record.threads_per_threadgroup)?,
                });
            }
            groups.push(prepared);
        }
        Ok(groups)
    }

    /// 開いた compute encoder 1 つに、run の記録を順に積んで閉じる。失敗しうる処理を持たない。
    ///
    /// `run` の各要素の真偽は、`setComputePipelineState` を呼ぶか（積み方の計画が決める）。
    fn encode_run(
        encoder: &ProtocolObject<dyn MTLComputeCommandEncoder>,
        run: Vec<(PreparedDispatch, bool)>,
    ) {
        for (dispatch, set_pipeline) in run {
            if set_pipeline {
                encoder.setComputePipelineState(&dispatch.state);
            }
            set_bindings(encoder, &dispatch.bindings);
            set_constants(encoder, &dispatch.constants);
            set_threadgroup_memory(encoder, &dispatch.threadgroup_memory);
            encoder.dispatchThreadgroups_threadsPerThreadgroup(dispatch.grid, dispatch.threads);
        }
        encoder.endEncoding();
    }

    /// 束縛のバッファを、それぞれの index に offset つきで設定する。
    fn set_bindings(
        encoder: &ProtocolObject<dyn MTLComputeCommandEncoder>,
        bindings: &[(MetalBuffer, usize, usize)],
    ) {
        for (buffer, offset, index) in bindings {
            // SAFETY: `setBuffer_offset_atIndex` に渡す `buffer` は `Retained` で持った生きたバッファで、
            // 参照を保持する command buffer に積むので GPU が使う間も生きている。`offset` は記録の時点で
            // 確保の長さ以下かつ 4 の倍数（`BUFFER_OFFSET_ALIGN`）であることを確かめた。`index` は記録の時点で 0〜30 であることを確かめた。
            // このバッファを使う投入が終わるまで CPU は読み書きしない（読み書きと解放は、最後に使った投入が
            // 完了していることを表の錠と投入の錠の中で確かめてから行う）。
            unsafe { encoder.setBuffer_offset_atIndex(Some(buffer), *offset, *index) };
        }
    }

    /// 定数の bytes を、それぞれの index に値渡しで設定する。
    fn set_constants(
        encoder: &ProtocolObject<dyn MTLComputeCommandEncoder>,
        constants: &[(Vec<u8>, usize)],
    ) {
        for (bytes, index) in constants {
            let pointer = NonNull::from(bytes.as_slice()).cast::<c_void>();
            // SAFETY: `setBytes_length_atIndex` に渡す `pointer` は、長さ 1〜4096 の `Vec<u8>` の先頭で、
            // 呼び出しの間は `constants` が借用しているので生きている。長さは `bytes.len()` と同じで、
            // 記録の時点で 1〜4096 であることを確かめた。`index` は記録の時点で 0〜30 であることを確かめた。
            // 値は呼び出しの中でコピーされるので、戻った後にポインタは使われない。
            unsafe { encoder.setBytes_length_atIndex(pointer, bytes.len(), *index) };
        }
    }

    /// 共有メモリの長さを、それぞれの index に設定する。
    fn set_threadgroup_memory(
        encoder: &ProtocolObject<dyn MTLComputeCommandEncoder>,
        threadgroup_memory: &[(usize, usize)],
    ) {
        for (length, index) in threadgroup_memory {
            // SAFETY: `setThreadgroupMemoryLength_atIndex` に渡す共有メモリの長さは、記録の時点で 16 の倍数で、
            // パイプラインの固定の分との合計が device の上限以下であることを確かめた。
            // `index` は記録の時点で 0〜30 であることを確かめた。ポインタは渡さない。
            unsafe { encoder.setThreadgroupMemoryLength_atIndex(*length, *index) };
        }
    }

    impl MetalDevice {
        /// いまの世代。投入の印・バッファのハンドルの払い出しと照合は、この値を読む。
        fn current_generation(&self) -> u64 {
            self.generation.load(Ordering::Acquire)
        }

        /// 表の錠を取る。他のスレッドが panic して錠が汚れていても、表は壊れない更新だけなので中身を使う。
        fn lock_buffers(&self) -> MutexGuard<'_, Buffers> {
            self.buffers.lock().unwrap_or_else(|e| e.into_inner())
        }

        /// パイプラインの表の錠を取る。表は壊れない更新だけなので、汚れていても中身を使う。
        fn lock_pipelines(&self) -> MutexGuard<'_, Pipelines> {
            self.pipelines.lock().unwrap_or_else(|e| e.into_inner())
        }

        /// 投入済みの表の錠を取る。表は壊れない更新だけなので、汚れていても中身を使う。
        fn lock_submissions(&self) -> MutexGuard<'_, Submissions<Vec<CommandBuffer>>> {
            self.submissions.lock().unwrap_or_else(|e| e.into_inner())
        }

        /// 記録を束縛のバッファの確かめ・積み・確定・登録の順で投入し、通し番号の印を返す。
        ///
        /// `submit`・`submit_many` が共有する。`records` は記録先ごとの記録の列（渡した順）。
        /// バッファの表の錠を取ったまま、全束縛のバッファが有効なことを確かめ（外れれば `InvalidArgument` で
        /// command buffer も通し番号も作らない）、command buffer を 1 つ作り、記録のある記録先ごとに
        /// compute encoder を 1 つ開いて記録の順に積んで閉じ、確定し、束縛した各バッファの最後に使った投入を
        /// 払い出した通し番号にする。記録がまったく無いときは encoder を開かず、空の command buffer を確定する。
        /// 錠の順は バッファの表 -> パイプラインの表 -> 投入済みの表 -> `commit_hold`。
        /// 錠の中の `commandBuffer()` は、キューの未完了の上限に当たると確定済みの仕事の進みを待つだけで戻る。
        /// 製品の経路では、確定していないコマンドバッファを錠の中で溜めない（溜めると戻らなくなる）。
        /// 試験ビルドだけ、`commit_or_hold` で確定を遅らせて溜めることがある（数個まで。上限には届かない）。
        fn commit_records(
            &self,
            generation: u64,
            records: Vec<Vec<DispatchRecord>>,
        ) -> Result<SubmitToken, ContractError> {
            let mut buffers = self.lock_buffers();
            check_recorded_buffers(
                &records,
                |id| buffers.map.get(&id).map(|entry| entry.generation),
                self.current_generation(),
            )?;
            // 任意の MSL がカーネルのアクセス範囲を保証されないまま走るのを防ぐ。dispatch を含む投入は、
            // 範囲の保証が入るまで製品のビルドでは何も積まず確定せずに拒む（fail-closed）。
            if !DISPATCH_EXECUTION_ENABLED && records.iter().any(|group| !group.is_empty()) {
                return Err(ContractError::Unsupported {
                    op: "submit",
                    backend: BackendKind::Metal,
                });
            }
            let bound_ids: Vec<u64> = records
                .iter()
                .flatten()
                .flat_map(|record| record.bindings.iter().map(|binding| binding.buffer_id))
                .collect();
            // encoder を開く前に、失敗しうる変換と表の引き直しを済ませる。
            let groups = {
                let pipelines = self.lock_pipelines();
                prepare_groups(&buffers, &pipelines, records)?
            };
            let runs = encode_plan(
                groups,
                |dispatch| dispatch.pipeline_id,
                |dispatch| dispatch.threadgroup_indices.as_slice(),
            );

            let mut subs = self.lock_submissions();
            subs.reclaim(group_state);
            let sequence = subs.next_sequence;
            let next = sequence
                .checked_add(1)
                .ok_or_else(|| ContractError::BackendFailure {
                    op: "submit",
                    backend: BACKEND,
                    detail: "submission sequence space is exhausted".into(),
                })?;
            let buffer =
                self.queue
                    .commandBuffer()
                    .ok_or_else(|| ContractError::BackendFailure {
                        op: "submit",
                        backend: BACKEND,
                        detail: "commandBuffer returned None".into(),
                    })?;
            for run in runs {
                let Some(encoder) = buffer.computeCommandEncoder() else {
                    // 確定せずに落とす。通し番号も登録も進めない。
                    return Err(ContractError::BackendFailure {
                        op: "submit",
                        backend: BACKEND,
                        detail: "computeCommandEncoder returned None".into(),
                    });
                };
                encode_run(&encoder, run);
            }
            #[cfg(not(test))]
            buffer.commit();
            #[cfg(test)]
            self.commit_or_hold(&buffer);
            subs.next_sequence = next;
            subs.pending.insert(sequence, vec![buffer]);
            for id in bound_ids {
                if let Some(entry) = buffers.map.get_mut(&id) {
                    entry.last_submission = Some(sequence);
                }
            }
            Ok(SubmitToken::__backend_new(generation, sequence, self.id))
        }

        /// device を組み立てる。`request_with` と試験が共有する。
        fn new(
            id: u64,
            generation: u64,
            adapter_name: String,
            limits: LimitsTable,
            device: Retained<ProtocolObject<dyn MTLDevice>>,
            queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
        ) -> Self {
            Self {
                id,
                generation: AtomicU64::new(generation),
                #[cfg(feature = "fault-injection")]
                lost: super::LostState::default(),
                ownership: DeviceOwnership::ProductIndependent,
                adapter_name,
                limits,
                device,
                queue,
                buffers: Mutex::new(Buffers {
                    next_id: 1,
                    map: HashMap::new(),
                    next_slot: 1,
                    pool: SizePool::new(),
                    pending: PendingReleases::new(),
                }),
                pipelines: Mutex::new(Pipelines {
                    next_id: AtomicU64::new(1),
                    map: HashMap::new(),
                }),
                open: Arc::new(OpenEncoders::new()),
                submissions: Mutex::new(Submissions::new()),
                #[cfg(test)]
                commit_hold: Mutex::new(None),
            }
        }

        /// 試験専用。確定を遅らせる状態なら溜め、そうでなければ確定する。`commit_records` から投入の錠の中で呼ぶ。
        ///
        /// 錠の順は 投入の錠 -> `commit_hold` の錠。`release_held_commits` は `commit_hold` の錠だけを取る。
        #[cfg(test)]
        fn commit_or_hold(&self, buffer: &CommandBuffer) {
            let mut hold = self.commit_hold.lock().unwrap_or_else(|e| e.into_inner());
            match hold.as_mut() {
                Some(held) => held.push(buffer.clone()),
                None => buffer.commit(),
            }
        }

        /// 試験専用。以後の投入の確定を遅らせる。
        #[cfg(test)]
        fn hold_commits(&self) {
            let mut hold = self.commit_hold.lock().unwrap_or_else(|e| e.into_inner());
            hold.get_or_insert_with(Vec::new);
        }

        /// 試験専用。確定を遅らせる状態をやめ、溜めたコマンドバッファを投入順に確定する。何度呼んでもよい。
        #[cfg(test)]
        fn release_held_commits(&self) -> Vec<CommandBuffer> {
            let held = self
                .commit_hold
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take();
            let held: Vec<CommandBuffer> = held.into_iter().flatten().collect();
            for buffer in &held {
                buffer.commit();
            }
            held
        }

        /// 登録済みの callback のうち終端になったものを、呼び出し元のスレッドで 1 回ずつ呼ぶ。
        ///
        /// `generation` は呼び出した操作が読んだ今の世代。世代がこれと違う登録は、列の状態に関わらず `Lost` で呼ぶ。
        ///
        /// `submit`・`submit_many`・`poll`・`poll_ex`・`wait_blocking`・`allocate` の中で呼ぶ駆動点。本体は `drive_table`。
        fn drive_callbacks(&self, generation: u64) {
            drive_table(
                &self.submissions,
                generation,
                || self.current_generation(),
                group_state,
                |callback: Callback, state| callback(state),
            );
        }

        /// 駆動してから、照合と払い出しに使う今の世代を返す（理由は `driven_generation`）。
        fn drive_and_generation(&self) -> u64 {
            driven_generation(
                |generation| self.drive_callbacks(generation),
                || self.current_generation(),
            )
        }

        /// 対象の列の完了を呼び出し元のスレッドで待ち、その後の `poll` の値を返す。`wait_blocking(None)` が使う。
        ///
        /// 列を clone して表の錠を放してから待つ。待つ間も同じ錠を取る `poll`・`submit` を妨げない（錠を握ったまま待たない）。
        #[cfg(feature = "native-blocking-wait")]
        fn wait_for_completion(&self, token: SubmitToken, record_timing: bool) -> CompletionState {
            // 列は回収の前に取る（完了済みで先頭の列は回収で表から外れ、時刻を読めなくなる）。
            let group = {
                let mut subs = self.lock_submissions();
                let group = subs.pending.get(&token.sequence).cloned();
                subs.reclaim(group_state);
                group
            };
            if let Some(group) = &group {
                for buffer in group {
                    buffer.waitUntilCompleted();
                }
            }
            // 時刻を書くのは `wait_blocking(None)` だけ（書くかどうかは `complete_wait` が決める）。
            let span = group
                .as_ref()
                .filter(|group| group_state(group) == GroupState::Completed)
                .and_then(|group| {
                    span_of(
                        group
                            .iter()
                            .map(|buffer| (buffer.GPUStartTime(), buffer.GPUEndTime())),
                    )
                });
            self.lock_submissions()
                .complete_wait(token.sequence, span, record_timing);
            self.poll(token)
        }

        /// 試験専用。投入に成功した回数の通算。
        #[cfg(test)]
        fn test_submitted_count(&self) -> u64 {
            self.lock_submissions().next_sequence - 1
        }

        /// 試験専用。再利用の一覧から取り出した回数の通算。
        #[cfg(test)]
        fn test_pool_reuse_count(&self) -> u64 {
            self.lock_buffers().pool.reuse_count
        }

        /// 試験専用。取り出した slot が直前に一覧へ入れた slot と一致した回数の通算。
        #[cfg(test)]
        fn test_pool_slot_match_count(&self) -> u64 {
            self.lock_buffers().pool.slot_match_count
        }

        /// 試験専用。再利用の一覧にあるバッファの数。
        #[cfg(test)]
        fn test_pool_len(&self) -> usize {
            self.lock_buffers().pool.entries
        }

        /// 試験専用。解放の保留の一覧にあるバッファの数。
        #[cfg(test)]
        fn test_pending_release_count(&self) -> usize {
            self.lock_buffers().pending.len()
        }

        /// 試験専用。ハンドルが指す表のバッファの slot。表に無ければ `None`。
        #[cfg(test)]
        fn test_slot_of(&self, handle: &BufferHandle) -> Option<u64> {
            self.lock_buffers()
                .map
                .get(&handle.__backend_id())
                .map(|entry| entry.slot)
        }

        /// 試験専用。開いている記録先の数。
        #[cfg(test)]
        fn test_open_encoder_count(&self) -> usize {
            self.open.lock().len()
        }

        /// 表から外したバッファを全バイト 0 にしてから再利用の一覧へ回す。受け入れられなかったものは解放する。
        ///
        /// どの錠も持たない所から呼ぶ（一覧の検査と入れる処理が、表の錠を取り直す）。
        fn retire_entry(&self, entry: BufferEntry) {
            // 一覧の分類は、ハンドルの申告ではなく Metal のバッファの実際の長さで決める。
            let Ok(byte_len) = u64::try_from(entry.buffer.length()) else {
                return;
            };
            // 表から外したので、このバッファはどのハンドルからも到達できない。ゼロ化は錠の外で行う。
            let rejected = retire(
                byte_len,
                entry,
                |len| self.lock_buffers().pool.would_accept(len),
                zero_fill,
                |entry| {
                    let BufferEntry {
                        buffer,
                        slot,
                        generation,
                        last_submission,
                    } = entry;
                    self.lock_buffers()
                        .pool
                        .put(byte_len, slot, buffer)
                        .map(|buffer| BufferEntry {
                            buffer,
                            slot,
                            generation,
                            last_submission,
                        })
                },
            );
            // 錠を放してから落とす。Metal のバッファの解放を錠の中で行わない。
            drop(rejected);
        }

        /// 解放の保留の一覧から、最後に使った投入が終わったバッファを外し、ゼロ化して再利用の一覧へ回す。
        ///
        /// 駆動する呼び出し（`submit`・`submit_many`・`allocate`・`poll`・`poll_ex`）が呼ぶ。`wait_blocking` は
        /// どの経路も最後に `poll` を通るので、そこで回収される。投入済みの表の錠を持った所からは呼ばない。
        fn release_finished(&self) {
            let finished = {
                let mut buffers = self.lock_buffers();
                let subs = self.lock_submissions();
                buffers
                    .pending
                    .take_finished(|sequence| subs.is_finished(sequence, group_state))
            };
            for entry in finished {
                self.retire_entry(entry);
            }
        }

        /// 最後に使った投入が終わっていないバッファの読み書きを `InvalidArgument` で拒む。
        ///
        /// 表の錠を持った所（`with_buffer` の `f` の中）から呼ぶ。投入済みの表の錠は判定の間だけ持つ。
        fn check_not_in_use(&self, entry: &BufferEntry) -> Result<(), ContractError> {
            if let Some(sequence) = entry.last_submission
                && !self.lock_submissions().is_finished(sequence, group_state)
            {
                return Err(ContractError::InvalidArgument(
                    "buffer is in use by an unfinished submission".into(),
                ));
            }
            Ok(())
        }

        /// ハンドルを照合し、この device のバッファの表の項目を `&mut` で借りて `f` を呼ぶ。`f` の間は表の錠を持つ。
        ///
        /// 照合の順は、払い出し元の device の識別子（違えば `DeviceMismatch`）、表にある識別子、
        /// 世代の一致（表に無い・世代が違うものは偽造として `InvalidArgument`）。
        fn with_buffer<R>(
            &self,
            handle: &BufferHandle,
            f: impl FnOnce(&mut BufferEntry) -> Result<R, ContractError>,
        ) -> Result<R, ContractError> {
            if handle.__backend_device_id() != self.id {
                return Err(ContractError::DeviceMismatch);
            }
            let mut buffers = self.lock_buffers();
            let Some(entry) = buffers.map.get_mut(&handle.__backend_id()) else {
                return Err(ContractError::InvalidArgument(
                    "buffer handle is not in this device's table".into(),
                ));
            };
            if handle.__backend_generation() != self.current_generation() {
                return Err(ContractError::InvalidArgument(
                    "buffer handle generation does not match this device".into(),
                ));
            }
            f(entry)
        }

        /// 範囲を検査してから `[offset, offset + len)` を読み出す。`download_*` が共有する。
        fn read_range(
            &self,
            src: &BufferHandle,
            src_offset: u64,
            byte_len: u64,
        ) -> Result<Vec<u8>, ContractError> {
            self.with_buffer(src, |entry| {
                self.check_not_in_use(entry)?;
                let buffer = &entry.buffer;
                let allocation_len = u64::try_from(buffer.length()).map_err(|_| {
                    ContractError::InvalidArgument("buffer length does not fit u64".into())
                })?;
                // `host_range` が検査と `usize` への変換を行い、収まらなければ `InvalidArgument` を返す。
                let (offset, len) = host_range(src_offset, byte_len, allocation_len)?;
                // 確保できなければ abort せず `AllocationFailed` で返す。
                let mut out: Vec<u8> = Vec::new();
                out.try_reserve_exact(len)
                    .map_err(|_| ContractError::AllocationFailed {
                        requested_bytes: byte_len,
                    })?;
                out.resize(len, 0);
                if len > 0 {
                    let base = buffer.contents().as_ptr().cast::<u8>().cast_const();
                    // SAFETY: バッファは shared の storage なので `contents()` は CPU から読める
                    // `buffer.length()` バイトの領域を指す。`host_range`（`validate_buffer_range` を呼ぶ）が
                    // `offset + len <= length()` を保証している。GPU の書き込みと重ならないことは、
                    // 最後に使った投入が完了していることを、表の錠と投入の錠の中で
                    // `check_not_in_use` が確かめてからコピーすることに根拠を置く。
                    // `with_buffer` が表の錠を持ったまま
                    // `f` を呼ぶため、コピーの間は `free` でバッファが落ちない。`out` は新しく確保した
                    // `len` バイトの領域で、Metal の領域とは重ならない。
                    unsafe {
                        core::ptr::copy_nonoverlapping(base.add(offset), out.as_mut_ptr(), len);
                    }
                }
                Ok(out)
            })
        }
    }

    impl Drop for MetalDevice {
        /// 登録が残っていれば、その列の完了を待ってから、残りの callback を `poll` と同じ判定の値で 1 回ずつ呼ぶ。
        ///
        /// 登録が無ければ待たない。待ちに上限は無く、確定済みの command buffer が終わるまで戻らない。
        /// 「確定済みの command buffer は必ず終わる」ことに頼る。待つのは登録のある列だけで、登録の無い確定済みの
        /// command buffer は、落としても Metal が参照するバッファを保持する。callback の panic は捨てる。本体は `finish_table`。
        fn drop(&mut self) {
            let generation = self.generation.load(Ordering::Acquire);
            let subs = self
                .submissions
                .get_mut()
                .unwrap_or_else(|e| e.into_inner());
            finish_table(
                subs,
                generation,
                |group| {
                    for buffer in group {
                        buffer.waitUntilCompleted();
                    }
                },
                group_state,
                |callback: Callback, state| callback(state),
            );
        }
    }

    impl Capabilities for MetalDevice {
        /// この段の `query` は、どの機能にも `Unsupported` を返す。
        fn query(&self, _feature: FeatureId) -> CapabilityLevel {
            CapabilityLevel::Unsupported
        }

        fn limits(&self) -> &LimitsTable {
            &self.limits
        }

        /// どの機能も未知として、理由 `UnknownFeature` を添えて `Unsupported` を返す。
        fn query_ex(&self, _feature: FeatureId) -> CapabilityQuery {
            CapabilityQuery {
                level: CapabilityLevel::Unsupported,
                unsupported_reason: Some(UnsupportedReason::UnknownFeature),
                evidence: None,
            }
        }
    }

    /// 模擬のロストだけを持つ故障の差し込みの口。実際の故障ではない。
    ///
    /// `arm_gate`・`open_gate`・`simulate_runtime_error`・`simulate_validation_report` は既定の本体のまま
    /// （`UnsupportedExtensionDefault`）。ロストの後の操作の拒否はここでは持たない。
    #[cfg(feature = "fault-injection")]
    impl FaultHooks for MetalDevice {
        /// 1 回目は理由を記録し、世代を取り直し、登録されたコールバックを錠の外で 1 回呼ぶ。2 回目以降は何もせず `Ok`。
        /// 世代の源が尽きていたら、状態を変えずに `BackendFailure` を返す。
        fn simulate_device_lost(&self, reason: DeviceLostReason) -> Result<(), ContractError> {
            let first = self
                .lost
                .lose(reason, &self.generation, &NEXT_GENERATION)
                .map_err(|_| ContractError::BackendFailure {
                    op: "simulate_device_lost",
                    backend: BACKEND,
                    detail: "device generation space is exhausted".into(),
                })?;
            if first {
                // device のロストのコールバックの後に、世代の古い完了の callback を `Lost` で 1 回ずつ呼ぶ。
                // 錠はどちらも放してある。世代はここで 1 回だけ読む。
                sweep_stale_table(
                    &self.submissions,
                    self.current_generation(),
                    |callback: Callback, state| callback(state),
                );
            }
            Ok(())
        }
    }

    /// ロストの通知先の登録。登録済みなら上書きし、ロストの後の登録は保持するだけで呼ばない。
    #[cfg(feature = "fault-injection")]
    impl DeviceLostNotify for MetalDevice {
        fn set_lost_callback(&self, callback: LostCallback) {
            self.lost.set_callback(callback);
        }
    }

    impl Device for MetalDevice {
        fn backend(&self) -> BackendKind {
            BACKEND
        }

        fn ownership(&self) -> &DeviceOwnership {
            &self.ownership
        }

        fn generation(&self) -> u64 {
            self.current_generation()
        }

        #[cfg(feature = "fault-injection")]
        fn fault_hooks(&self) -> Option<&dyn FaultHooks> {
            Some(self)
        }

        #[cfg(feature = "artifact-inspection")]
        fn artifact_hooks(&self) -> Option<&dyn ArtifactHooks> {
            Some(self)
        }

        /// `create_pipeline` を持つ `ComputeOps` の実装（この型自身）を返す（TASK-2956 のポインタ）。
        fn compute(&self) -> Option<&dyn ComputeOps> {
            Some(self)
        }

        fn memory(&self) -> &dyn MemoryOps {
            self
        }

        fn queue(&self) -> &dyn CommandQueue {
            self
        }

        fn diagnostics(&self) -> DeviceDiagnostics {
            DeviceDiagnostics {
                adapter_name: self.adapter_name.clone(),
                backend_name: BACKEND_NAME,
            }
        }
    }

    impl CommandQueue for MetalDevice {
        /// 記録先の識別子と空の記録の列だけを作る。コマンドバッファは作らず、何も投入しない。
        ///
        /// 識別子はプロセス全体の通し番号で、この device の開いている表に入れる。
        /// 投入せずに破棄すると hook が、積んだ記録ごと表から外す。
        fn create_encoder(&self) -> CommandEncoder {
            let closer: Arc<dyn __BackendSharedEncoderCloser> = self.open.clone();
            match __backend_next_encoder_id() {
                Some(id) => {
                    self.open.lock().insert(id, Vec::new());
                    CommandEncoder::__backend_new_shared(id, closer)
                }
                // 通し番号が尽きた場合の id 0 は集合に入れない。投入は `InvalidArgument` で拒まれる。
                None => CommandEncoder::__backend_new_shared(0, closer),
            }
        }

        /// 記録先に積んだ dispatch を、コマンドバッファ 1 つの compute encoder に積んで確定する。
        ///
        /// 空でない `wait` は何も投入せず `Unsupported`。記録先がこの device の開いている表に無ければ
        /// `InvalidArgument`（別の device の記録先を含む）で、何も投入しない。記録先は取り出した時点で消費され、
        /// 束縛のバッファが投入の前に解放されていた場合（COMP-39）も `InvalidArgument` で、何も積まず通し番号も払い出さない。
        /// 積めたら、束縛した各バッファの最後に使った投入が、払い出した通し番号になる。記録が無いときは空のコマンドバッファを確定する。
        fn submit(
            &self,
            mut commands: CommandEncoder,
            wait: &[ImportedSemaphoreHandle],
        ) -> Result<SubmitToken, ContractError> {
            // 駆動は利用者の callback を呼ぶので、再入のロストで世代が動き得る。払い出しの世代は駆動の後に読む。
            let generation = self.drive_and_generation();
            self.release_finished();
            precheck_submit(wait)?;
            let records = self.open.claim(&[commands.__backend_id()])?;
            commands.__backend_disarm_closer();
            self.commit_records(generation, records)
        }

        /// 複数の記録先を 1 回で投入し、1 つの印を返す。
        ///
        /// 順は、`after` の照合（別 device は `DeviceMismatch`、世代違いは `DeviceLost`）、
        /// 空の列（`InvalidArgument`）、`wait`（`Unsupported`）、記録先の所属（`InvalidArgument`）、
        /// 束縛のバッファの確かめ（`InvalidArgument`）。検査に当たると何も投入しない。
        /// コマンドバッファは 1 つで、記録のある記録先ごとに compute encoder を渡した順に積む（記録の無い記録先は開かない）。
        /// 単一の列でコマンドバッファは投入順に完了するので、`after` は照合だけで追加の待ちは入れない。
        fn submit_many(
            &self,
            mut commands: Vec<CommandEncoder>,
            after: &[SubmitToken],
            wait: &[ImportedSemaphoreHandle],
        ) -> Result<SubmitToken, ContractError> {
            // 駆動は利用者の callback を呼ぶので、再入のロストで世代が動き得る。
            // `after` の照合と印の払い出しには、駆動の後に読んだ同じ値を使う。
            let generation = self.drive_and_generation();
            self.release_finished();
            precheck_submit_many(commands.len(), after, wait, self.id, generation)?;
            let ids: Vec<u64> = commands.iter().map(CommandEncoder::__backend_id).collect();
            let records = self.open.claim(&ids)?;
            for command in &mut commands {
                command.__backend_disarm_closer();
            }
            self.commit_records(generation, records)
        }

        /// 列の全部が完了なら `Complete`、どれかが `Error` なら `Lost`、他は `Pending`。
        ///
        /// 別 device の印・世代違いの印は `Lost`。回収済みの番号は watermark と失敗の集合で答える。
        ///
        /// 回収のあとで、終端になった登録済みの callback を呼ぶ（呼び出しが戻る前、このスレッドで）。
        fn poll(&self, token: SubmitToken) -> CompletionState {
            let generation = self.current_generation();
            if check_token_owner(&token, self.id, generation).is_err() {
                return CompletionState::Lost;
            }
            let state = {
                let mut subs = self.lock_submissions();
                subs.reclaim_and_state_of(token.sequence, group_state)
            };
            self.release_finished();
            self.drive_callbacks(generation);
            state
        }

        /// `poll` の詳しい版。未完了は `Pending`、完了は `Complete`、`Error` で終わった列は `RuntimeError`。
        ///
        /// 別 device の印・世代違いの印は `poll` と同じく `Lost`。払い出していない番号（0 を含む）は `Unknown`。
        /// 回収済みの番号は watermark と失敗の集合で答える（失敗した投入を `Complete` とは返さない）。
        /// `poll` は失敗した列と払い出していない番号を `Lost` とし、`poll_ex` はこれを `RuntimeError`・`Unknown` に分ける。
        /// `Pending`・`Complete` は `poll` と同じ。
        ///
        /// 回収のあとで、終端になった登録済みの callback を呼ぶ（`poll` と同じく、錠を放してからこのスレッドで）。
        fn poll_ex(&self, token: SubmitToken) -> CompletionStateEx {
            let generation = self.current_generation();
            if check_token_owner(&token, self.id, generation).is_err() {
                return CompletionStateEx::Lost;
            }
            let state = {
                let mut subs = self.lock_submissions();
                subs.reclaim_and_state_ex_of(token.sequence, group_state)
            };
            self.release_finished();
            self.drive_callbacks(generation);
            state
        }

        /// 終端の状態（`Complete` か `Lost`）で `callback` をちょうど 1 回呼ぶ。`Pending` は渡さない。待たず、ブロックしない。
        ///
        /// 別 device の印・世代違いの印・表に無い番号・回収済みの番号は、登録の呼び出しの中でその場で呼ぶ。
        /// 未完了なら登録して戻り、`submit`・`submit_many`・`poll`・`poll_ex`・`wait_blocking`・`allocate` の
        /// どれかの中で終端になったと分かった時に、その呼び出しを行ったスレッドで呼ぶ（待ちのスレッドも完了ハンドラも使わない）。
        /// 登録は回収せず、ほかの登録済みの callback も呼ばない。callback は錠の外で呼ぶ。
        ///
        /// # 注意
        ///
        /// 上の駆動する呼び出し以外（`upload`・`download_*`・`free` など）では callback を呼ばない。
        /// 呼ばれるには駆動する呼び出しを続ける必要がある。駆動しなければ、`MetalDevice` を落とすまで残る
        /// （落とすときに、その列の完了を待ってから 1 回呼ぶ。待ちに上限は無い）。
        ///
        /// callback の panic は、駆動した呼び出し（`submit` 系・`allocate`・`poll`・`wait_blocking`）から伝わる。
        /// そのとき `submit` 系に渡した `CommandEncoder` は閉じられて戻らず、`wait_blocking` は結果を返さずに巻き戻る。
        /// 登録の呼び出しの中でその場で呼んだ callback の panic は、`on_complete` 自身から伝わる。
        /// その場で呼ぶときも錠の外で呼ぶので、表の錠は毒化せず、そのあとも登録・駆動を続けられる。
        /// これは利用者の callback の panic を伝えるもので、この背後実装自身は panic しない。
        /// 残りの callback は panic のあとでも呼び切る。2 個目以降の panic の中身の `Drop` が panic しても、そこで止めずに呼び切る。
        /// `Drop` の中の callback の panic と、その中身の破棄の panic は捨てる。
        fn on_complete(
            &self,
            token: SubmitToken,
            callback: Box<dyn FnOnce(CompletionState) + Send>,
        ) {
            let call = |callback: Callback, state| callback(state);
            if check_token_owner(&token, self.id, self.current_generation()).is_err() {
                call(callback, CompletionState::Lost);
                return;
            }
            // 登録に token の世代を持たせる。ロストと重なって掃き出しをすり抜けても、駆動が `Lost` で呼ぶ。
            register_table(
                &self.submissions,
                token.sequence,
                token.generation,
                || self.current_generation(),
                callback,
                group_state,
                call,
            );
        }

        /// 完了まで待つ。`None` はコマンドバッファの完了を待ち、`Some` は 1 ms 間隔で `poll` を確かめる。
        ///
        /// 待つ間は表の錠を持たない。失敗した投入は `BackendFailure`。別 device の印は `DeviceMismatch`、世代違いは `DeviceLost`。
        #[cfg(feature = "native-blocking-wait")]
        fn wait_blocking(
            &self,
            token: SubmitToken,
            timeout: Option<std::time::Duration>,
        ) -> Result<(), ContractError> {
            check_token_owner(&token, self.id, self.current_generation())?;
            match timeout {
                None => wait_result(self.wait_for_completion(token, true)),
                Some(limit) => {
                    let started = std::time::Instant::now();
                    loop {
                        match self.poll(token) {
                            CompletionState::Pending => {}
                            done => return wait_result(done),
                        }
                        let elapsed = started.elapsed();
                        if elapsed >= limit {
                            return Err(ContractError::Timeout);
                        }
                        std::thread::sleep(
                            (limit - elapsed).min(std::time::Duration::from_millis(1)),
                        );
                    }
                }
            }
        }

        /// 投入の GPU 側の開始・終了時刻（秒）。読み取り専用で、回収も `on_complete` の駆動も状態の変更もしない。
        ///
        /// 時刻は、feature `native-blocking-wait` の `wait_blocking(token, None)` が完了を検出した時にだけ device の表へ書かれる
        /// （`poll`・`poll_ex`・`submit` の回収と、期限付きの待ちは書かない）。表の上限は 1,024 件で、
        /// 超えたら通し番号の最も小さいものから捨てる。返す値:
        ///
        /// - `Available`: 表に時刻がある。開始は列のコマンドバッファの最小、終了は最大で、終了は開始以上。差分は呼び出し側で取る
        /// - `Pending`: 投入済みで未完了
        /// - `Reclaimed`: 完了して回収された、時刻を残さずに完了した、または表から捨てられた投入
        /// - `Unavailable`: `Error` で終わった投入、別 device の印、世代違いの印、払い出していない番号（0 を含む）
        fn gpu_timing(&self, token: SubmitToken) -> GpuTiming {
            if check_token_owner(&token, self.id, self.current_generation()).is_err() {
                return GpuTiming::Unavailable;
            }
            self.lock_submissions()
                .timing_of(token.sequence, group_state)
        }

        // dispatch の記録は TASK-775 のポインタ。
        /// 引数を検査し、通ったら記録先に dispatch を 1 件積む。command buffer と encoder は投入の時に作る。
        ///
        /// 照らす順は、記録先（この device の開いている表にあるか。表から外さない）、パイプライン
        /// （別の device は `DeviceMismatch`、表に無ければ `InvalidArgument`）、束縛のバッファ（並びの順。
        /// 別の device は `DeviceMismatch`、表に無い・世代違いは `InvalidArgument`）、その後に引数の値の検査。
        /// 拒否は記録先の表・device の状態を変えない。通ったら記録を積んで `Ok(())` を返す。
        fn dispatch_ex(
            &self,
            encoder: &mut CommandEncoder,
            pipeline: &fandhe_silicon_core::execution::PipelineHandle,
            bindings: &[fandhe_silicon_core::execution::Binding<'_>],
            consts: &[fandhe_silicon_core::execution::ConstantBinding<'_>],
            dispatch_dims: [u32; 3],
            threads_per_threadgroup: [u32; 3],
            threadgroup_memory: &[fandhe_silicon_core::execution::ThreadgroupMemory],
        ) -> Result<(), ContractError> {
            if !self.open.is_open(encoder.__backend_id()) {
                return Err(ContractError::InvalidArgument(
                    "encoder is not open on this device".into(),
                ));
            }

            if pipeline.__backend_device_id() != self.id {
                return Err(ContractError::DeviceMismatch);
            }
            let (layout_slots, pipeline_max_total_threads, pipeline_static_threadgroup_memory) = {
                let pipelines = self.lock_pipelines();
                let Some((state, layout)) = pipelines.map.get(&pipeline.__backend_id()) else {
                    return Err(ContractError::InvalidArgument(
                        "pipeline handle is not in this device's table".into(),
                    ));
                };
                (
                    layout.slots.clone(),
                    to_u64(state.maxTotalThreadsPerThreadgroup())?,
                    to_u64(state.staticThreadgroupMemoryLength())?,
                )
            };

            let mut binding_args = Vec::with_capacity(bindings.len());
            for binding in bindings {
                let buffer_len =
                    self.with_buffer(binding.buffer, |entry| to_u64(entry.buffer.length()))?;
                binding_args.push(BindingArg {
                    slot: binding.slot,
                    offset: binding.offset,
                    buffer_len,
                });
            }
            let constant_args: Vec<ConstantArg> = consts
                .iter()
                .map(|c| ConstantArg {
                    slot: c.slot,
                    len: c.bytes.len(),
                })
                .collect();
            let threadgroup_args: Vec<ThreadgroupArg> = threadgroup_memory
                .iter()
                .map(|t| ThreadgroupArg {
                    index: t.index,
                    bytes: t.bytes,
                })
                .collect();

            let max_threads = self.device.maxThreadsPerThreadgroup();
            check_dispatch_args(&DispatchArgs {
                dispatch_dims,
                threads_per_threadgroup,
                bindings: &binding_args,
                constants: &constant_args,
                threadgroup_memory: &threadgroup_args,
                layout_slots: &layout_slots,
                pipeline_max_total_threads,
                pipeline_static_threadgroup_memory,
                device_max_threadgroup_memory: to_u64(self.device.maxThreadgroupMemoryLength())?,
                device_max_threads_per_threadgroup: [
                    to_u64(max_threads.width)?,
                    to_u64(max_threads.height)?,
                    to_u64(max_threads.depth)?,
                ],
                max_dispatch_dims: self.limits.max_dispatch_dims,
            })?;

            let record = DispatchRecord {
                pipeline: pipeline.__backend_id(),
                bindings: bindings
                    .iter()
                    .map(|binding| RecordedBinding {
                        slot: binding.slot,
                        buffer_id: binding.buffer.__backend_id(),
                        generation: binding.buffer.__backend_generation(),
                        offset: binding.offset,
                    })
                    .collect(),
                constants: consts
                    .iter()
                    .map(|constant| (constant.slot, constant.bytes.to_vec()))
                    .collect(),
                dispatch_dims,
                threads_per_threadgroup,
                threadgroup_memory: threadgroup_memory
                    .iter()
                    .map(|memory| (memory.index, memory.bytes))
                    .collect(),
            };
            self.open.record(encoder.__backend_id(), record)?;
            encoder.__backend_record();
            Ok(())
        }

        /// `limits()` の `max_buffer_bytes` と同じ値を返す。
        fn max_buffer_bytes(&self) -> u64 {
            self.limits.max_buffer_bytes
        }

        // TASK-436 のポインタ。
        /// 引数の宣言を C8、C7、C4、C5、C6 の順に照らし、通ったら `dispatch_ex` を 1 回呼んでその結果を返す。
        ///
        /// C4 の容量は束縛のバッファの実際の長さ、上限は `limits()` の `max_buffer_bytes`。C8・C7 の違反は
        /// ハンドルを表で引かずに返す。C4 の段で束縛のハンドルを並びの順に照らし、別の device のものは
        /// `DeviceMismatch`、表に無いもの・世代や大きさが表の項目と違うものは `InvalidArgument` で返す
        /// （どちらも `dispatch_ex` は呼ばない）。項目は BIND-52。
        // 引数は dispatch の宣言の並びそのままで、まとめる型を挟まない。
        #[allow(clippy::too_many_arguments)]
        fn dispatch_checked(
            &self,
            encoder: &mut CommandEncoder,
            pipeline: &PipelineHandle,
            bindings: &[fandhe_silicon_core::execution::Binding<'_>],
            consts: &[fandhe_silicon_core::execution::ConstantBinding<'_>],
            declared: &[OperandSpec<'_>],
            dispatch_dims: [u32; 3],
            threads_per_threadgroup: [u32; 3],
            threadgroup_memory: &[fandhe_silicon_core::execution::ThreadgroupMemory],
        ) -> Result<(), ContractError> {
            let operand_bindings: Vec<OperandBinding> = bindings
                .iter()
                .map(|b| OperandBinding {
                    slot: b.slot,
                    offset: b.offset,
                })
                .collect();
            let operand_consts: Vec<OperandConstant<'_>> = consts
                .iter()
                .map(|c| OperandConstant {
                    slot: c.slot,
                    bytes: c.bytes,
                })
                .collect();
            let capacity = |i: usize| -> Result<u64, ContractError> {
                let Some(binding) = bindings.get(i) else {
                    return Err(ContractError::InvalidArgument(
                        "binding position is out of range".into(),
                    ));
                };
                self.with_buffer(binding.buffer, |entry| {
                    check_handle_record(
                        binding.buffer.__backend_generation(),
                        binding.buffer.size(),
                        entry.generation,
                        to_u64(entry.buffer.length())?,
                    )
                })
            };
            check_declared_operands(
                declared,
                &operand_bindings,
                &operand_consts,
                capacity,
                self.limits.max_buffer_bytes,
            )?;
            self.dispatch_ex(
                encoder,
                pipeline,
                bindings,
                consts,
                dispatch_dims,
                threads_per_threadgroup,
                threadgroup_memory,
            )
        }
    }

    /// Metal から読んだ `NSUInteger` を `u64` にする。収まらなければ切り詰めず `InvalidArgument` にする。
    fn to_u64(value: usize) -> Result<u64, ContractError> {
        u64::try_from(value)
            .map_err(|_| ContractError::InvalidArgument("value does not fit u64".into()))
    }

    /// `allocate` が Metal のバッファを作る前に通す検査。
    ///
    /// 共通の `validate_allocation_size` は上限だけを見て 0 を通す。長さ 0 を Metal に渡したときの
    /// 振る舞いには頼らず、呼び出し側の引数の誤りとして、この背後実装では 0 を先に
    /// `InvalidArgument` で拒む。
    fn validate_allocation_request(
        byte_len: u64,
        max_buffer_bytes: u64,
    ) -> Result<(), ContractError> {
        if byte_len == 0 {
            return Err(ContractError::InvalidArgument(
                "allocation size must be greater than zero".into(),
            ));
        }
        validate_allocation_size(byte_len, max_buffer_bytes)
    }

    /// 範囲を検査してから、`bytes` を `entry` のバッファの `offset` から書き込む。`upload` とゼロ化が共有する。
    ///
    /// 検査に当たると、バッファの内容を変えずに `InvalidArgument` を返す。
    /// 書き込みの排他は `&mut BufferEntry` が型で示す。`upload` は表の錠の中の `get_mut` で借りた項目を、
    /// ゼロ化は表から外して所有している項目を渡す。共有参照からは書けない。非公開。
    fn write_range(
        entry: &mut BufferEntry,
        offset: u64,
        bytes: &[u8],
    ) -> Result<(), ContractError> {
        let buffer = &entry.buffer;
        let len = u64::try_from(bytes.len())
            .map_err(|_| ContractError::InvalidArgument("data length does not fit u64".into()))?;
        let allocation_len = u64::try_from(buffer.length())
            .map_err(|_| ContractError::InvalidArgument("buffer length does not fit u64".into()))?;
        let (offset, _) = host_range(offset, len, allocation_len)?;
        if !bytes.is_empty() {
            let base = buffer.contents().as_ptr().cast::<u8>();
            // SAFETY: バッファは shared の storage なので `contents()` は CPU から書ける
            // `buffer.length()` バイトの領域を指し、`entry` が `Retained` を持つので書き込みの間は生きている。
            // `host_range`（`validate_buffer_range` を呼ぶ）が `offset + bytes.len() <= length()` を保証している。
            // `&mut BufferEntry` を受けているので、この領域を CPU 側で読み書きする他の参照は同時に存在しない
            // （表の項目は表の錠の中で、または表から外して単独で所有した状態でしか借りられない）。
            // GPU の読み書きと重ならないことは、最後に使った投入が完了していることの確かめに根拠を置く。
            // `upload` は表の錠と投入の錠の中で `check_not_in_use` が確かめてから書く。ゼロ化は、
            // `free` と `release_finished` が最後に使った投入の完了（`is_finished`）を確かめてから
            // 表の外で単独で所有した項目に対して行う。`bytes` は呼び出し側の借用した領域で、
            // Metal の領域とは重ならない。
            unsafe {
                core::ptr::copy_nonoverlapping(bytes.as_ptr(), base.add(offset), bytes.len());
            }
        }
        Ok(())
    }

    /// `entry` のバッファの全バイトを 0 にする。
    fn zero_fill(entry: &mut BufferEntry) -> Result<(), ContractError> {
        let total = u64::try_from(entry.buffer.length())
            .map_err(|_| ContractError::InvalidArgument("buffer length does not fit u64".into()))?;
        let mut offset: u64 = 0;
        while offset < total {
            let remaining = total - offset;
            let step = remaining.min(ZEROS.len() as u64);
            // `step` は `ZEROS.len()` 以下なので usize に収まる。
            let chunk = &ZEROS[..usize::try_from(step).unwrap_or(ZEROS.len())];
            write_range(entry, offset, chunk)?;
            offset += step;
        }
        Ok(())
    }

    /// `values` の `index` 番の function constant に、種別 `kind` の値 `bytes` を設定する。
    ///
    /// `ComputeOps::create_pipeline` が `prepare_constants` の結果を渡して呼ぶ。`U32` と `Bool` の両方を
    /// この 1 つの関数が扱う。`bytes` の長さが `kind` の大きさと違えば、Metal を呼ばずに失敗で返す。
    fn set_constant(
        values: &MTLFunctionConstantValues,
        index: u32,
        kind: ConstantKind,
        bytes: &[u8],
    ) -> Result<(), ContractError> {
        let failed = |detail: &str| ContractError::BackendFailure {
            op: "create_pipeline",
            backend: BACKEND,
            detail: detail.into(),
        };
        if bytes.len() != kind.byte_len() {
            return Err(failed("function constant size does not match its type"));
        }
        let data_type = match kind {
            ConstantKind::UInt => MTLDataType::UInt,
            ConstantKind::Bool => MTLDataType::Bool,
        };
        // 範囲は `prepare_constants` でも検査済みだが、この関数単体でも SAFETY の根拠を保つために再確認する。
        if index > MAX_FUNCTION_CONSTANT_INDEX {
            return Err(ContractError::InvalidArgument(format!(
                "function constant index out of range: {index} (max {MAX_FUNCTION_CONSTANT_INDEX})"
            )));
        }
        let index = usize::try_from(index).map_err(|_| failed("function constant index"))?;
        // 4 バイトにそろえた局所の値へ写して、その先頭を渡す。`UInt` は 4 バイト、`Bool` は 1 バイトだけが読まれる。
        let mut local = Aligned4([0_u8; 4]);
        local.0[..bytes.len()].copy_from_slice(bytes);
        let value = core::ptr::NonNull::from(&mut local).cast::<core::ffi::c_void>();
        // SAFETY: `setConstantValue_type_atIndex` に渡す `value` は、局所の値 `local` を指す非 null のポインタで、
        // この呼び出しの間だけ生きる。`local` は 4 バイトで 4 バイトにそろえてあり、Metal が読む大きさ
        // （`UInt` は 4 バイト、`Bool` は 1 バイト）とアラインメントが `data_type` と一致する。
        // `bytes` の長さは直前で `kind.byte_len()` と照合した。`index` は直前で MSL の範囲（0〜65535）に
        // 入ることを検査済みで、`usize` への変換で値は失われない。MSL 4.1 の 5.8 節は 0〜65535 を有効な
        // index とし、Apple の `makeFunction(name:constantValues:)` の文書は、関数に宣言の無い index の値を
        // 関数の生成で無視すると書く。範囲内なら宣言の有無を問わない。
        unsafe {
            values.setConstantValue_type_atIndex(value, data_type, index);
        }
        Ok(())
    }

    /// function constant の値を置く 4 バイトの領域。`UInt` の読み出しに要る 4 バイトのアラインメントをそろえる。
    #[repr(align(4))]
    struct Aligned4([u8; 4]);

    /// reflection の引数の数と、バッファの引数を slot の昇順で読む。
    // `arguments()` と `MTLArgument` の読み出しはこの関数だけが使う。
    #[allow(deprecated)]
    fn read_reflection(
        reflection: &MTLComputePipelineReflection,
    ) -> Result<PipelineReflection, ContractError> {
        let arguments = reflection.arguments().to_vec();
        let mut buffers = Vec::new();
        for a in &arguments {
            if a.r#type() == objc2_metal::MTLArgumentType::Buffer {
                buffers.push(classify_reflected_buffer(
                    a.index(),
                    a.bufferDataSize(),
                    metal_data_type(a.bufferDataType()),
                )?);
            }
        }
        buffers.sort_by_key(|b| b.slot);
        Ok(PipelineReflection {
            argument_count: arguments.len(),
            buffers,
        })
    }

    /// `MTLDataType` を reflection の要素の型に写す。対応表に無い型は `Other`。
    fn metal_data_type(t: MTLDataType) -> ReflectedDataType {
        match t {
            MTLDataType::Float => ReflectedDataType::Float,
            MTLDataType::Half => ReflectedDataType::Half,
            MTLDataType::UInt => ReflectedDataType::UInt,
            MTLDataType::Int => ReflectedDataType::Int,
            MTLDataType::Bool => ReflectedDataType::Bool,
            MTLDataType::Struct => ReflectedDataType::Struct,
            other => ReflectedDataType::Other(u64::try_from(other.0).unwrap_or(u64::MAX)),
        }
    }

    /// NSError の説明文を取り出す。
    fn describe(error: &objc2_foundation::NSError) -> String {
        error.localizedDescription().to_string()
    }

    impl MetalDevice {
        /// `create_pipeline` と同じ入力の分類・検査・コンパイル・特殊化で関数を作る。
        fn compile_function(
            &self,
            input: &PipelineInput<'_>,
            layout: &BindingLayout,
        ) -> Result<Retained<ProtocolObject<dyn MTLFunction>>, ContractError> {
            let (source, entry_point, constants, specialized) = classify_pipeline_input(input)?;
            let plan = compile_plan(input)?;
            check_unique_slots(&layout.slots)?;
            let prepared = prepare_constants(constants)?;
            let options = compile_options(plan)?;

            let rejected = |kind: &'static str, what: &str, error: &objc2_foundation::NSError| {
                ContractError::ArtifactRejected {
                    kind,
                    reason: format!("{what}: {}", describe(error)),
                }
            };
            let library = self
                .device
                .newLibraryWithSource_options_error(&NSString::from_str(source), options.as_deref())
                .map_err(|e| rejected("msl", "MSL compilation failed", &e))?;
            let name = NSString::from_str(entry_point);
            // 特殊化しない関数から function constant の宣言を読み、必須の定数が渡されているかを
            // 特殊化の前に自分で確かめる（Metal の結果に頼らない）。`Msl` は定数の並びが空として扱う。
            let base = library.newFunctionWithName(&name).ok_or_else(|| {
                ContractError::ArtifactRejected {
                    kind: "msl",
                    reason: format!("entry point `{entry_point}` was not found in the MSL source"),
                }
            })?;
            let declared: Vec<(usize, bool)> = base
                .functionConstantsDictionary()
                .allValues()
                .to_vec()
                .iter()
                .map(|c| (c.index(), c.required()))
                .collect();
            let provided: Vec<u32> = prepared.iter().map(|(index, _, _)| *index).collect();
            let missing = missing_required_constants(&declared, &provided);
            if !missing.is_empty() {
                return Err(ContractError::ArtifactRejected {
                    kind: "msl",
                    reason: missing_required_reason(&missing),
                });
            }
            let function = if !specialized {
                base
            } else {
                let values = MTLFunctionConstantValues::new();
                for (index, kind, bytes) in &prepared {
                    set_constant(&values, *index, *kind, bytes)?;
                }
                library
                    .newFunctionWithName_constantValues_error(&name, &values)
                    .map_err(|e| rejected("msl", "MSL function specialization failed", &e))?
            };
            Ok(function)
        }

        /// パイプライン状態を `layout` の写しと一緒に表に置き、handle を返す。
        fn register_pipeline(
            &self,
            state: Retained<ProtocolObject<dyn MTLComputePipelineState>>,
            layout: &BindingLayout,
        ) -> Result<PipelineHandle, ContractError> {
            let mut pipelines = self.lock_pipelines();
            let id =
                take_next(&pipelines.next_id).ok_or_else(|| ContractError::BackendFailure {
                    op: "create_pipeline",
                    backend: BACKEND,
                    detail: "pipeline id space is exhausted".into(),
                })?;
            pipelines.map.insert(id, (state, layout.clone()));
            Ok(PipelineHandle::__backend_new(id, self.id))
        }

        /// 関数から reflection 付きでパイプライン状態を作り、引数の数とバッファの引数ごとの
        /// slot・バイト長・型を返す。表には置かない。バイト長 0 は `ReflectedSize::Unchecked`、
        /// 型 `Struct` は `ReflectedDataType::Struct`。
        #[cfg_attr(not(test), allow(dead_code))]
        fn reflect_function(
            &self,
            function: &ProtocolObject<dyn MTLFunction>,
        ) -> Result<
            (
                Retained<ProtocolObject<dyn MTLComputePipelineState>>,
                PipelineReflection,
            ),
            ContractError,
        > {
            let descriptor = MTLComputePipelineDescriptor::new();
            descriptor.setComputeFunction(Some(function));
            let mut reflection = None;
            let state = self
                .device
                .newComputePipelineStateWithDescriptor_options_reflection_error(
                    &descriptor,
                    MTLPipelineOption::BindingInfo,
                    Some(&mut reflection),
                )
                .map_err(|e| ContractError::ArtifactRejected {
                    kind: "compute-pipeline",
                    reason: format!("pipeline state creation failed: {}", describe(&e)),
                })?;
            let reflection = reflection.ok_or_else(|| ContractError::BackendFailure {
                op: "create_pipeline",
                backend: BACKEND,
                detail: "pipeline reflection was not returned".into(),
            })?;
            Ok((state, read_reflection(&reflection)?))
        }
    }

    /// 公開の `Device`（`MetalDeviceProvider::request` が返す `Box<dyn Device>`）からは
    /// `Device::compute()` でこの実装へ届く（TASK-2956 のポインタ。`create_pipeline` の実装は TASK-773 のポインタ）。
    impl ComputeOps for MetalDevice {
        /// MSL のソースから compute パイプラインを作り、表に登録して返す。
        ///
        /// 受け付けるのは `Msl`・`MslSpecialized`・`MslWithOptions`・`MslSpecializedWithOptions`。
        /// オプション付きの入力は、`Safe` なら数学モードを設定し、`Fast` と `MslFloatFunctions::Fast` は
        /// 何も設定せず、`Precise` なら関数の精度を設定する。言語版は設定しない。設定の関数を持たない
        /// 環境では `Unsupported`。`Msl`・`MslSpecialized` は既定のオプションでコンパイルする。
        /// 入力の分類・`layout.slots` と function constant の
        /// 重複の検査は Metal の API を呼ぶ前に行う。ライブラリのコンパイルと、特殊化しない関数からの
        /// function constant の宣言の読み出しの後、特殊化の前に、必須の定数（`is_function_constant_defined` で
        /// 守られていないもの）がすべて渡されているかを確かめ、足りなければ `ArtifactRejected { kind: "msl" }`
        /// を返す。`Msl` は定数の並びが空として扱うので、必須の定数を持つカーネルは拒まれる。
        /// `MslSpecialized` は function constant が空でも特殊化の API（`newFunctionWithName_constantValues_error`）で
        /// 関数を作る。Metal 側の失敗は握りつぶさず `ArtifactRejected` に写す。
        /// `layout` は検査したうえで写しを表に持つだけで、カーネルの引数とは突き合わせない。
        fn create_pipeline(
            &self,
            input: PipelineInput<'_>,
            layout: &BindingLayout,
        ) -> Result<PipelineHandle, ContractError> {
            let function = self.compile_function(&input, layout)?;
            let state = self
                .device
                .newComputePipelineStateWithFunction_error(&function)
                .map_err(|e| ContractError::ArtifactRejected {
                    kind: "compute-pipeline",
                    reason: format!("pipeline state creation failed: {}", describe(&e)),
                })?;
            self.register_pipeline(state, layout)
        }

        /// MSL の 4 つの入力のキャッシュキーを返す（TASK-501 のポインタ）。パイプラインは作らない。
        ///
        /// 欄の作り方は `metal_cache_key`。`compiler_version`・`driver_version` は、OS の版の文字列
        /// （`NSProcessInfo` の `operatingSystemVersionString`）と device 名（`MTLDevice::name`）から作る。
        fn pipeline_cache_key(
            &self,
            input: &PipelineInput<'_>,
            bounds_checked: bool,
        ) -> Option<CacheKey> {
            let os_version = NSProcessInfo::processInfo()
                .operatingSystemVersionString()
                .to_string();
            let device_name = self.device.name().to_string();
            metal_cache_key(
                input,
                bounds_checked,
                cache_key_version(&os_version, &device_name),
            )
        }
    }

    /// feature `artifact-inspection` のビルドで `Device::artifact_hooks()` が返す実装（TASK-492 のポインタ）。
    #[cfg(feature = "artifact-inspection")]
    impl ArtifactHooks for MetalDevice {
        /// MSL を数学モードつきでコンパイルし、`path` へ binary archive を書き出して、書き出したバイト列を返す。
        ///
        /// `path` は絶対パス。Metal の失敗は `ArtifactRejected { kind: "binary-archive" }`。
        /// パイプライン状態は作らず、表にも置かない。
        fn compile_and_write_binary_archive(
            &self,
            source: &str,
            entry_point: &str,
            math_mode: MslMathMode,
            path: &std::path::Path,
        ) -> Result<Vec<u8>, ContractError> {
            let path_str = artifact_path_str(path)?;
            let input = PipelineInput::MslWithOptions {
                source,
                entry_point,
                math_mode,
            };
            let function = self.compile_function(&input, &BindingLayout::new(Vec::new()))?;
            let rejected =
                |what: &str, error: &objc2_foundation::NSError| ContractError::ArtifactRejected {
                    kind: "binary-archive",
                    reason: format!("{what}: {}", describe(error)),
                };
            let descriptor = MTLComputePipelineDescriptor::new();
            descriptor.setComputeFunction(Some(&function));
            let archive = self
                .device
                .newBinaryArchiveWithDescriptor_error(&MTLBinaryArchiveDescriptor::new())
                .map_err(|e| rejected("binary archive creation failed", &e))?;
            archive
                .addComputePipelineFunctionsWithDescriptor_error(&descriptor)
                .map_err(|e| rejected("adding the pipeline to the binary archive failed", &e))?;
            let url = NSURL::fileURLWithPath(&NSString::from_str(path_str));
            archive
                .serializeToURL_error(&url)
                .map_err(|e| rejected("binary archive serialization failed", &e))?;
            read_written_artifact(path, ARTIFACT_READ_LIMIT)
        }

        /// MSL と `path` の binary archive からパイプラインを作り、空の layout で表に置いて handle を返す。
        ///
        /// `path` は絶対パス。`fail_on_archive_miss` が `true` のときは archive に無いと作成が失敗する。
        /// archive の読み込みの失敗は `ArtifactRejected { kind: "binary-archive" }`、
        /// パイプライン状態の作成の失敗は `ArtifactRejected { kind: "compute-pipeline" }`。
        fn create_pipeline_from_archive_path(
            &self,
            source: &str,
            entry_point: &str,
            math_mode: MslMathMode,
            path: &std::path::Path,
            fail_on_archive_miss: bool,
        ) -> Result<PipelineHandle, ContractError> {
            let path_str = artifact_path_str(path)?;
            let input = PipelineInput::MslWithOptions {
                source,
                entry_point,
                math_mode,
            };
            let layout = BindingLayout::new(Vec::new());
            let function = self.compile_function(&input, &layout)?;
            let archive_descriptor = MTLBinaryArchiveDescriptor::new();
            archive_descriptor.setUrl(Some(&NSURL::fileURLWithPath(&NSString::from_str(path_str))));
            let archive = self
                .device
                .newBinaryArchiveWithDescriptor_error(&archive_descriptor)
                .map_err(|e| ContractError::ArtifactRejected {
                    kind: "binary-archive",
                    reason: format!("binary archive loading failed: {}", describe(&e)),
                })?;
            let descriptor = MTLComputePipelineDescriptor::new();
            descriptor.setComputeFunction(Some(&function));
            descriptor.setBinaryArchives(Some(&NSArray::from_retained_slice(&[archive])));
            let options = if fail_on_archive_miss {
                MTLPipelineOption::FailOnBinaryArchiveMiss
            } else {
                MTLPipelineOption::None
            };
            let state = self
                .device
                .newComputePipelineStateWithDescriptor_options_reflection_error(
                    &descriptor,
                    options,
                    None,
                )
                .map_err(|e| ContractError::ArtifactRejected {
                    kind: "compute-pipeline",
                    reason: format!("pipeline state creation failed: {}", describe(&e)),
                })?;
            self.register_pipeline(state, &layout)
        }

        /// `metallib_path` の metallib から `entry_point` の関数を読み、`declared` の slot の並びの layout で表に置いて handle を返す。
        ///
        /// slot の重複と宣言の次元数の検査、`metallib_path`（絶対パス）の検査は device とファイルに触る前に行う。
        /// MSL のソースからはコンパイルしない。読み込みの失敗と入口の名前が無いときは `ArtifactRejected { kind: "metallib" }`。
        fn create_pipeline_checked_from_metallib(
            &self,
            metallib_path: &std::path::Path,
            entry_point: &str,
            declared: &[OperandSpec<'_>],
        ) -> Result<PipelineHandle, ContractError> {
            let layout = BindingLayout::new(declared.iter().map(|d| d.slot).collect());
            check_unique_slots(&layout.slots)?;
            validate_declared_operands(declared, &layout)?;
            let path_str = artifact_path_str(metallib_path)?;
            let library = self
                .device
                .newLibraryWithURL_error(&NSURL::fileURLWithPath(&NSString::from_str(path_str)))
                .map_err(|e| ContractError::ArtifactRejected {
                    kind: "metallib",
                    reason: format!("metallib loading failed: {}", describe(&e)),
                })?;
            let function = library
                .newFunctionWithName(&NSString::from_str(entry_point))
                .ok_or_else(|| ContractError::ArtifactRejected {
                    kind: "metallib",
                    reason: format!("entry point `{entry_point}` was not found in the metallib"),
                })?;
            let (state, _reflection) = self.reflect_function(&function)?;
            self.register_pipeline(state, &layout)
        }
    }

    impl MemoryOps for MetalDevice {
        /// shared の storage でバッファを確保する。
        ///
        /// 長さ 0 と上限（`limits().max_buffer_bytes`）超えは `InvalidArgument`。
        /// 返すハンドルの `size`・`usage` は、受けた `byte_len`・`usage`。
        fn allocate(
            &self,
            byte_len: u64,
            usage: BufferUsage,
        ) -> Result<BufferHandle, ContractError> {
            // 駆動は利用者の callback を呼ぶので、再入のロストで世代が動き得る。払い出しの世代は駆動の後に読む。
            let generation = self.drive_and_generation();
            self.release_finished();
            validate_allocation_request(byte_len, self.limits.max_buffer_bytes)?;
            let failed = || ContractError::AllocationFailed {
                requested_bytes: byte_len,
            };
            let length = usize::try_from(byte_len).map_err(|_| failed())?;
            let mut buffers = self.lock_buffers();
            // ハンドルの識別子は再利用しない。解放済みのハンドルが新しい利用者のバッファに当たらないようにする。
            // 一覧から取る前に払い出すので、あふれて失敗しても一覧のバッファは減らない。
            let id = buffers.next_id;
            buffers.next_id = id.checked_add(1).ok_or_else(failed)?;
            // 同じ `byte_len` の再利用の一覧にあれば、それを使う（中身は一覧に入れる前に 0 にしてある）。
            // 無ければ Metal のバッファを新しく作る（確保 API が全バイトを 0 にする）。
            let (slot, buffer) = match buffers.pool.take(byte_len) {
                Some(pooled) => pooled,
                None => {
                    // 錠を放してから確保し、取り直す。
                    drop(buffers);
                    let buffer = self
                        .device
                        .newBufferWithLength_options(length, MTLResourceOptions::StorageModeShared)
                        .ok_or_else(failed)?;
                    buffers = self.lock_buffers();
                    let slot = buffers.next_slot;
                    buffers.next_slot = slot.checked_add(1).ok_or_else(failed)?;
                    (slot, buffer)
                }
            };
            buffers.map.insert(
                id,
                BufferEntry {
                    buffer,
                    slot,
                    generation,
                    last_submission: None,
                },
            );
            Ok(BufferHandle::__backend_new(
                generation, id, self.id, byte_len, usage,
            ))
        }

        /// `bytes` を `dst` の `dst_offset` から書き込む。範囲の検査に当たると、バッファの内容を変えずに返す。
        ///
        /// 最後に使った投入が終わっていないバッファは `InvalidArgument` で、内容を変えない。
        fn upload(
            &self,
            dst: &BufferHandle,
            dst_offset: u64,
            bytes: &[u8],
        ) -> Result<(), ContractError> {
            self.with_buffer(dst, |entry| {
                self.check_not_in_use(entry)?;
                write_range(entry, dst_offset, bytes)
            })
        }

        /// `src` の `src_offset` から `byte_len` バイトを読み出す。待たずに返る。
        ///
        /// 最後に使った投入が終わっていないバッファは `InvalidArgument` で、読まない。
        #[cfg(feature = "native-blocking-wait")]
        fn download_blocking(
            &self,
            src: &BufferHandle,
            src_offset: u64,
            byte_len: u64,
        ) -> Result<Vec<u8>, ContractError> {
            self.read_range(src, src_offset, byte_len)
        }

        /// 読み出してから、その場で `callback` を 1 回だけ呼ぶ。失敗も `callback` に渡す。
        ///
        /// 最後に使った投入が終わっていないバッファは `InvalidArgument` を渡し、読まない。
        fn download_async(
            &self,
            src: &BufferHandle,
            src_offset: u64,
            byte_len: u64,
            callback: Box<dyn FnOnce(Result<Vec<u8>, ContractError>) + Send>,
        ) {
            callback(self.read_range(src, src_offset, byte_len));
        }

        /// この device のハンドルなら、表から外す。最後に使った投入が終わっていれば（使った投入が無い場合を含む）、
        /// 全バイトを 0 にしてから再利用の一覧へ回す。終わっていなければ、解放の保留の一覧に置き、ゼロ化も一覧への回しもしない。
        ///
        /// 保留のバッファは、投入が終わった後の回収で、同じ流れ（ゼロ化してから一覧へ回す）に乗る。
        /// 一覧が上限（件数・大きさの合計）に達していれば、ゼロ化せずに解放する。ゼロ化に失敗したバッファも
        /// 一覧に入れず解放する（以前の内容を次の `allocate` の利用者に見せない側に倒す）。
        /// 一覧に入ったバッファは device の Drop まで保持される（合計の上限は 64 MiB）。
        /// 別の device のハンドルは何もしない（同じ識別子を持つ自 device のバッファを巻き込まない）。
        /// 照合は device の今の世代ではなく、表の項目を払い出したときの世代と比べるので、ロストの後でもロスト前のハンドルで解放できる。
        fn free(&self, buffer: BufferHandle) {
            if buffer.__backend_device_id() != self.id {
                return;
            }
            let entry = {
                let mut buffers = self.lock_buffers();
                // ロストの後でも、払い出したときの世代と一致するハンドルなら外して解放する（device の世代とは比べない）。
                let removed = remove_issued(
                    &mut buffers.map,
                    buffer.__backend_id(),
                    buffer.__backend_generation(),
                    |entry| entry.generation,
                );
                let Some(entry) = removed else {
                    return;
                };
                if let Some(sequence) = entry.last_submission {
                    // 錠の順は表 → 投入済みの表。
                    let finished = self.lock_submissions().is_finished(sequence, group_state);
                    if !finished {
                        buffers.pending.push(sequence, entry);
                        return;
                    }
                }
                entry
            };
            self.retire_entry(entry);
        }
    }

    /// `compile_options` が計画をオプションに書くことの確認。device には触らない。
    #[cfg(test)]
    mod compile_options_tests {
        use super::{
            CompilePlan, MTLCompileOptions, MTLMathFloatingPointFunctions, MTLMathMode,
            compile_options,
        };
        use objc2::runtime::NSObjectProtocol;
        use objc2::sel;

        #[test]
        fn default_plan_gives_no_options() {
            assert!(matches!(compile_options(CompilePlan::default()), Ok(None)));
        }

        #[test]
        fn plan_is_written_to_the_options_and_language_version_is_left_alone() {
            let baseline = MTLCompileOptions::new();
            for selector in [
                sel!(mathMode),
                sel!(mathFloatingPointFunctions),
                sel!(languageVersion),
                sel!(setMathMode:),
                sel!(setMathFloatingPointFunctions:),
            ] {
                assert!(
                    baseline.respondsToSelector(selector),
                    "MTLCompileOptions does not respond to {selector:?}"
                );
            }
            let plans = [(true, false), (false, true), (true, true)];
            for (safe_math, precise_functions) in plans {
                let plan = CompilePlan {
                    safe_math,
                    precise_functions,
                };
                let options = compile_options(plan)
                    .expect("options")
                    .expect("non-default plan gives options");
                let expected_math = if safe_math {
                    MTLMathMode::Safe
                } else {
                    baseline.mathMode()
                };
                let expected_functions = if precise_functions {
                    MTLMathFloatingPointFunctions::Precise
                } else {
                    baseline.mathFloatingPointFunctions()
                };
                assert_eq!(options.mathMode(), expected_math, "{plan:?}");
                assert_eq!(
                    options.mathFloatingPointFunctions(),
                    expected_functions,
                    "{plan:?}"
                );
                assert_eq!(options.languageVersion(), baseline.languageVersion());
            }
        }
    }

    /// `metal_data_type` の写しの確認。device には触らない。
    #[cfg(test)]
    mod reflection_type_tests {
        use super::{MTLDataType, ReflectedDataType, metal_data_type};

        #[test]
        fn the_six_mapped_types_are_copied() {
            assert_eq!(
                metal_data_type(MTLDataType::Float),
                ReflectedDataType::Float
            );
            assert_eq!(metal_data_type(MTLDataType::Half), ReflectedDataType::Half);
            assert_eq!(metal_data_type(MTLDataType::UInt), ReflectedDataType::UInt);
            assert_eq!(metal_data_type(MTLDataType::Int), ReflectedDataType::Int);
            assert_eq!(metal_data_type(MTLDataType::Bool), ReflectedDataType::Bool);
            assert_eq!(
                metal_data_type(MTLDataType::Struct),
                ReflectedDataType::Struct
            );
        }

        #[test]
        fn a_type_outside_the_table_keeps_its_raw_value() {
            assert_eq!(
                metal_data_type(MTLDataType::Float4),
                ReflectedDataType::Other(u64::try_from(MTLDataType::Float4.0).unwrap())
            );
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// `allocate` の前段の検査の境界。device には触らない。
        const LIMIT: u64 = 1024;

        #[test]
        fn zero_length_is_invalid_argument() {
            let r = validate_allocation_request(0, LIMIT);
            assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
        }

        #[test]
        fn zero_length_is_rejected_even_when_the_limit_is_zero() {
            let r = validate_allocation_request(0, 0);
            assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
        }

        #[test]
        fn one_byte_and_the_limit_itself_pass() {
            assert!(validate_allocation_request(1, LIMIT).is_ok());
            assert!(validate_allocation_request(LIMIT, LIMIT).is_ok());
        }

        #[test]
        fn one_past_the_limit_is_invalid_argument() {
            let r = validate_allocation_request(LIMIT + 1, LIMIT);
            assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
        }

        /// キーの版の欄に使う OS の版の文字列は空でなく、2 回読んで同じ。device には触らない。
        #[test]
        fn os_version_string_for_the_cache_key_is_not_empty_and_stable() {
            let read = || {
                NSProcessInfo::processInfo()
                    .operatingSystemVersionString()
                    .to_string()
            };
            let a = read();
            assert!(!a.is_empty());
            assert_eq!(a, read());
        }
    }

    /// device に触る試験専用の数の口の確認。実機の回でだけ流す。
    #[cfg(test)]
    mod device_tests {
        use super::*;
        use crate::backend_metal::POOL_MAX_ENTRIES;
        use fandhe_silicon_core::execution::{
            FunctionConstant, FunctionConstantValue, MslFloatFunctions, MslMathMode,
        };

        fn new_device() -> MetalDevice {
            // 具体型で数の口を読むため、`request_with` を通さず同じ手順で作る。
            let device = MTLCreateSystemDefaultDevice().expect("default device");
            let queue = device.newCommandQueue().expect("queue");
            MetalDevice::new(
                __backend_next_device_id().expect("id"),
                take_next(&NEXT_GENERATION).expect("generation"),
                String::new(),
                LimitsTable {
                    max_buffer_bytes: 1 << 20,
                    max_dispatch_dims: [u32::MAX; 3],
                    max_texture_size: MAX_TEXTURE_SIZE,
                },
                device,
                queue,
            )
        }

        /// reflection 付きの生成が表を変えずに値を返すことの確認（ART-89 のポインタ）。
        mod reflection {
            use super::*;
            use crate::backend_metal::expected_data_type;
            use crate::backend_metal::{ReflectedBuffer, ReflectedSize};
            use fandhe_silicon_core::execution::DType;

            const ONE_FLOAT_BUFFER: &str = "#include <metal_stdlib>\nusing namespace metal;\nkernel void k(device float* a [[buffer(0)]]) { a[0] = 1.0f; }\n";

            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn reflect_function_reads_one_float_buffer_and_leaves_the_table_alone() {
                let device = new_device();
                let layout = BindingLayout::new(vec![0]);
                let function = device
                    .compile_function(
                        &PipelineInput::Msl {
                            source: ONE_FLOAT_BUFFER,
                            entry_point: "k",
                        },
                        &layout,
                    )
                    .expect("compile");
                let before = device.lock_pipelines().map.len();
                let (state, reflection) = device.reflect_function(&function).expect("reflect");
                assert_eq!(
                    reflection,
                    PipelineReflection {
                        argument_count: 1,
                        buffers: vec![ReflectedBuffer {
                            slot: 0,
                            size: ReflectedSize::Bytes(4),
                            data_type: ReflectedDataType::Float,
                        }],
                    }
                );
                assert_eq!(device.lock_pipelines().map.len(), before);
                let handle = device.register_pipeline(state, &layout).expect("register");
                assert_eq!(device.lock_pipelines().map.len(), before + 1);
                assert_eq!(handle.__backend_device_id(), device.id);
            }

            const GEMM_KERNEL: &str = "#include <metal_stdlib>\n\
                using namespace metal;\n\
                struct GemmDims { uint m; uint n; uint k; };\n\
                kernel void gemm(device const float* a [[buffer(0)]],\n\
                                device const float* b [[buffer(1)]],\n\
                                device float* c [[buffer(2)]],\n\
                                constant GemmDims& dims [[buffer(3)]],\n\
                                uint2 gid [[thread_position_in_grid]]) {\n\
                    if (gid.x >= dims.n || gid.y >= dims.m) { return; }\n\
                    float acc = 0.0f;\n\
                    for (uint i = 0; i < dims.k; ++i) { acc = fma(a[gid.y * dims.k + i], b[i * dims.n + gid.x], acc); }\n\
                    c[gid.y * dims.n + gid.x] = acc;\n\
                }\n";

            const RELU_KERNEL: &str = "#include <metal_stdlib>\n\
                using namespace metal;\n\
                kernel void relu(device const float* x [[buffer(0)]],\n\
                                device float* y [[buffer(1)]],\n\
                                constant uint& n [[buffer(2)]],\n\
                                uint i [[thread_position_in_grid]]) {\n\
                    if (i >= n) { return; }\n\
                    y[i] = max(x[i], 0.0f);\n\
                }\n";

            const TOPK_KERNEL: &str = "#include <metal_stdlib>\n\
                using namespace metal;\n\
                struct TopkParams { uint n; uint k; };\n\
                kernel void topk(device const float* scores [[buffer(0)]],\n\
                                device uint* out_index [[buffer(1)]],\n\
                                device float* out_value [[buffer(2)]],\n\
                                constant TopkParams& params [[buffer(3)]],\n\
                                uint gid [[thread_position_in_grid]]) {\n\
                    if (gid != 0) { return; }\n\
                    for (uint j = 0; j < params.k; ++j) { out_value[j] = -INFINITY; out_index[j] = 0; }\n\
                    for (uint i = 0; i < params.n; ++i) {\n\
                        float v = scores[i];\n\
                        uint at = i;\n\
                        for (uint j = 0; j < params.k; ++j) {\n\
                            if (v > out_value[j]) {\n\
                                float tv = out_value[j]; uint ti = out_index[j];\n\
                                out_value[j] = v; out_index[j] = at;\n\
                                v = tv; at = ti;\n\
                            }\n\
                        }\n\
                    }\n\
                }\n";

            const DTYPE_TABLE_KERNEL: &str = "#include <metal_stdlib>\n\
                using namespace metal;\n\
                kernel void dtypes(device const half* h [[buffer(1)]],\n\
                                device const float* f [[buffer(0)]],\n\
                                device const int* i [[buffer(3)]],\n\
                                device const uint* u [[buffer(2)]],\n\
                                device const bool* b [[buffer(4)]],\n\
                                device float* out [[buffer(5)]]) {\n\
                    out[0] = f[0] + float(h[0]) + float(u[0]) + float(i[0]) + (b[0] ? 1.0f : 0.0f);\n\
                }\n";

            fn msl<'a>(source: &'a str, entry_point: &'a str) -> PipelineInput<'a> {
                PipelineInput::Msl {
                    source,
                    entry_point,
                }
            }

            fn msl_safe<'a>(source: &'a str, entry_point: &'a str) -> PipelineInput<'a> {
                PipelineInput::MslWithOptions {
                    source,
                    entry_point,
                    math_mode: MslMathMode::Safe,
                }
            }

            fn reflect(
                device: &MetalDevice,
                input: &PipelineInput<'_>,
                layout: &BindingLayout,
            ) -> PipelineReflection {
                let function = device.compile_function(input, layout).expect("compile");
                let (_state, reflection) = device.reflect_function(&function).expect("reflect");
                reflection
            }

            fn buffer(slot: u32, bytes: u64, data_type: ReflectedDataType) -> ReflectedBuffer {
                ReflectedBuffer {
                    slot,
                    size: ReflectedSize::Bytes(bytes),
                    data_type,
                }
            }

            /// GEMM のカーネルの引数 4 つが slot・バイト長・型どおりに読める（ART-89 のポインタ）。
            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn reflect_function_reads_the_gemm_kernel_arguments() {
                let device = new_device();
                let layout = BindingLayout::new(vec![0, 1, 2, 3]);
                let got = reflect(&device, &msl(GEMM_KERNEL, "gemm"), &layout);
                assert_eq!(
                    got,
                    PipelineReflection {
                        argument_count: 4,
                        buffers: vec![
                            buffer(0, 4, ReflectedDataType::Float),
                            buffer(1, 4, ReflectedDataType::Float),
                            buffer(2, 4, ReflectedDataType::Float),
                            buffer(3, 12, ReflectedDataType::Struct),
                        ],
                    }
                );
            }

            /// 要素ごと演算 Relu のカーネルの引数 3 つが slot・バイト長・型どおりに読める（ART-89 のポインタ）。
            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn reflect_function_reads_the_relu_kernel_arguments() {
                let device = new_device();
                let layout = BindingLayout::new(vec![0, 1, 2]);
                let got = reflect(&device, &msl(RELU_KERNEL, "relu"), &layout);
                assert_eq!(
                    got,
                    PipelineReflection {
                        argument_count: 3,
                        buffers: vec![
                            buffer(0, 4, ReflectedDataType::Float),
                            buffer(1, 4, ReflectedDataType::Float),
                            buffer(2, 4, ReflectedDataType::UInt),
                        ],
                    }
                );
            }

            /// 部分 Top-k のカーネルの引数 4 つが slot・バイト長・型どおりに読める（ART-89 のポインタ）。
            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn reflect_function_reads_the_topk_kernel_arguments() {
                let device = new_device();
                let layout = BindingLayout::new(vec![0, 1, 2, 3]);
                let got = reflect(&device, &msl(TOPK_KERNEL, "topk"), &layout);
                assert_eq!(
                    got,
                    PipelineReflection {
                        argument_count: 4,
                        buffers: vec![
                            buffer(0, 4, ReflectedDataType::Float),
                            buffer(1, 4, ReflectedDataType::UInt),
                            buffer(2, 4, ReflectedDataType::Float),
                            buffer(3, 8, ReflectedDataType::Struct),
                        ],
                    }
                );
            }

            /// `Msl` と `Safe` 指定の `MslWithOptions` で読める引数が同じで、どちらもパイプラインになる（ART-89 のポインタ）。
            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn msl_and_safe_options_reflect_the_same_arguments_and_create_pipeline() {
                let device = new_device();
                let cases: [(&str, &str, Vec<u32>); 3] = [
                    (GEMM_KERNEL, "gemm", vec![0, 1, 2, 3]),
                    (RELU_KERNEL, "relu", vec![0, 1, 2]),
                    (TOPK_KERNEL, "topk", vec![0, 1, 2, 3]),
                ];
                for (source, entry, slots) in cases {
                    let layout = BindingLayout::new(slots);
                    let plain = reflect(&device, &msl(source, entry), &layout);
                    let safe = reflect(&device, &msl_safe(source, entry), &layout);
                    assert_eq!(plain, safe, "entry={entry}");
                    assert!(
                        device.create_pipeline(msl(source, entry), &layout).is_ok(),
                        "entry={entry}"
                    );
                    assert!(
                        device
                            .create_pipeline(msl_safe(source, entry), &layout)
                            .is_ok(),
                        "entry={entry}"
                    );
                }
            }

            /// 5 つの型のバッファが slot の昇順に並び、型とバイト長が対応表どおりになる（ART-89 のポインタ）。
            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn reflect_function_maps_five_dtypes_by_slot_order() {
                let device = new_device();
                let layout = BindingLayout::new(vec![0, 1, 2, 3, 4, 5]);
                let got = reflect(&device, &msl(DTYPE_TABLE_KERNEL, "dtypes"), &layout);
                assert_eq!(got.argument_count, 6);
                assert_eq!(got.buffers.len(), 6);
                let slots: Vec<u32> = got.buffers.iter().map(|b| b.slot).collect();
                assert_eq!(slots, vec![0, 1, 2, 3, 4, 5]);
                let dtypes = [DType::F32, DType::F16, DType::U32, DType::I32, DType::Bool];
                for (b, d) in got.buffers.iter().zip(dtypes) {
                    assert_eq!(Some(b.data_type), expected_data_type(d), "slot={}", b.slot);
                    assert_eq!(
                        b.size,
                        ReflectedSize::Bytes(d.byte_size()),
                        "slot={}",
                        b.slot
                    );
                }
                assert_eq!(got.buffers[5], buffer(5, 4, ReflectedDataType::Float));
            }
        }

        /// 成果物の検査の口が device から届き、metallib の経路が入力の誤りを表に触れずに拒むことの確認。
        #[cfg(feature = "artifact-inspection")]
        mod artifact_hooks_reach {
            use super::*;
            use fandhe_silicon_core::execution::DType;
            use std::path::Path;

            fn spec(slot: u32) -> OperandSpec<'static> {
                OperandSpec {
                    slot,
                    dtype: DType::F32,
                    shape: &[4],
                    stride: &[1],
                    alignment: 4,
                }
            }

            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn metallib_route_rejects_bad_inputs_without_touching_the_table() {
                let device = new_device();
                let hooks = device.artifact_hooks().expect("artifact hooks");
                let before = device.lock_pipelines().map.len();
                let absolute = std::env::temp_dir().join("fandhe-silicon-unused.metallib");

                let duplicated = [spec(0), spec(0)];
                assert!(matches!(
                    hooks.create_pipeline_checked_from_metallib(&absolute, "k", &duplicated),
                    Err(ContractError::InvalidArgument(_))
                ));
                assert_eq!(device.lock_pipelines().map.len(), before);

                let declared = [spec(0)];
                assert!(matches!(
                    hooks.create_pipeline_checked_from_metallib(
                        Path::new("relative.metallib"),
                        "k",
                        &declared
                    ),
                    Err(ContractError::InvalidArgument(_))
                ));
                assert_eq!(device.lock_pipelines().map.len(), before);

                let missing = std::env::temp_dir().join(format!(
                    "fandhe-silicon-missing-{}.metallib",
                    std::process::id()
                ));
                assert!(matches!(
                    hooks.create_pipeline_checked_from_metallib(&missing, "k", &declared),
                    Err(ContractError::ArtifactRejected {
                        kind: "metallib",
                        ..
                    })
                ));
                assert_eq!(device.lock_pipelines().map.len(), before);
            }
        }

        /// 一覧が空の状態の確保は、全バイトが 0（MEM-2 のポインタ）。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn a_fresh_allocation_reads_back_all_zero() {
            let device = new_device();
            for len in [1_u64, 4096, 4097, 65_536] {
                let h = device
                    .allocate(len, BufferUsage::empty())
                    .expect("allocate");
                let bytes = device.read_range(&h, 0, len).expect("read");
                assert_eq!(bytes.len() as u64, len);
                assert_eq!(bytes.iter().filter(|b| **b != 0).count(), 0, "len={len}");
            }
            assert_eq!(device.test_pool_reuse_count(), 0);
        }

        /// upload -> free -> allocate を 1,000 回くり返しても、再利用されたバッファから以前の内容が読めない
        /// （MEM-17 のポインタ）。再利用が実際に起きたこと（回数と slot の一致の回数）も確かめる。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn reused_buffers_never_expose_previous_contents_over_1000_rounds() {
            const LEN: u64 = 4097;
            let device = new_device();
            let mut handle = device
                .allocate(LEN, BufferUsage::empty())
                .expect("allocate");
            let mut seen_slot = device.test_slot_of(&handle).expect("slot");
            for round in 0..1000_u32 {
                let fill = u8::try_from(round % 255).unwrap_or(0) + 1;
                device
                    .upload(&handle, 0, &vec![fill; LEN as usize])
                    .expect("upload");
                device.free(handle);
                handle = device
                    .allocate(LEN, BufferUsage::empty())
                    .expect("allocate");
                let bytes = device.read_range(&handle, 0, LEN).expect("read");
                let nonzero = bytes.iter().filter(|b| **b != 0).count();
                assert_eq!(nonzero, 0, "round={round}");
                let slot = device.test_slot_of(&handle).expect("slot");
                assert_eq!(slot, seen_slot, "round={round}: the slot was not reused");
                seen_slot = slot;
            }
            let reused = device.test_pool_reuse_count();
            let matched = device.test_pool_slot_match_count();
            println!("pool_reuse_count={reused} pool_slot_match_count={matched}");
            assert!(reused >= 1, "no reuse happened");
            assert_eq!(reused, 1000);
            assert_eq!(matched, reused);
        }

        /// 解放したハンドルは、同じ大きさの再利用の後も新しいハンドルに当たらない。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn a_freed_handle_does_not_alias_the_reused_buffer() {
            let device = new_device();
            let first = device.allocate(64, BufferUsage::empty()).expect("allocate");
            let stale = BufferHandle::__backend_new(
                first.__backend_generation(),
                first.__backend_id(),
                first.__backend_device_id(),
                64,
                BufferUsage::empty(),
            );
            let first_slot = device.test_slot_of(&first).expect("slot");
            device.free(first);
            assert_eq!(device.test_pending_release_count(), 0);
            let second = device.allocate(64, BufferUsage::empty()).expect("allocate");
            assert_ne!(stale.__backend_id(), second.__backend_id());
            assert_eq!(device.test_pool_reuse_count(), 1);
            assert_eq!(device.test_slot_of(&second), Some(first_slot));
            let r = device.upload(&stale, 0, &[1]);
            assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
            // 解放済みのハンドルでの二重の free は、一覧に何も足さない。
            device.free(stale);
            assert_eq!(device.test_pool_len(), 0);
        }

        /// 一覧が上限を超えて溜まらない。別の大きさは再利用されない。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn the_pool_is_bounded_and_matches_only_the_same_length() {
            let device = new_device();
            let handles: Vec<_> = (0..POOL_MAX_ENTRIES + 4)
                .map(|_| device.allocate(32, BufferUsage::empty()).expect("allocate"))
                .collect();
            for h in handles {
                device.free(h);
            }
            assert_eq!(device.test_pool_len(), POOL_MAX_ENTRIES);
            let other = device.allocate(33, BufferUsage::empty()).expect("allocate");
            assert_eq!(device.test_pool_reuse_count(), 0);
            assert_eq!(device.test_pool_len(), POOL_MAX_ENTRIES);
            device.free(other);
        }

        /// CMD-1 のポインタ。記録先を作る・破棄するだけでは投入の件数が増えず、破棄で開いている件数が戻る。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn creating_and_dropping_encoders_does_not_submit() {
            let device = new_device();
            let a = device.create_encoder();
            let b = device.create_encoder();
            assert_eq!(device.test_open_encoder_count(), 2);
            assert_eq!(device.test_submitted_count(), 0);
            drop(a);
            assert_eq!(device.test_open_encoder_count(), 1);
            drop(b);
            assert_eq!(device.test_open_encoder_count(), 0);
            assert_eq!(device.test_submitted_count(), 0);
        }

        /// CMD-12 のポインタ。別 device の記録先は投入されず、元の device の件数も変わらない。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn foreign_encoder_is_rejected_without_submitting() {
            let first = new_device();
            let second = new_device();
            let foreign = first.create_encoder();
            let r = second.submit(foreign, &[]);
            assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
            assert_eq!(second.test_submitted_count(), 0);
            assert_eq!(first.test_submitted_count(), 0);
            // 記録先は破棄で元の device の表から外れる。
            assert_eq!(first.test_open_encoder_count(), 0);
        }

        /// 試験専用。確定を遅らせている間、溜めたコマンドバッファを持つ。落とすと必ず確定する。
        ///
        /// assert が落ちて巻き戻っても、確定していないコマンドバッファは `Drop` の完了待ちで永久に待つので、
        /// device より先に解放する。device の後に宣言し（先に落ちる）、device を落とす試験では明示で解放してから落とす。
        struct HeldCommits<'a>(&'a MetalDevice);

        impl<'a> HeldCommits<'a> {
            fn new(device: &'a MetalDevice) -> Self {
                device.hold_commits();
                Self(device)
            }

            fn release(&self) {
                self.0.release_held_commits();
            }

            /// 確定し、溜めていたコマンドバッファがすべて終わるまで待つ。submission の表は駆動しない
            /// （コマンドバッファ自身の完了待ちなので、callback は呼ばれない）。
            fn release_and_wait_all(&self) {
                for buffer in self.0.release_held_commits() {
                    buffer.waitUntilCompleted();
                }
            }
        }

        impl Drop for HeldCommits<'_> {
            fn drop(&mut self) {
                self.0.release_held_commits();
            }
        }

        type Seen = Arc<Mutex<Vec<CompletionState>>>;

        /// 登録の時点では未完了（確定していない）なので、callback は登録の中で呼ばれない。それを固定して記録先を返す。
        fn register_pending(device: &MetalDevice, token: SubmitToken) -> Seen {
            let seen: Seen = Arc::new(Mutex::new(Vec::new()));
            let sink = Arc::clone(&seen);
            device.on_complete(
                token,
                Box::new(move |state| sink.lock().expect("lock").push(state)),
            );
            let at_registration = !seen.lock().expect("lock").is_empty();
            println!("called_at_registration={at_registration}");
            assert!(!at_registration, "callback was called at registration");
            seen
        }

        /// 確定前は駆動しても呼ばれず、確定後に `drive` を 10 秒まで繰り返すと `Complete` で 1 回だけ呼ばれる。
        fn assert_driven_once(held: &HeldCommits<'_>, seen: &Seen, mut drive: impl FnMut()) {
            drive();
            assert!(
                seen.lock().expect("lock").is_empty(),
                "called while pending"
            );
            held.release();
            let started = std::time::Instant::now();
            while seen.lock().expect("lock").is_empty() {
                assert!(
                    started.elapsed() < std::time::Duration::from_secs(10),
                    "callback was not called"
                );
                drive();
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            drive();
            assert_eq!(*seen.lock().expect("lock"), vec![CompletionState::Complete]);
        }

        fn submit_empty(device: &MetalDevice) -> SubmitToken {
            device.submit(device.create_encoder(), &[]).expect("submit")
        }

        /// TASK-595。`submit` の中で駆動される経路を決定的に通す。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn drive_path_submit_calls_a_pending_registration() {
            let device = new_device();
            let held = HeldCommits::new(&device);
            let token = submit_empty(&device);
            let seen = register_pending(&device, token);
            assert_driven_once(&held, &seen, || {
                submit_empty(&device);
            });
        }

        /// TASK-595。`submit_many` の中で駆動される経路を決定的に通す。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn drive_path_submit_many_calls_a_pending_registration() {
            let device = new_device();
            let held = HeldCommits::new(&device);
            let token = submit_empty(&device);
            let seen = register_pending(&device, token);
            assert_driven_once(&held, &seen, || {
                device
                    .submit_many(vec![device.create_encoder()], &[], &[])
                    .expect("submit_many");
            });
        }

        /// TASK-595。`allocate` の中で駆動される経路を決定的に通す。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn drive_path_allocate_calls_a_pending_registration() {
            let device = new_device();
            let held = HeldCommits::new(&device);
            let token = submit_empty(&device);
            let seen = register_pending(&device, token);
            assert_driven_once(&held, &seen, || {
                let h = device.allocate(16, BufferUsage::empty()).expect("allocate");
                device.free(h);
            });
        }

        /// TASK-595。`poll` の中で駆動される経路を決定的に通す。未確定の間の `poll` は `Pending`。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn drive_path_poll_calls_a_pending_registration() {
            let device = new_device();
            let held = HeldCommits::new(&device);
            let token = submit_empty(&device);
            assert_eq!(device.poll(token), CompletionState::Pending);
            let seen = register_pending(&device, token);
            assert_driven_once(&held, &seen, || {
                device.poll(token);
            });
        }

        /// TASK-595。`poll_ex` の中で駆動される経路を決定的に通す。未確定の間の `poll_ex` は `Pending`。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn drive_path_poll_ex_calls_a_pending_registration() {
            let device = new_device();
            let held = HeldCommits::new(&device);
            let token = submit_empty(&device);
            assert_eq!(device.poll_ex(token), CompletionStateEx::Pending);
            let seen = register_pending(&device, token);
            assert_driven_once(&held, &seen, || {
                device.poll_ex(token);
            });
        }

        /// TASK-595。`wait_blocking(None)` の中で駆動される経路だけを決定的に通す。
        ///
        /// 未確定のコマンドバッファの完了は待てないので、確定してから待つ。確定までは呼ばれていないことを固定する。
        #[cfg(feature = "native-blocking-wait")]
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn drive_path_wait_blocking_calls_a_pending_registration() {
            let device = new_device();
            let held = HeldCommits::new(&device);
            let token = submit_empty(&device);
            let seen = register_pending(&device, token);
            assert!(seen.lock().expect("lock").is_empty());
            held.release();
            device.wait_blocking(token, None).expect("wait_blocking");
            assert_eq!(*seen.lock().expect("lock"), vec![CompletionState::Complete]);
        }

        /// TASK-595。panic が駆動の呼び出し（この試験では `poll`）から 1 回だけ伝わり、残りの callback は 1 回呼ばれ、
        /// device は使い続けられる。この試験は `poll` からの伝わりだけを確かめる。
        ///
        /// 確定を保留して 2 つを登録し、確定して両方の完了を待ってから 1 回だけ駆動する。panic する callback と
        /// その後ろの callback は、毎回同じ駆動に入る。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn drive_path_panic_propagates_from_drive_once_and_the_rest_are_called() {
            let device = new_device();
            let held = HeldCommits::new(&device);
            let first = submit_empty(&device);
            let second = submit_empty(&device);
            let at_registration = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                device.on_complete(first, Box::new(|_| panic!("callback failed")));
            }))
            .is_err();
            assert!(!at_registration, "the panic came from registration");
            let seen = register_pending(&device, second);
            // 両方のコマンドバッファが終わってから 1 回だけ駆動し、panic する callback と
            // その後ろの callback が同じ駆動に入る経路を必ず通す。
            held.release_and_wait_all();
            let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                device.poll(second);
            }))
            .is_err();
            assert!(
                panicked,
                "the drive did not propagate the panic (the commits were not held, or the callback was not called)"
            );
            assert_eq!(
                *seen.lock().expect("lock"),
                vec![CompletionState::Complete],
                "the callback after the panicking one was not called in the same drive"
            );
            println!("panic_propagated_from=drive");
            // 以後の駆動では panic せず、残りの callback も 1 回のまま。device は使い続けられる。
            for _ in 0..3 {
                assert_eq!(device.poll(second), CompletionState::Complete);
            }
            assert_eq!(*seen.lock().expect("lock"), vec![CompletionState::Complete]);
            assert_eq!(device.poll(first), CompletionState::Complete);
            submit_empty(&device);
        }

        /// TASK-595。未確定の登録が残ったまま device を落とすと、`Drop` の中で残りの callback が 1 回呼ばれる。
        ///
        /// 未確定のまま落とすと `Drop` の完了待ちが戻らないので、確定してから落とす。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn drive_path_dropping_the_device_calls_a_pending_registration_once() {
            let device = new_device();
            let held = HeldCommits::new(&device);
            let token = submit_empty(&device);
            let seen = register_pending(&device, token);
            assert!(seen.lock().expect("lock").is_empty());
            held.release();
            drop(held);
            drop(device);
            assert_eq!(*seen.lock().expect("lock"), vec![CompletionState::Complete]);
        }

        /// CMD-11 のポインタ。空でない `wait` は投入せず `Unsupported` で、記録先は破棄で閉じる。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn non_empty_wait_is_unsupported_without_submitting() {
            let device = new_device();
            let encoder = device.create_encoder();
            let r = device.submit(encoder, &[ImportedSemaphoreHandle::__backend_new(1)]);
            assert!(matches!(r, Err(ContractError::Unsupported { .. })), "{r:?}");
            assert_eq!(device.test_submitted_count(), 0);
            assert_eq!(device.test_open_encoder_count(), 0);
        }

        const KERNEL: &str = "#include <metal_stdlib>\n\
            using namespace metal;\n\
            kernel void k(device float* out [[buffer(0)]], uint tid [[thread_position_in_grid]]) {\n\
                out[tid] = 1.0;\n\
            }\n";

        const SPECIALIZED_KERNEL: &str = "#include <metal_stdlib>\n\
            using namespace metal;\n\
            constant uint N [[function_constant(0)]];\n\
            constant bool B [[function_constant(1)]];\n\
            kernel void k(device uint* out [[buffer(0)]]) {\n\
                out[0] = N;\n\
                if (B) { out[1] = 1; }\n\
            }\n";

        fn msl(entry_point: &str) -> PipelineInput<'_> {
            PipelineInput::Msl {
                source: KERNEL,
                entry_point,
            }
        }

        /// TASK-773 のポインタ。MSL から `Ok(PipelineHandle)` が返り、device の識別子が入る。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn create_pipeline_returns_a_handle_of_this_device() {
            let device = new_device();
            let h = device
                .create_pipeline(msl("k"), &BindingLayout::new(vec![0]))
                .expect("create_pipeline");
            assert_eq!(h.__backend_device_id(), device.id);
        }

        /// 2 回作ると識別子が違う。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn create_pipeline_twice_gives_different_ids() {
            let device = new_device();
            let layout = BindingLayout::new(vec![0]);
            let a = device.create_pipeline(msl("k"), &layout).expect("a");
            let b = device.create_pipeline(msl("k"), &layout).expect("b");
            assert_ne!(a.__backend_id(), b.__backend_id());
            assert_eq!(device.lock_pipelines().map.len(), 2);
        }

        /// `[[function_constant(N)]]` を持つ MSL は、`U32` と `Bool` を与えて `Ok` になる。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn specialized_msl_with_u32_and_bool_constants_is_created() {
            let device = new_device();
            let constants = [
                FunctionConstant {
                    index: 0,
                    value: FunctionConstantValue::U32(u32::MAX),
                },
                FunctionConstant {
                    index: 1,
                    value: FunctionConstantValue::Bool(true),
                },
            ];
            let r = device.create_pipeline(
                PipelineInput::MslSpecialized {
                    source: SPECIALIZED_KERNEL,
                    entry_point: "k",
                    function_constants: &constants,
                },
                &BindingLayout::new(vec![0]),
            );
            assert!(r.is_ok(), "{r:?}");
        }

        /// `is_function_constant_defined` で未設定を扱う function constant を持つ MSL は、
        /// 定数が空の `MslSpecialized` でも `Ok` になる。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn empty_msl_specialized_with_optional_function_constant_is_created() {
            const OPTIONAL_KERNEL: &str = "#include <metal_stdlib>\n\
                using namespace metal;\n\
                constant uint k [[function_constant(0)]];\n\
                constant bool has_k = is_function_constant_defined(k);\n\
                kernel void opt(device uint* out [[buffer(0)]]) {\n\
                    out[0] = has_k ? k : 0;\n\
                }\n";
            let device = new_device();
            let r = device.create_pipeline(
                PipelineInput::MslSpecialized {
                    source: OPTIONAL_KERNEL,
                    entry_point: "opt",
                    function_constants: &[],
                },
                &BindingLayout::new(vec![0]),
            );
            assert!(r.is_ok(), "{r:?}");
        }

        /// 文法の誤った MSL は `ArtifactRejected { kind: "msl" }`。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn invalid_msl_is_artifact_rejected() {
            let device = new_device();
            let r = device.create_pipeline(
                PipelineInput::Msl {
                    source: "kernel void k( {",
                    entry_point: "k",
                },
                &BindingLayout::new(vec![]),
            );
            assert!(
                matches!(r, Err(ContractError::ArtifactRejected { kind: "msl", .. })),
                "{r:?}"
            );
        }

        /// 存在しない入口名は `ArtifactRejected { kind: "msl" }`。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn missing_entry_point_is_artifact_rejected() {
            let device = new_device();
            let r = device.create_pipeline(msl("no_such_entry"), &BindingLayout::new(vec![]));
            assert!(
                matches!(r, Err(ContractError::ArtifactRejected { kind: "msl", .. })),
                "{r:?}"
            );
        }

        /// `function_constant(0)` を `uint` で宣言したソースに、`Bool` を渡すと型違いで拒まれる。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn constant_of_the_wrong_type_is_artifact_rejected() {
            let device = new_device();
            let constants = [FunctionConstant {
                index: 0,
                value: FunctionConstantValue::Bool(true),
            }];
            let r = device.create_pipeline(
                PipelineInput::MslSpecialized {
                    source: "#include <metal_stdlib>\n\
                        using namespace metal;\n\
                        constant uint N [[function_constant(0)]];\n\
                        kernel void k(device uint* out [[buffer(0)]]) { out[0] = N; }\n",
                    entry_point: "k",
                    function_constants: &constants,
                },
                &BindingLayout::new(vec![0]),
            );
            assert!(
                matches!(r, Err(ContractError::ArtifactRejected { kind: "msl", .. })),
                "{r:?}"
            );
        }

        /// `function_constant` を宣言したソースに、空の定数の `MslSpecialized` を渡すと拒まれる。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn specialized_msl_with_no_constants_is_artifact_rejected() {
            let device = new_device();
            let r = device.create_pipeline(
                PipelineInput::MslSpecialized {
                    source: SPECIALIZED_KERNEL,
                    entry_point: "k",
                    function_constants: &[],
                },
                &BindingLayout::new(vec![0]),
            );
            assert!(
                matches!(r, Err(ContractError::ArtifactRejected { .. })),
                "{r:?}"
            );
        }

        /// 必須の定数の一部だけを渡した `MslSpecialized` は、特殊化の前に拒まれる。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn specialized_msl_missing_a_required_constant_is_artifact_rejected() {
            const TWO_REQUIRED: &str = "#include <metal_stdlib>\n\
                using namespace metal;\n\
                constant uint A [[function_constant(0)]];\n\
                constant uint B [[function_constant(1)]];\n\
                kernel void k(device uint* out [[buffer(0)]]) { out[0] = A + B; }\n";
            let device = new_device();
            let constants = [FunctionConstant {
                index: 0,
                value: FunctionConstantValue::U32(1),
            }];
            let r = device.create_pipeline(
                PipelineInput::MslSpecialized {
                    source: TWO_REQUIRED,
                    entry_point: "k",
                    function_constants: &constants,
                },
                &BindingLayout::new(vec![0]),
            );
            match r {
                Err(ContractError::ArtifactRejected {
                    kind: "msl",
                    reason,
                }) => assert_eq!(
                    reason,
                    "required function constant at index 1 was not provided"
                ),
                other => panic!("{other:?}"),
            }
        }

        /// 必須の定数を持つカーネルは、`Msl`（定数の並びが空）でも拒まれる。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn msl_with_a_required_constant_is_artifact_rejected() {
            let device = new_device();
            let r = device.create_pipeline(
                PipelineInput::Msl {
                    source: SPECIALIZED_KERNEL,
                    entry_point: "k",
                },
                &BindingLayout::new(vec![0]),
            );
            assert!(
                matches!(r, Err(ContractError::ArtifactRejected { kind: "msl", .. })),
                "{r:?}"
            );
        }

        /// 8×8×8 の f32 の GEMM（1 スレッドが出力の 1 要素を k の昇順の `fma` で求める）。
        const GEMM_8X8X8: &str = "#include <metal_stdlib>\n\
            using namespace metal;\n\
            kernel void gemm8(device const float* a [[buffer(0)]],\n\
                            device const float* b [[buffer(1)]],\n\
                            device float* c [[buffer(2)]],\n\
                            uint2 gid [[thread_position_in_grid]]) {\n\
                if (gid.x >= 8 || gid.y >= 8) { return; }\n\
                float acc = 0.0f;\n\
                for (uint k = 0; k < 8; ++k) { acc = fma(a[gid.y * 8 + k], b[k * 8 + gid.x], acc); }\n\
                c[gid.y * 8 + gid.x] = acc;\n\
            }\n";

        fn msl_with_options(source: &str, mode: MslMathMode) -> PipelineInput<'_> {
            PipelineInput::MslWithOptions {
                source,
                entry_point: "gemm8",
                math_mode: mode,
            }
        }

        /// TASK-774 のポインタ（COMP-99）。同じ GEMM のソースが `Safe` と `Fast` の `MslWithOptions` で
        /// それぞれ `Ok(PipelineHandle)` になり、別の識別子で表に入る。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn msl_with_options_safe_and_fast_are_created() {
            let device = new_device();
            let layout = BindingLayout::new(vec![0, 1, 2]);
            let safe = device
                .create_pipeline(msl_with_options(GEMM_8X8X8, MslMathMode::Safe), &layout)
                .expect("safe");
            let fast = device
                .create_pipeline(msl_with_options(GEMM_8X8X8, MslMathMode::Fast), &layout)
                .expect("fast");
            assert_eq!(safe.__backend_device_id(), device.id);
            assert_eq!(fast.__backend_device_id(), device.id);
            assert_ne!(safe.__backend_id(), fast.__backend_id());
            assert_eq!(device.lock_pipelines().map.len(), 2);
        }

        /// TASK-774 のポインタ（COMP-20）。function constant・数学モード・関数の精度を同時に指定した
        /// `MslSpecializedWithOptions` が、数学モード 2 値 × 関数の精度 2 値のすべてで `Ok` になる。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn msl_specialized_with_options_is_created_for_every_mode_and_function_choice() {
            let device = new_device();
            let constants = [
                FunctionConstant {
                    index: 0,
                    value: FunctionConstantValue::U32(7),
                },
                FunctionConstant {
                    index: 1,
                    value: FunctionConstantValue::Bool(false),
                },
            ];
            for mode in [MslMathMode::Safe, MslMathMode::Fast] {
                for functions in [MslFloatFunctions::Fast, MslFloatFunctions::Precise] {
                    let r = device.create_pipeline(
                        PipelineInput::MslSpecializedWithOptions {
                            source: SPECIALIZED_KERNEL,
                            entry_point: "k",
                            function_constants: &constants,
                            math_mode: mode,
                            float_functions: functions,
                        },
                        &BindingLayout::new(vec![0]),
                    );
                    assert!(r.is_ok(), "{mode:?} {functions:?}: {r:?}");
                }
            }
            assert_eq!(device.lock_pipelines().map.len(), 4);
        }

        /// オプション付きの入力でも、文法の誤りと存在しない入口名は `ArtifactRejected { kind: "msl" }`。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn invalid_msl_with_options_is_artifact_rejected() {
            let device = new_device();
            for mode in [MslMathMode::Safe, MslMathMode::Fast] {
                let r = device.create_pipeline(
                    msl_with_options("kernel void gemm8( {", mode),
                    &BindingLayout::new(vec![]),
                );
                assert!(
                    matches!(r, Err(ContractError::ArtifactRejected { kind: "msl", .. })),
                    "{r:?}"
                );
                let r = device.create_pipeline(
                    PipelineInput::MslWithOptions {
                        source: GEMM_8X8X8,
                        entry_point: "no_such_entry",
                        math_mode: mode,
                    },
                    &BindingLayout::new(vec![]),
                );
                assert!(
                    matches!(r, Err(ContractError::ArtifactRejected { kind: "msl", .. })),
                    "{r:?}"
                );
            }
            assert!(device.lock_pipelines().map.is_empty());
        }

        /// 必須の定数を持つカーネルは、定数が空の `MslSpecializedWithOptions` でも `MslWithOptions` でも拒まれる。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn option_inputs_without_a_required_constant_are_artifact_rejected() {
            let device = new_device();
            let specialized = device.create_pipeline(
                PipelineInput::MslSpecializedWithOptions {
                    source: SPECIALIZED_KERNEL,
                    entry_point: "k",
                    function_constants: &[],
                    math_mode: MslMathMode::Safe,
                    float_functions: MslFloatFunctions::Precise,
                },
                &BindingLayout::new(vec![0]),
            );
            assert!(
                matches!(
                    specialized,
                    Err(ContractError::ArtifactRejected { kind: "msl", .. })
                ),
                "{specialized:?}"
            );
            let plain = device.create_pipeline(
                PipelineInput::MslWithOptions {
                    source: SPECIALIZED_KERNEL,
                    entry_point: "k",
                    math_mode: MslMathMode::Safe,
                },
                &BindingLayout::new(vec![0]),
            );
            assert!(
                matches!(
                    plain,
                    Err(ContractError::ArtifactRejected { kind: "msl", .. })
                ),
                "{plain:?}"
            );
        }

        /// オプション付きの入力でも、重複した slot・重複した定数・範囲外の定数の番号は、Metal を呼ぶ前に
        /// `InvalidArgument` で拒まれ、表に何も入らない。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn option_inputs_reject_duplicate_slots_and_bad_constants_before_compiling() {
            let device = new_device();
            let r = device.create_pipeline(
                msl_with_options(GEMM_8X8X8, MslMathMode::Safe),
                &BindingLayout::new(vec![0, 1, 0]),
            );
            assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
            let bad = [
                [
                    FunctionConstant {
                        index: 0,
                        value: FunctionConstantValue::U32(1),
                    },
                    FunctionConstant {
                        index: 0,
                        value: FunctionConstantValue::U32(2),
                    },
                ],
                [
                    FunctionConstant {
                        index: 65536,
                        value: FunctionConstantValue::U32(1),
                    },
                    FunctionConstant {
                        index: 1,
                        value: FunctionConstantValue::Bool(true),
                    },
                ],
            ];
            for constants in &bad {
                let r = device.create_pipeline(
                    PipelineInput::MslSpecializedWithOptions {
                        source: SPECIALIZED_KERNEL,
                        entry_point: "k",
                        function_constants: constants,
                        math_mode: MslMathMode::Safe,
                        float_functions: MslFloatFunctions::Precise,
                    },
                    &BindingLayout::new(vec![0]),
                );
                assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
            }
            assert!(device.lock_pipelines().map.is_empty());
        }

        /// 重複した slot は Metal を呼ぶ前に `InvalidArgument` で拒まれ、表に何も入らない。
        /// 検査そのものは device なしでも `precheck_tests` で確かめる。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn duplicate_slots_are_invalid_argument_and_leave_the_table_empty() {
            let device = new_device();
            let r = device.create_pipeline(msl("k"), &BindingLayout::new(vec![0, 1, 0]));
            assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
            assert!(device.lock_pipelines().map.is_empty());
        }

        /// `slots` がカーネルの引数と食い違っても生成は `Ok`。表には `layout` の写しが入る。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn mismatched_slots_still_create_and_the_layout_copy_is_stored() {
            let device = new_device();
            let layout = BindingLayout::new(vec![9]);
            let h = device.create_pipeline(msl("k"), &layout).expect("create");
            let pipelines = device.lock_pipelines();
            let entry = pipelines.map.values().next().expect("one entry");
            assert_eq!(entry.1, layout);
            assert_eq!(pipelines.map.len(), 1);
            drop(pipelines);
            assert_eq!(h.__backend_device_id(), device.id);
        }

        /// dispatch の記録・投入の時の積み・完了を、小さなカーネルで確かめる（TASK-775 のポインタ）。
        ///
        /// 結果は CPU で計算した参照とバイトで比べる。待ちは `poll` の繰り返しで、feature に依らない。
        mod dispatch {
            use super::*;
            use fandhe_silicon_core::execution::{Binding, ConstantBinding, ThreadgroupMemory};

            const DISPATCH_KERNELS: &str = "#include <metal_stdlib>\n\
                        using namespace metal;\n\
                        kernel void copy_u32(device const uint* src [[buffer(0)]],\n\
                                                device uint* dst [[buffer(1)]],\n\
                                                uint gid [[thread_position_in_grid]]) {\n\
                            dst[gid] = src[gid];\n\
                        }\n\
                        kernel void add_const(device const uint* src [[buffer(0)]],\n\
                                                device uint* dst [[buffer(1)]],\n\
                                                constant uint& k [[buffer(2)]],\n\
                                                uint gid [[thread_position_in_grid]]) {\n\
                            dst[gid] = src[gid] + k;\n\
                        }\n\
                        kernel void add_one(device const uint* src [[buffer(0)]],\n\
                                            device uint* dst [[buffer(1)]],\n\
                                            uint gid [[thread_position_in_grid]]) {\n\
                            dst[gid] = src[gid] + 1;\n\
                        }\n\
                        kernel void mul_two(device const uint* src [[buffer(0)]],\n\
                                            device uint* dst [[buffer(1)]],\n\
                                            uint gid [[thread_position_in_grid]]) {\n\
                            dst[gid] = src[gid] * 2;\n\
                        }\n\
                        kernel void group_sum(device const uint* src [[buffer(0)]],\n\
                                                device uint* dst [[buffer(1)]],\n\
                                                threadgroup uint* tg [[threadgroup(0)]],\n\
                                                uint gid [[thread_position_in_grid]],\n\
                                                uint lid [[thread_position_in_threadgroup]],\n\
                                                uint group [[threadgroup_position_in_grid]],\n\
                                                uint size [[threads_per_threadgroup]]) {\n\
                            tg[lid] = src[gid];\n\
                            threadgroup_barrier(mem_flags::mem_threadgroup);\n\
                            if (lid == 0) {\n\
                                uint sum = 0;\n\
                                for (uint i = 0; i < size; i++) { sum += tg[i]; }\n\
                                dst[group] = sum;\n\
                            }\n\
                        }\n\
                        kernel void group_mark(device uint* dst [[buffer(0)]],\n\
                                                uint group [[threadgroup_position_in_grid]]) {\n\
                            dst[group] = group + 1;\n\
                        }\n\
                        kernel void skip_unused(device uint* dst [[buffer(0)]],\n\
                                                device uint* unused [[buffer(1)]],\n\
                                                uint gid [[thread_position_in_grid]]) {\n\
                            dst[gid] = gid + 7;\n\
                        }\n";

            fn pipeline(device: &MetalDevice, entry: &str, slots: Vec<u32>) -> PipelineHandle {
                device
                    .create_pipeline(
                        PipelineInput::Msl {
                            source: DISPATCH_KERNELS,
                            entry_point: entry,
                        },
                        &BindingLayout::new(slots),
                    )
                    .expect("create_pipeline")
            }

            fn to_bytes(values: &[u32]) -> Vec<u8> {
                values.iter().flat_map(|v| v.to_le_bytes()).collect()
            }

            fn buffer_of(device: &MetalDevice, values: &[u32]) -> BufferHandle {
                let h = device
                    .allocate(values.len() as u64 * 4, BufferUsage::empty())
                    .expect("allocate");
                device.upload(&h, 0, &to_bytes(values)).expect("upload");
                h
            }

            fn zeroed(device: &MetalDevice, words: u64) -> BufferHandle {
                device
                    .allocate(words * 4, BufferUsage::empty())
                    .expect("allocate")
            }

            fn read_u32s(device: &MetalDevice, h: &BufferHandle, words: u64) -> Vec<u32> {
                let bytes = device.read_range(h, 0, words * 4).expect("read_range");
                bytes
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|c| u32::from_le_bytes(*c))
                    .collect()
            }

            /// 10 秒まで `poll` を繰り返し、`Complete` になることを確かめる。
            fn wait_done(device: &MetalDevice, token: SubmitToken) {
                let started = std::time::Instant::now();
                loop {
                    match device.poll(token) {
                        CompletionState::Pending => {}
                        state => {
                            assert_eq!(state, CompletionState::Complete);
                            return;
                        }
                    }
                    assert!(
                        started.elapsed() < std::time::Duration::from_secs(10),
                        "submission did not complete"
                    );
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            }

            fn record_count(device: &MetalDevice, id: u64) -> Option<usize> {
                device.open.lock().get(&id).map(Vec::len)
            }

            fn submit_and_wait(device: &MetalDevice, encoder: CommandEncoder) {
                let token = device.submit(encoder, &[]).expect("submit");
                wait_done(device, token);
            }

            /// COMP-4・COMP-26 のポインタ。0 でない `offset` の位置から読み書きする。
            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn dispatch_reads_and_writes_at_a_nonzero_binding_offset() {
                let device = new_device();
                let p = pipeline(&device, "copy_u32", vec![0, 1]);
                let input: Vec<u32> = (100..116).collect();
                let src = buffer_of(&device, &input);
                let dst = zeroed(&device, 16);
                let mut enc = device.create_encoder();
                device
                    .dispatch(
                        &mut enc,
                        &p,
                        &[Binding::new(0, &src, 16), Binding::new(1, &dst, 32)],
                        [8, 1, 1],
                    )
                    .expect("dispatch");
                submit_and_wait(&device, enc);
                let mut expected = vec![0_u32; 16];
                expected[8..16].copy_from_slice(&input[4..12]);
                assert_eq!(read_u32s(&device, &dst, 16), expected);
            }

            /// COMP-5・COMP-27 のポインタ。定数の slot が配置に含まれ、カーネルがその値を読む。
            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn dispatch_passes_a_constant_by_slot() {
                let device = new_device();
                let p = pipeline(&device, "add_const", vec![0, 1, 2]);
                let input: Vec<u32> = (0..8).map(|i| i * 3).collect();
                let src = buffer_of(&device, &input);
                let dst = zeroed(&device, 8);
                let k = 41_u32;
                let k_bytes = k.to_le_bytes();
                let mut enc = device.create_encoder();
                device
                    .dispatch_ex(
                        &mut enc,
                        &p,
                        &[Binding::new(0, &src, 0), Binding::new(1, &dst, 0)],
                        &[ConstantBinding::new(2, &k_bytes)],
                        [8, 1, 1],
                        [1, 1, 1],
                        &[],
                    )
                    .expect("dispatch_ex");
                submit_and_wait(&device, enc);
                let expected: Vec<u32> = input.iter().map(|v| v + k).collect();
                assert_eq!(read_u32s(&device, &dst, 8), expected);
            }

            /// COMP-6・COMP-30 のポインタ。スレッドグループの数と大きさ、共有メモリの `index` と `bytes` で動く。
            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn dispatch_runs_the_given_threadgroups_with_threadgroup_memory() {
                let device = new_device();
                let p = pipeline(&device, "group_sum", vec![0, 1]);
                let input: Vec<u32> = (1..=32).collect();
                let src = buffer_of(&device, &input);
                let dst = zeroed(&device, 4);
                let mut enc = device.create_encoder();
                device
                    .dispatch_ex(
                        &mut enc,
                        &p,
                        &[Binding::new(0, &src, 0), Binding::new(1, &dst, 0)],
                        &[],
                        [4, 1, 1],
                        [8, 1, 1],
                        &[ThreadgroupMemory::new(0, 32)],
                    )
                    .expect("dispatch_ex");
                submit_and_wait(&device, enc);
                let expected: Vec<u32> = input.chunks(8).map(|c| c.iter().sum()).collect();
                assert_eq!(read_u32s(&device, &dst, 4), expected);
            }

            /// COMP-6・COMP-30 のポインタ。スレッド数を `[1, 1, 1]` と明示した dispatch の全スレッドグループが動く。
            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn dispatch_with_one_thread_per_threadgroup_marks_every_group() {
                let device = new_device();
                let p = pipeline(&device, "group_mark", vec![0]);
                let dst = zeroed(&device, 6);
                let mut enc = device.create_encoder();
                device
                    .dispatch_ex(
                        &mut enc,
                        &p,
                        &[Binding::new(0, &dst, 0)],
                        &[],
                        [6, 1, 1],
                        [1, 1, 1],
                        &[],
                    )
                    .expect("dispatch_ex");
                submit_and_wait(&device, enc);
                assert_eq!(read_u32s(&device, &dst, 6), vec![1, 2, 3, 4, 5, 6]);
            }

            /// COMP-36 のポインタ。前の出力を次の入力にした 3 回の dispatch が記録の順に動く。
            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn three_dispatches_in_one_encoder_run_in_recorded_order() {
                let device = new_device();
                let p = pipeline(&device, "add_one", vec![0, 1]);
                let input: Vec<u32> = (10..18).collect();
                let b0 = buffer_of(&device, &input);
                let chain = [
                    b0,
                    zeroed(&device, 8),
                    zeroed(&device, 8),
                    zeroed(&device, 8),
                ];
                let mut enc = device.create_encoder();
                for pair in chain.windows(2) {
                    device
                        .dispatch(
                            &mut enc,
                            &p,
                            &[Binding::new(0, &pair[0], 0), Binding::new(1, &pair[1], 0)],
                            [8, 1, 1],
                        )
                        .expect("dispatch");
                }
                submit_and_wait(&device, enc);
                let expected: Vec<u32> = input.iter().map(|v| v + 3).collect();
                assert_eq!(read_u32s(&device, &chain[3], 8), expected);
            }

            /// COMP-3 のポインタ。同じパイプラインを続けて渡し、別のパイプラインに替えて戻しても結果が参照と一致する。
            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn repeating_and_switching_pipelines_matches_the_reference() {
                let device = new_device();
                let add = pipeline(&device, "add_one", vec![0, 1]);
                let mul = pipeline(&device, "mul_two", vec![0, 1]);
                let input: Vec<u32> = (0..8).collect();
                let b0 = buffer_of(&device, &input);
                let chain = [
                    b0,
                    zeroed(&device, 8),
                    zeroed(&device, 8),
                    zeroed(&device, 8),
                    zeroed(&device, 8),
                ];
                let mut enc = device.create_encoder();
                for (i, p) in [&add, &add, &mul, &add].into_iter().enumerate() {
                    device
                        .dispatch(
                            &mut enc,
                            p,
                            &[
                                Binding::new(0, &chain[i], 0),
                                Binding::new(1, &chain[i + 1], 0),
                            ],
                            [8, 1, 1],
                        )
                        .expect("dispatch");
                }
                submit_and_wait(&device, enc);
                let expected: Vec<u32> = input.iter().map(|v| (v + 2) * 2 + 1).collect();
                assert_eq!(read_u32s(&device, &chain[4], 8), expected);
            }

            /// COMP-34 のポインタ。簡易版の `dispatch` は、既定の値を渡した `dispatch_ex` と同じ結果になる。
            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn simple_dispatch_matches_dispatch_ex_with_the_defaults() {
                let device = new_device();
                let p = pipeline(&device, "add_one", vec![0, 1]);
                let input: Vec<u32> = (5..13).collect();
                let src = buffer_of(&device, &input);
                let simple_out = zeroed(&device, 8);
                let ex_out = zeroed(&device, 8);
                let mut a = device.create_encoder();
                device
                    .dispatch(
                        &mut a,
                        &p,
                        &[Binding::new(0, &src, 0), Binding::new(1, &simple_out, 0)],
                        [8, 1, 1],
                    )
                    .expect("dispatch");
                let mut b = device.create_encoder();
                device
                    .dispatch_ex(
                        &mut b,
                        &p,
                        &[Binding::new(0, &src, 0), Binding::new(1, &ex_out, 0)],
                        &[],
                        [8, 1, 1],
                        [1, 1, 1],
                        &[],
                    )
                    .expect("dispatch_ex");
                submit_and_wait(&device, a);
                submit_and_wait(&device, b);
                let simple = device.read_range(&simple_out, 0, 32).expect("read");
                let ex = device.read_range(&ex_out, 0, 32).expect("read");
                assert_eq!(simple, ex);
                let expected: Vec<u32> = input.iter().map(|v| v + 1).collect();
                assert_eq!(simple, to_bytes(&expected));
            }

            /// COMP-35 のポインタ。前に束縛した slot を省いた記録は拒まれ、記録の数は増えない。
            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn omitting_a_previously_bound_slot_is_rejected_and_not_recorded() {
                let device = new_device();
                let p = pipeline(&device, "copy_u32", vec![0, 1]);
                let input: Vec<u32> = (1..9).collect();
                let src = buffer_of(&device, &input);
                let dst = zeroed(&device, 8);
                let mut enc = device.create_encoder();
                let id = enc.__backend_id();
                device
                    .dispatch(
                        &mut enc,
                        &p,
                        &[Binding::new(0, &src, 0), Binding::new(1, &dst, 0)],
                        [8, 1, 1],
                    )
                    .expect("dispatch");
                assert_eq!(record_count(&device, id), Some(1));
                let r = device.dispatch(&mut enc, &p, &[Binding::new(0, &src, 0)], [8, 1, 1]);
                match r {
                    Err(ContractError::InvalidArgument(message)) => {
                        assert!(message.contains("slot 1"), "{message}");
                    }
                    other => panic!("unexpected: {other:?}"),
                }
                assert_eq!(record_count(&device, id), Some(1));
                submit_and_wait(&device, enc);
                assert_eq!(read_u32s(&device, &dst, 8), input);
            }

            /// COMP-39 のポインタ。投入の前に束縛したバッファを `free` すると、投入は拒まれて何も積まれない。
            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn freeing_a_bound_buffer_before_submit_is_rejected_without_submitting() {
                let device = new_device();
                let p = pipeline(&device, "copy_u32", vec![0, 1]);
                let src = buffer_of(&device, &[1, 2, 3, 4]);
                let dst = zeroed(&device, 4);
                let mut enc = device.create_encoder();
                device
                    .dispatch(
                        &mut enc,
                        &p,
                        &[Binding::new(0, &src, 0), Binding::new(1, &dst, 0)],
                        [4, 1, 1],
                    )
                    .expect("dispatch");
                device.free(src);
                let before = device.test_submitted_count();
                let r = device.submit(enc, &[]);
                match r {
                    Err(ContractError::InvalidArgument(message)) => {
                        assert!(message.contains("slot 0"), "{message}");
                    }
                    other => panic!("unexpected: {other:?}"),
                }
                assert_eq!(device.test_submitted_count(), before);
                assert_eq!(read_u32s(&device, &dst, 4), vec![0, 0, 0, 0]);
            }

            /// COMP-39 のポインタ。`submit_many` は、どれか 1 つの記録先が当たれば全部を積まない。
            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn submit_many_with_one_freed_buffer_submits_nothing() {
                let device = new_device();
                let p = pipeline(&device, "add_one", vec![0, 1]);
                let src_a = buffer_of(&device, &[1, 2, 3, 4]);
                let src_b = buffer_of(&device, &[5, 6, 7, 8]);
                let dst_a = zeroed(&device, 4);
                let dst_b = zeroed(&device, 4);
                let mut a = device.create_encoder();
                let mut b = device.create_encoder();
                device
                    .dispatch(
                        &mut a,
                        &p,
                        &[Binding::new(0, &src_a, 0), Binding::new(1, &dst_a, 0)],
                        [4, 1, 1],
                    )
                    .expect("dispatch a");
                device
                    .dispatch(
                        &mut b,
                        &p,
                        &[Binding::new(0, &src_b, 0), Binding::new(1, &dst_b, 0)],
                        [4, 1, 1],
                    )
                    .expect("dispatch b");
                device.free(src_b);
                let before = device.test_submitted_count();
                let r = device.submit_many(vec![a, b], &[], &[]);
                assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
                assert_eq!(device.test_submitted_count(), before);
                assert_eq!(device.test_open_encoder_count(), 0);
                assert_eq!(read_u32s(&device, &dst_a, 4), vec![0, 0, 0, 0]);
                assert_eq!(read_u32s(&device, &dst_b, 4), vec![0, 0, 0, 0]);
            }

            /// COMP-87 のポインタ。長さを超える `offset` は記録の時点で拒まれ、長さちょうどの `offset` は動く。
            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn an_offset_past_the_length_is_rejected_and_the_exact_length_runs() {
                let device = new_device();
                let p = pipeline(&device, "skip_unused", vec![0, 1]);
                let dst = zeroed(&device, 4);
                let unused = zeroed(&device, 16);
                let mut enc = device.create_encoder();
                let id = enc.__backend_id();
                let r = device.dispatch(
                    &mut enc,
                    &p,
                    &[Binding::new(0, &dst, 0), Binding::new(1, &unused, 68)],
                    [4, 1, 1],
                );
                match r {
                    Err(ContractError::InvalidArgument(message)) => {
                        assert!(message.contains("slot 1"), "{message}");
                    }
                    other => panic!("unexpected: {other:?}"),
                }
                assert_eq!(record_count(&device, id), Some(0));
                device
                    .dispatch(
                        &mut enc,
                        &p,
                        &[Binding::new(0, &dst, 0), Binding::new(1, &unused, 64)],
                        [4, 1, 1],
                    )
                    .expect("dispatch at the exact length");
                submit_and_wait(&device, enc);
                assert_eq!(read_u32s(&device, &dst, 4), vec![7, 8, 9, 10]);
            }

            /// 投入の後・完了の前の `free` は解放の保留に置かれ、完了の後に pool へ回る。
            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn freeing_a_buffer_of_a_held_submission_keeps_it_until_completion() {
                let device = new_device();
                let held = HeldCommits::new(&device);
                let p = pipeline(&device, "copy_u32", vec![0, 1]);
                let src = buffer_of(&device, &[1, 2, 3, 4]);
                let dst = zeroed(&device, 4);
                let mut enc = device.create_encoder();
                device
                    .dispatch(
                        &mut enc,
                        &p,
                        &[Binding::new(0, &src, 0), Binding::new(1, &dst, 0)],
                        [4, 1, 1],
                    )
                    .expect("dispatch");
                let token = device.submit(enc, &[]).expect("submit");
                let held_slot = device.test_slot_of(&dst).expect("slot");
                let pool_before = device.test_pool_len();
                device.free(dst);
                assert_eq!(device.test_pending_release_count(), 1);
                assert_eq!(device.test_pool_len(), pool_before);
                let other = zeroed(&device, 4);
                assert_ne!(device.test_slot_of(&other), Some(held_slot));
                held.release();
                wait_done(&device, token);
                assert_eq!(device.test_pending_release_count(), 0);
                assert_eq!(device.test_pool_len(), pool_before + 1);
            }

            /// 使用中のバッファの `upload`・`download_async`・`read_range` は拒まれ、完了の後は通る。
            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn reads_and_writes_of_an_in_use_buffer_are_rejected_until_completion() {
                let device = new_device();
                let held = HeldCommits::new(&device);
                let p = pipeline(&device, "copy_u32", vec![0, 1]);
                let input = [11_u32, 12, 13, 14];
                let src = buffer_of(&device, &input);
                let dst = zeroed(&device, 4);
                let mut enc = device.create_encoder();
                device
                    .dispatch(
                        &mut enc,
                        &p,
                        &[Binding::new(0, &src, 0), Binding::new(1, &dst, 0)],
                        [4, 1, 1],
                    )
                    .expect("dispatch");
                let token = device.submit(enc, &[]).expect("submit");
                let r = device.upload(&dst, 0, &[0xFF; 16]);
                assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
                type Downloads = Arc<Mutex<Vec<Result<Vec<u8>, ContractError>>>>;
                let seen: Downloads = Arc::new(Mutex::new(Vec::new()));
                let sink = Arc::clone(&seen);
                device.download_async(
                    &dst,
                    0,
                    16,
                    Box::new(move |result| sink.lock().expect("lock").push(result)),
                );
                {
                    let calls = seen.lock().expect("lock");
                    assert_eq!(calls.len(), 1);
                    assert!(
                        matches!(calls[0], Err(ContractError::InvalidArgument(_))),
                        "{:?}",
                        calls[0]
                    );
                }
                let r = device.read_range(&dst, 0, 16);
                assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
                held.release();
                wait_done(&device, token);
                assert_eq!(read_u32s(&device, &dst, 4), input);
                device.upload(&dst, 0, &[0xAB; 16]).expect("upload");
                assert_eq!(
                    device.read_range(&dst, 0, 16).expect("read"),
                    vec![0xAB; 16]
                );
            }

            /// 使用中のバッファの `download_blocking` は拒まれ、完了の後は通る。
            #[cfg(feature = "native-blocking-wait")]
            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn download_blocking_of_an_in_use_buffer_is_rejected_until_completion() {
                let device = new_device();
                let held = HeldCommits::new(&device);
                let p = pipeline(&device, "copy_u32", vec![0, 1]);
                let src = buffer_of(&device, &[21, 22, 23, 24]);
                let dst = zeroed(&device, 4);
                let mut enc = device.create_encoder();
                device
                    .dispatch(
                        &mut enc,
                        &p,
                        &[Binding::new(0, &src, 0), Binding::new(1, &dst, 0)],
                        [4, 1, 1],
                    )
                    .expect("dispatch");
                let token = device.submit(enc, &[]).expect("submit");
                let r = device.download_blocking(&dst, 0, 16);
                assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
                held.release();
                wait_done(&device, token);
                let bytes = device.download_blocking(&dst, 0, 16).expect("download");
                assert_eq!(bytes, to_bytes(&[21, 22, 23, 24]));
            }

            /// `submit_many` の 2 つの記録先は 1 つの印で両方の結果がそろい、投入の数は 1 だけ増える。
            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn submit_many_with_two_encoders_completes_both_with_one_token() {
                let device = new_device();
                let p = pipeline(&device, "add_one", vec![0, 1]);
                let in_a: Vec<u32> = (0..8).collect();
                let in_b: Vec<u32> = (50..58).collect();
                let src_a = buffer_of(&device, &in_a);
                let src_b = buffer_of(&device, &in_b);
                let dst_a = zeroed(&device, 8);
                let dst_b = zeroed(&device, 8);
                let mut a = device.create_encoder();
                let mut b = device.create_encoder();
                device
                    .dispatch(
                        &mut a,
                        &p,
                        &[Binding::new(0, &src_a, 0), Binding::new(1, &dst_a, 0)],
                        [8, 1, 1],
                    )
                    .expect("dispatch a");
                device
                    .dispatch(
                        &mut b,
                        &p,
                        &[Binding::new(0, &src_b, 0), Binding::new(1, &dst_b, 0)],
                        [8, 1, 1],
                    )
                    .expect("dispatch b");
                let before = device.test_submitted_count();
                let token = device
                    .submit_many(vec![a, b], &[], &[])
                    .expect("submit_many");
                wait_done(&device, token);
                assert_eq!(device.test_submitted_count(), before + 1);
                let exp_a: Vec<u32> = in_a.iter().map(|v| v + 1).collect();
                let exp_b: Vec<u32> = in_b.iter().map(|v| v + 1).collect();
                assert_eq!(read_u32s(&device, &dst_a, 8), exp_a);
                assert_eq!(read_u32s(&device, &dst_b, 8), exp_b);
            }

            /// COMP-35・COMP-87 のポインタ。記録の時点の拒否は、どれも記録先の記録を変えない。
            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn record_time_rejections_leave_the_encoder_records_unchanged() {
                let device = new_device();
                let other = new_device();
                let p = pipeline(&device, "copy_u32", vec![0, 1]);
                let foreign_p = pipeline(&other, "copy_u32", vec![0, 1]);
                let src = buffer_of(&device, &[1, 2, 3, 4]);
                let dst = zeroed(&device, 4);
                let foreign_buf = zeroed(&other, 4);
                let ok = [Binding::new(0, &src, 0), Binding::new(1, &dst, 0)];

                let mut enc = device.create_encoder();
                let id = enc.__backend_id();
                device
                    .dispatch(&mut enc, &p, &ok, [4, 1, 1])
                    .expect("dispatch");
                assert_eq!(record_count(&device, id), Some(1));

                let r = device.dispatch(&mut enc, &foreign_p, &ok, [4, 1, 1]);
                assert!(matches!(r, Err(ContractError::DeviceMismatch)), "{r:?}");

                let r = device.dispatch(
                    &mut enc,
                    &p,
                    &[Binding::new(0, &foreign_buf, 0), Binding::new(1, &dst, 0)],
                    [4, 1, 1],
                );
                assert!(matches!(r, Err(ContractError::DeviceMismatch)), "{r:?}");

                let forged = PipelineHandle::__backend_new(p.__backend_id() + 1000, device.id);
                let r = device.dispatch(&mut enc, &forged, &ok, [4, 1, 1]);
                assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");

                let r = device.dispatch(&mut enc, &p, &ok, [4, 0, 1]);
                assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");

                let device_max_width =
                    u32::try_from(device.device.maxThreadsPerThreadgroup().width)
                        .expect("width fits u32");
                let r = device.dispatch_ex(
                    &mut enc,
                    &p,
                    &ok,
                    &[],
                    [1, 1, 1],
                    [device_max_width, 2, 1],
                    &[],
                );
                assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");

                let mut done = device.create_encoder();
                let done_id = done.__backend_id();
                device
                    .dispatch(&mut done, &p, &ok, [4, 1, 1])
                    .expect("dispatch");
                submit_and_wait(&device, done);
                let mut stale = CommandEncoder::__backend_new(done_id, None);
                let r = device.dispatch(&mut stale, &p, &ok, [4, 1, 1]);
                assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");

                assert_eq!(record_count(&device, id), Some(1));
                submit_and_wait(&device, enc);
                assert_eq!(read_u32s(&device, &dst, 4), vec![1, 2, 3, 4]);
            }

            use fandhe_silicon_core::execution::DType;

            /// `dispatch_checked` の試験が使うカーネル。`DISPATCH_KERNELS` とは別に持つ。
            const CHECKED_KERNELS: &str = "#include <metal_stdlib>\n\
                        using namespace metal;\n\
                        kernel void gemm_f32(device const float* a [[buffer(0)]],\n\
                            device const float* b [[buffer(1)]],\n\
                            device float* c [[buffer(2)]],\n\
                            constant uint* dims [[buffer(3)]],\n\
                            uint2 gid [[thread_position_in_grid]]) {\n\
                            uint m = dims[0];\n\
                            uint n = dims[1];\n\
                            uint k = dims[2];\n\
                            if (gid.y >= m || gid.x >= n) { return; }\n\
                            float acc = 0.0;\n\
                            for (uint i = 0; i < k; i++) { acc += a[gid.y * k + i] * b[i * n + gid.x]; }\n\
                            c[gid.y * n + gid.x] = acc;\n\
                        }\n\
                        kernel void noop() {}\n";

            fn checked_pipeline(
                device: &MetalDevice,
                entry: &str,
                slots: Vec<u32>,
            ) -> PipelineHandle {
                device
                    .create_pipeline(
                        PipelineInput::Msl {
                            source: CHECKED_KERNELS,
                            entry_point: entry,
                        },
                        &BindingLayout::new(slots),
                    )
                    .expect("create_pipeline")
            }

            fn decl<'a>(
                slot: u32,
                dtype: DType,
                shape: &'a [u64],
                alignment: u32,
            ) -> OperandSpec<'a> {
                OperandSpec {
                    slot,
                    dtype,
                    shape,
                    stride: &[],
                    alignment,
                }
            }

            fn rejected_with(r: &Result<(), ContractError>, prefix: &str) -> bool {
                matches!(r, Err(ContractError::InvalidArgument(m)) if m.starts_with(prefix))
            }

            /// 次元を `[4, 1, 1]`・`[1, 1, 1]` に固定して `dispatch_checked` を呼ぶ。
            fn checked_run(
                device: &MetalDevice,
                enc: &mut CommandEncoder,
                p: &PipelineHandle,
                bindings: &[Binding<'_>],
                consts: &[ConstantBinding<'_>],
                declared: &[OperandSpec<'_>],
            ) -> Result<(), ContractError> {
                device.dispatch_checked(
                    enc,
                    p,
                    bindings,
                    consts,
                    declared,
                    [4, 1, 1],
                    [1, 1, 1],
                    &[],
                )
            }

            /// BIND-50 のポインタ。長さ 0 の次元の宣言は、表に無いハンドルを束縛していても C8 で拒む。
            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn dispatch_checked_rejects_a_zero_length_gemm_before_looking_up_handles() {
                let device = new_device();
                let p = checked_pipeline(&device, "gemm_f32", vec![0, 1, 2, 3]);
                let missing = BufferHandle::__backend_new(
                    device.generation(),
                    u64::MAX,
                    device.id,
                    16,
                    BufferUsage::empty(),
                );
                let dims = to_bytes(&[0, 4, 4]);
                let bindings = [
                    Binding::new(0, &missing, 0),
                    Binding::new(1, &missing, 0),
                    Binding::new(2, &missing, 0),
                ];
                let consts = [ConstantBinding::new(3, &dims)];
                let (a, b, c, d) = ([0u64, 4], [4u64, 4], [0u64, 4], [3u64]);
                let declared = [
                    decl(0, DType::F32, &a, 0),
                    decl(1, DType::F32, &b, 0),
                    decl(2, DType::F32, &c, 0),
                    decl(3, DType::U32, &d, 0),
                ];
                let mut enc = device.create_encoder();
                let id = enc.__backend_id();
                let before = device.test_submitted_count();
                let r = device.dispatch_checked(
                    &mut enc,
                    &p,
                    &bindings,
                    &consts,
                    &declared,
                    [1, 1, 1],
                    [1, 1, 1],
                    &[],
                );
                assert!(rejected_with(&r, "C8:"), "{r:?}");
                assert_eq!(record_count(&device, id), Some(0));
                assert_eq!(device.test_submitted_count(), before);
            }

            /// BIND-52 のポインタ。バッファの実際の長さを超える宣言は C4 で拒む。
            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn dispatch_checked_rejects_a_declaration_larger_than_the_buffer() {
                let device = new_device();
                let p = pipeline(&device, "group_mark", vec![0]);
                let buf = zeroed(&device, 4);
                let shape = [100u64];
                let declared = [decl(0, DType::F32, &shape, 0)];
                let mut enc = device.create_encoder();
                let id = enc.__backend_id();
                let r = device.dispatch_checked(
                    &mut enc,
                    &p,
                    &[Binding::new(0, &buf, 0)],
                    &[],
                    &declared,
                    [4, 1, 1],
                    [1, 1, 1],
                    &[],
                );
                assert!(rejected_with(&r, "C4:"), "{r:?}");
                assert_eq!(record_count(&device, id), Some(0));
            }

            /// BIND-61 のポインタ。上限を 1 バイト超える宣言は C8、上限ちょうどは通る。
            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn dispatch_checked_rejects_one_byte_over_the_limit_and_runs_at_the_limit() {
                let device = new_device();
                let p = checked_pipeline(&device, "noop", vec![]);
                let limit = device.limits().max_buffer_bytes;
                let over = [limit.checked_add(1).expect("limit plus one")];
                let at = [limit];
                let mut enc = device.create_encoder();
                let id = enc.__backend_id();
                let declared_over = [decl(0, DType::Bool, &over, 0)];
                let r = device.dispatch_checked(
                    &mut enc,
                    &p,
                    &[],
                    &[],
                    &declared_over,
                    [1, 1, 1],
                    [1, 1, 1],
                    &[],
                );
                assert!(rejected_with(&r, "C8:"), "{r:?}");
                assert_eq!(record_count(&device, id), Some(0));
                let declared_at = [decl(0, DType::Bool, &at, 0)];
                let r = device.dispatch_checked(
                    &mut enc,
                    &p,
                    &[],
                    &[],
                    &declared_at,
                    [1, 1, 1],
                    [1, 1, 1],
                    &[],
                );
                assert_eq!(r, Ok(()));
                assert_eq!(record_count(&device, id), Some(1));
            }

            /// 表の項目と合わないハンドルは、通る宣言でも `dispatch_ex` の前に拒む。
            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn dispatch_checked_rejects_handles_that_do_not_match_the_table() {
                let device = new_device();
                let other = new_device();
                let p = pipeline(&device, "group_mark", vec![0]);
                let h = zeroed(&device, 4);
                let foreign = zeroed(&other, 4);
                let copy = |id: u64, generation: u64, size: u64| {
                    BufferHandle::__backend_new(
                        generation,
                        id,
                        h.__backend_device_id(),
                        size,
                        BufferUsage::empty(),
                    )
                };
                let bigger = copy(h.__backend_id(), h.__backend_generation(), 400);
                let smaller = copy(h.__backend_id(), h.__backend_generation(), 4);
                let next_generation = copy(
                    h.__backend_id(),
                    h.__backend_generation().checked_add(1).expect("generation"),
                    h.size(),
                );
                let unknown = copy(u64::MAX, h.__backend_generation(), h.size());
                let shape = [4u64];
                let declared = [decl(0, DType::F32, &shape, 0)];
                let mut enc = device.create_encoder();
                let id = enc.__backend_id();
                for bad in [&bigger, &smaller, &next_generation, &unknown] {
                    let r = device.dispatch_checked(
                        &mut enc,
                        &p,
                        &[Binding::new(0, bad, 0)],
                        &[],
                        &declared,
                        [4, 1, 1],
                        [1, 1, 1],
                        &[],
                    );
                    assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
                    assert_eq!(record_count(&device, id), Some(0));
                }
                let r = device.dispatch_checked(
                    &mut enc,
                    &p,
                    &[Binding::new(0, &foreign, 0)],
                    &[],
                    &declared,
                    [4, 1, 1],
                    [1, 1, 1],
                    &[],
                );
                assert!(matches!(r, Err(ContractError::DeviceMismatch)), "{r:?}");
                assert_eq!(record_count(&device, id), Some(0));
                let r = device.dispatch_checked(
                    &mut enc,
                    &p,
                    &[Binding::new(0, &h, 0)],
                    &[],
                    &declared,
                    [4, 1, 1],
                    [1, 1, 1],
                    &[],
                );
                assert_eq!(r, Ok(()));
                assert_eq!(record_count(&device, id), Some(1));
            }

            /// C5・C6・C7（4 通り）・C8 の入力は、記録の前に拒む。
            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn dispatch_checked_rejects_c5_c6_c7_and_c8_before_recording() {
                let device = new_device();
                let copy = pipeline(&device, "copy_u32", vec![0, 1]);
                let add = pipeline(&device, "add_const", vec![0, 1, 2]);
                let src = buffer_of(&device, &(0..16).collect::<Vec<u32>>());
                let dst = zeroed(&device, 16);
                let shape = [4u64];
                let shape_u32 = [3u64];
                let shape_one = [1u64];
                let shape_max = [u64::MAX];
                let four = [0u8; 4];
                let eight = [0u8; 8];
                let mut enc = device.create_encoder();
                let id = enc.__backend_id();

                // C5: offset が宣言の alignment の倍数でない。
                let declared = [
                    decl(0, DType::F32, &shape, 0),
                    decl(1, DType::F32, &shape, 16),
                ];
                let r = checked_run(
                    &device,
                    &mut enc,
                    &copy,
                    &[Binding::new(0, &src, 0), Binding::new(1, &dst, 4)],
                    &[],
                    &declared,
                );
                assert!(rejected_with(&r, "C5:"), "{r:?}");
                assert_eq!(record_count(&device, id), Some(0));

                // C6: 定数の大きさが宣言と合わない。
                let declared = [
                    decl(0, DType::F32, &shape, 0),
                    decl(1, DType::F32, &shape, 0),
                    decl(2, DType::U32, &shape_u32, 0),
                ];
                let r = checked_run(
                    &device,
                    &mut enc,
                    &add,
                    &[Binding::new(0, &src, 0), Binding::new(1, &dst, 0)],
                    &[ConstantBinding::new(2, &eight)],
                    &declared,
                );
                assert!(rejected_with(&r, "C6:"), "{r:?}");
                assert_eq!(record_count(&device, id), Some(0));

                // C7: 宣言に無い束縛・同じ slot の束縛が 2 件・宣言に無い定数・束縛と定数の重なり。
                let one = [decl(0, DType::F32, &shape, 0)];
                let one_u32 = [decl(0, DType::U32, &shape_one, 0)];
                let r = checked_run(
                    &device,
                    &mut enc,
                    &copy,
                    &[Binding::new(0, &src, 0), Binding::new(5, &dst, 0)],
                    &[],
                    &one,
                );
                assert!(rejected_with(&r, "C7:"), "{r:?}");
                let r = checked_run(
                    &device,
                    &mut enc,
                    &copy,
                    &[Binding::new(0, &src, 0), Binding::new(0, &dst, 0)],
                    &[],
                    &one,
                );
                assert!(rejected_with(&r, "C7:"), "{r:?}");
                let r = checked_run(
                    &device,
                    &mut enc,
                    &copy,
                    &[Binding::new(0, &src, 0)],
                    &[ConstantBinding::new(9, &four)],
                    &one,
                );
                assert!(rejected_with(&r, "C7:"), "{r:?}");
                let r = checked_run(
                    &device,
                    &mut enc,
                    &copy,
                    &[Binding::new(0, &src, 0)],
                    &[ConstantBinding::new(0, &four)],
                    &one_u32,
                );
                assert!(rejected_with(&r, "C7:"), "{r:?}");
                assert_eq!(record_count(&device, id), Some(0));

                // C8: 要素数の積があふれる。
                let declared = [decl(0, DType::F32, &shape_max, 0)];
                let r = checked_run(
                    &device,
                    &mut enc,
                    &copy,
                    &[Binding::new(0, &src, 0)],
                    &[],
                    &declared,
                );
                assert!(rejected_with(&r, "C8:"), "{r:?}");
                assert_eq!(record_count(&device, id), Some(0));
            }

            /// ちょうどの宣言は `dispatch_ex` に渡り、投入した出力が入力と一致する。
            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn dispatch_checked_passes_an_exact_declaration_to_the_dispatch() {
                let device = new_device();
                let p = pipeline(&device, "copy_u32", vec![0, 1]);
                let input: Vec<u32> = (200..216).collect();
                let src = buffer_of(&device, &input);
                let dst = zeroed(&device, 16);
                let shape = [16u64];
                let declared = [
                    decl(0, DType::U32, &shape, 0),
                    decl(1, DType::U32, &shape, 0),
                ];
                let mut enc = device.create_encoder();
                let id = enc.__backend_id();
                let r = device.dispatch_checked(
                    &mut enc,
                    &p,
                    &[Binding::new(0, &src, 0), Binding::new(1, &dst, 0)],
                    &[],
                    &declared,
                    [16, 1, 1],
                    [1, 1, 1],
                    &[],
                );
                assert_eq!(r, Ok(()));
                assert_eq!(record_count(&device, id), Some(1));
                submit_and_wait(&device, enc);
                assert_eq!(read_u32s(&device, &dst, 16), input);
            }

            /// 新しい確保と、解放後に再利用した確保のハンドルは、その大きさの宣言で通る。
            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn dispatch_checked_accepts_fresh_and_reused_allocations() {
                let device = new_device();
                let p = pipeline(&device, "group_mark", vec![0]);
                for len in [4u64, 12, 4100] {
                    let shape = [len];
                    let declared = [decl(0, DType::Bool, &shape, 0)];
                    let fresh = device
                        .allocate(len, BufferUsage::empty())
                        .expect("allocate");
                    {
                        let mut enc = device.create_encoder();
                        let r = device.dispatch_checked(
                            &mut enc,
                            &p,
                            &[Binding::new(0, &fresh, 0)],
                            &[],
                            &declared,
                            [1, 1, 1],
                            [1, 1, 1],
                            &[],
                        );
                        assert_eq!(r, Ok(()), "len {len}");
                    }
                    device.free(fresh);
                    let before = device.test_pool_reuse_count();
                    let reused = device
                        .allocate(len, BufferUsage::empty())
                        .expect("allocate");
                    assert_eq!(device.test_pool_reuse_count(), before + 1, "len {len}");
                    let mut enc = device.create_encoder();
                    let r = device.dispatch_checked(
                        &mut enc,
                        &p,
                        &[Binding::new(0, &reused, 0)],
                        &[],
                        &declared,
                        [1, 1, 1],
                        [1, 1, 1],
                        &[],
                    );
                    assert_eq!(r, Ok(()), "len {len}");
                }
            }

            /// TASK-492 のポインタ。事前にコンパイルした metallib と binary archive から作る経路の device の試験。
            #[cfg(feature = "artifact-inspection")]
            mod metallib {
                use super::*;
                use fandhe_silicon_core::execution::DType;
                use std::path::{Path, PathBuf};
                use std::sync::atomic::{AtomicU64, Ordering};

                static NEXT_SCRATCH: AtomicU64 = AtomicU64::new(0);

                /// 試験ごとに新しく作る一時のディレクトリ。`Drop` で中身ごと消す。
                struct ScratchDir {
                    path: PathBuf,
                }

                impl ScratchDir {
                    fn new(tag: &str) -> Self {
                        let n = NEXT_SCRATCH.fetch_add(1, Ordering::Relaxed);
                        let path = std::env::temp_dir().join(format!(
                            "fandhe-silicon-{}-{}-{}",
                            std::process::id(),
                            tag,
                            n
                        ));
                        std::fs::create_dir(&path).expect("create scratch dir");
                        Self { path }
                    }

                    fn path(&self) -> &Path {
                        &self.path
                    }
                }

                impl Drop for ScratchDir {
                    fn drop(&mut self) {
                        let _ = std::fs::remove_dir_all(&self.path);
                    }
                }

                /// `source` を `.metal` に書いて metallib にし、`.metal` と `.air` を消して metallib のパスを返す。
                fn build_metallib(dir: &ScratchDir, name: &str, source: &str) -> PathBuf {
                    let metal = dir.path().join(format!("{name}.metal"));
                    let air = dir.path().join(format!("{name}.air"));
                    let lib = dir.path().join(format!("{name}.metallib"));
                    std::fs::write(&metal, source).expect("write metal source");
                    let compiled = std::process::Command::new("/usr/bin/xcrun")
                        .args([
                            "-sdk",
                            "macosx",
                            "metal",
                            "-c",
                            "-fmetal-math-mode=safe",
                            "-o",
                        ])
                        .arg(&air)
                        .arg(&metal)
                        .output()
                        .expect("run xcrun metal");
                    assert!(
                        compiled.status.success(),
                        "metal failed: {}",
                        String::from_utf8_lossy(&compiled.stderr)
                    );
                    let linked = std::process::Command::new("/usr/bin/xcrun")
                        .args(["-sdk", "macosx", "metallib", "-o"])
                        .arg(&lib)
                        .arg(&air)
                        .output()
                        .expect("run xcrun metallib");
                    assert!(
                        linked.status.success(),
                        "metallib failed: {}",
                        String::from_utf8_lossy(&linked.stderr)
                    );
                    std::fs::remove_file(&metal).expect("remove metal source");
                    std::fs::remove_file(&air).expect("remove air");
                    assert!(!metal.exists());
                    assert!(!air.exists());
                    lib
                }

                /// 行列積（`gemm_f32`）。各要素は 0 から `k` の昇順に `fma` で積む。
                const GEMM_MSL: &str = "#include <metal_stdlib>\n\
                        using namespace metal;\n\
                        struct GemmDims { uint m; uint k; uint n; };\n\
                        kernel void gemm_f32(device const float* a [[buffer(0)]],\n\
                                            device const float* b [[buffer(1)]],\n\
                                            device float* c [[buffer(2)]],\n\
                                            constant GemmDims& dims [[buffer(3)]],\n\
                                            uint2 gid [[thread_position_in_grid]]) {\n\
                            uint col = gid.x;\n\
                            uint row = gid.y;\n\
                            if (row >= dims.m || col >= dims.n) { return; }\n\
                            float acc = 0.0f;\n\
                            for (uint i = 0; i < dims.k; i++) {\n\
                                acc = fma(a[row * dims.k + i], b[i * dims.n + col], acc);\n\
                            }\n\
                            c[row * dims.n + col] = acc;\n\
                        }\n";

                /// ReLU（`relu_f32`）。`x > 0` ならそのまま、そうでなければ 0。
                const RELU_MSL: &str = "#include <metal_stdlib>\n\
                        using namespace metal;\n\
                        kernel void relu_f32(device const float* x [[buffer(0)]],\n\
                                            device float* y [[buffer(1)]],\n\
                                            constant uint& len [[buffer(2)]],\n\
                                            uint gid [[thread_position_in_grid]]) {\n\
                            if (gid >= len) { return; }\n\
                            y[gid] = x[gid] > 0.0f ? x[gid] : 0.0f;\n\
                        }\n";

                /// 部分 Top-k（`topk_f32`）。スレッド 0 だけが選ぶ。有限でないスコアは外し、
                /// f32 の全順序の降順（同点は添字の昇順）で先頭 `min(k, 有限の件数)` 件と件数を書く。
                const TOPK_MSL: &str = "#include <metal_stdlib>\n\
                        using namespace metal;\n\
                        struct TopkParams { uint len; uint k; };\n\
                        static int order_key(float s) {\n\
                            int v = as_type<int>(s);\n\
                            if (v < 0) { v ^= 0x7fffffff; }\n\
                            return v;\n\
                        }\n\
                        kernel void topk_f32(device const float* scores [[buffer(0)]],\n\
                                            device uint* out_slots [[buffer(1)]],\n\
                                            device float* out_scores [[buffer(2)]],\n\
                                            device uint* out_count [[buffer(3)]],\n\
                                            constant TopkParams& params [[buffer(4)]],\n\
                                            uint gid [[thread_position_in_grid]]) {\n\
                            if (gid != 0) { return; }\n\
                            uint finite = 0;\n\
                            for (uint i = 0; i < params.len; i++) {\n\
                                if (isfinite(scores[i])) { finite++; }\n\
                            }\n\
                            uint want = min(params.k, finite);\n\
                            int prev_key = 0;\n\
                            uint prev_idx = 0;\n\
                            for (uint r = 0; r < want; r++) {\n\
                                bool found = false;\n\
                                int best_key = 0;\n\
                                uint best_idx = 0;\n\
                                for (uint i = 0; i < params.len; i++) {\n\
                                    float s = scores[i];\n\
                                    if (!isfinite(s)) { continue; }\n\
                                    int key = order_key(s);\n\
                                    if (r > 0 && !(key < prev_key || (key == prev_key && i > prev_idx))) { continue; }\n\
                                    if (!found || key > best_key) {\n\
                                        found = true;\n\
                                        best_key = key;\n\
                                        best_idx = i;\n\
                                    }\n\
                                }\n\
                                out_slots[r] = best_idx;\n\
                                out_scores[r] = scores[best_idx];\n\
                                prev_key = best_key;\n\
                                prev_idx = best_idx;\n\
                            }\n\
                            out_count[0] = want;\n\
                        }\n";

                /// binary archive の試験用に、同じ形の入口を 2 つ持つ。
                const ARCHIVE_MSL: &str = "#include <metal_stdlib>\n\
                        using namespace metal;\n\
                        kernel void archived_copy(device const uint* src [[buffer(0)]],\n\
                                                device uint* dst [[buffer(1)]],\n\
                                                uint gid [[thread_position_in_grid]]) {\n\
                            dst[gid] = src[gid];\n\
                        }\n\
                        kernel void unarchived_copy(device const uint* src [[buffer(0)]],\n\
                                                    device uint* dst [[buffer(1)]],\n\
                                                    uint gid [[thread_position_in_grid]]) {\n\
                            dst[gid] = src[gid];\n\
                        }\n";

                fn spec<'a>(
                    slot: u32,
                    dtype: DType,
                    shape: &'a [u64],
                    stride: &'a [i64],
                ) -> OperandSpec<'a> {
                    OperandSpec {
                        slot,
                        dtype,
                        shape,
                        stride,
                        alignment: 4,
                    }
                }

                /// 宣言から作り、表の件数が 1 増えることを確かめる。
                fn builds_and_grows_table(
                    device: &MetalDevice,
                    lib: &Path,
                    entry: &str,
                    declared: &[OperandSpec<'_>],
                ) {
                    let hooks = device.artifact_hooks().expect("artifact hooks");
                    let before = device.lock_pipelines().map.len();
                    hooks
                        .create_pipeline_checked_from_metallib(lib, entry, declared)
                        .expect("create from metallib");
                    assert_eq!(device.lock_pipelines().map.len(), before + 1, "{entry}");
                }

                /// 書き出した archive のパスとバイト列を返す。
                fn archive_of(
                    device: &MetalDevice,
                    dir: &ScratchDir,
                    entry: &str,
                ) -> (PathBuf, Vec<u8>) {
                    let hooks = device.artifact_hooks().expect("artifact hooks");
                    let path = dir.path().join("copy.archive");
                    let bytes = hooks
                        .compile_and_write_binary_archive(
                            ARCHIVE_MSL,
                            entry,
                            MslMathMode::Safe,
                            &path,
                        )
                        .expect("write archive");
                    (path, bytes)
                }

                /// ART-58 のポインタ。GEMM・Top-k・ReLU の metallib から、引数と同じ数の宣言でパイプラインが作られ、表が 1 ずつ増える。
                #[test]
                #[ignore = "touches the Metal device; run only in a dedicated device session"]
                fn gemm_topk_and_relu_build_from_their_metallibs() {
                    let device = new_device();
                    let dir = ScratchDir::new("three");
                    let m4: &[u64] = &[4, 4];
                    let rm4: &[i64] = &[4, 1];
                    let gemm = build_metallib(&dir, "gemm", GEMM_MSL);
                    let declared = [
                        spec(0, DType::F32, m4, rm4),
                        spec(1, DType::F32, m4, rm4),
                        spec(2, DType::F32, m4, rm4),
                        spec(3, DType::U32, &[3], &[1]),
                    ];
                    builds_and_grows_table(&device, &gemm, "gemm_f32", &declared);

                    let topk = build_metallib(&dir, "topk", TOPK_MSL);
                    let declared = [
                        spec(0, DType::F32, &[8], &[1]),
                        spec(1, DType::U32, &[3], &[1]),
                        spec(2, DType::F32, &[3], &[1]),
                        spec(3, DType::U32, &[1], &[1]),
                        spec(4, DType::U32, &[2], &[1]),
                    ];
                    builds_and_grows_table(&device, &topk, "topk_f32", &declared);

                    let relu = build_metallib(&dir, "relu", RELU_MSL);
                    let declared = [
                        spec(0, DType::F32, &[8], &[1]),
                        spec(1, DType::F32, &[8], &[1]),
                        spec(2, DType::U32, &[1], &[1]),
                    ];
                    builds_and_grows_table(&device, &relu, "relu_f32", &declared);
                }

                /// ART-58 のポインタ。`.metal` と `.air` が無い状態でも、metallib だけからパイプラインが作られる。
                #[test]
                #[ignore = "touches the Metal device; run only in a dedicated device session"]
                fn a_metallib_builds_after_its_source_and_air_are_removed() {
                    let device = new_device();
                    let dir = ScratchDir::new("sourceless");
                    let lib = build_metallib(&dir, "relu", RELU_MSL);
                    assert!(!dir.path().join("relu.metal").exists());
                    assert!(!dir.path().join("relu.air").exists());
                    let declared = [
                        spec(0, DType::F32, &[8], &[1]),
                        spec(1, DType::F32, &[8], &[1]),
                        spec(2, DType::U32, &[1], &[1]),
                    ];
                    builds_and_grows_table(&device, &lib, "relu_f32", &declared);
                }

                /// metallib に無い入口の名前は `ArtifactRejected`（`metallib`）で、表は変わらない。
                #[test]
                #[ignore = "touches the Metal device; run only in a dedicated device session"]
                fn an_entry_point_missing_from_the_metallib_is_rejected() {
                    let device = new_device();
                    let dir = ScratchDir::new("missing-entry");
                    let lib = build_metallib(&dir, "relu", RELU_MSL);
                    let hooks = device.artifact_hooks().expect("artifact hooks");
                    let before = device.lock_pipelines().map.len();
                    let declared = [spec(0, DType::F32, &[8], &[1])];
                    assert!(matches!(
                        hooks.create_pipeline_checked_from_metallib(
                            &lib,
                            "no_such_entry",
                            &declared
                        ),
                        Err(ContractError::ArtifactRejected {
                            kind: "metallib",
                            ..
                        })
                    ));
                    assert_eq!(device.lock_pipelines().map.len(), before);
                }

                /// shape と stride の次元数が違う宣言は `C3:` で始まる `InvalidArgument` で、表は変わらない。
                #[test]
                #[ignore = "touches the Metal device; run only in a dedicated device session"]
                fn mismatched_shape_and_stride_dimensions_are_rejected_by_c3() {
                    let device = new_device();
                    let dir = ScratchDir::new("c3");
                    let lib = build_metallib(&dir, "relu", RELU_MSL);
                    let hooks = device.artifact_hooks().expect("artifact hooks");
                    let before = device.lock_pipelines().map.len();
                    let declared = [
                        spec(0, DType::F32, &[4, 4], &[1]),
                        spec(1, DType::F32, &[8], &[1]),
                        spec(2, DType::U32, &[1], &[1]),
                    ];
                    match hooks.create_pipeline_checked_from_metallib(&lib, "relu_f32", &declared) {
                        Err(ContractError::InvalidArgument(m)) => {
                            assert!(m.starts_with("C3:"), "{m}");
                        }
                        other => panic!("unexpected: {:?}", other.map(|_| ())),
                    }
                    assert_eq!(device.lock_pipelines().map.len(), before);
                }

                /// 書き出した binary archive のバイト列は、空でなく、ファイルの中身と同じ。
                #[test]
                #[ignore = "touches the Metal device; run only in a dedicated device session"]
                fn the_written_binary_archive_matches_the_file() {
                    let device = new_device();
                    let dir = ScratchDir::new("archive-bytes");
                    let (path, bytes) = archive_of(&device, &dir, "archived_copy");
                    assert!(!bytes.is_empty());
                    assert_eq!(std::fs::read(&path).expect("read archive"), bytes);
                }

                /// archive に入れた入口は、`fail_on_archive_miss` が `true` でもパイプラインが作られ、表が 1 増える。
                #[test]
                #[ignore = "touches the Metal device; run only in a dedicated device session"]
                fn a_pipeline_builds_from_the_archive_with_fail_on_miss() {
                    let device = new_device();
                    let dir = ScratchDir::new("archive-hit");
                    let (path, _) = archive_of(&device, &dir, "archived_copy");
                    let hooks = device.artifact_hooks().expect("artifact hooks");
                    let before = device.lock_pipelines().map.len();
                    hooks
                        .create_pipeline_from_archive_path(
                            ARCHIVE_MSL,
                            "archived_copy",
                            MslMathMode::Safe,
                            &path,
                            true,
                        )
                        .expect("create from archive");
                    assert_eq!(device.lock_pipelines().map.len(), before + 1);
                }

                /// archive に無い入口は、`true` なら `ArtifactRejected`（`compute-pipeline`）で表は変わらず、`false` なら作られる。
                #[test]
                #[ignore = "touches the Metal device; run only in a dedicated device session"]
                fn an_entry_missing_from_the_archive_fails_only_with_fail_on_miss() {
                    let device = new_device();
                    let dir = ScratchDir::new("archive-miss");
                    let (path, _) = archive_of(&device, &dir, "archived_copy");
                    let hooks = device.artifact_hooks().expect("artifact hooks");
                    let before = device.lock_pipelines().map.len();
                    assert!(matches!(
                        hooks.create_pipeline_from_archive_path(
                            ARCHIVE_MSL,
                            "unarchived_copy",
                            MslMathMode::Safe,
                            &path,
                            true,
                        ),
                        Err(ContractError::ArtifactRejected {
                            kind: "compute-pipeline",
                            ..
                        })
                    ));
                    assert_eq!(device.lock_pipelines().map.len(), before);
                    hooks
                        .create_pipeline_from_archive_path(
                            ARCHIVE_MSL,
                            "unarchived_copy",
                            MslMathMode::Safe,
                            &path,
                            false,
                        )
                        .expect("create without fail_on_archive_miss");
                    assert_eq!(device.lock_pipelines().map.len(), before + 1);
                }

                /// archive から作ったパイプラインは束縛の無い配置で表に置かれ、束縛 1 つの `dispatch_ex` は拒まれて記録されない。
                #[test]
                #[ignore = "touches the Metal device; run only in a dedicated device session"]
                fn an_archive_pipeline_rejects_a_binding_without_recording() {
                    let device = new_device();
                    let dir = ScratchDir::new("archive-dispatch");
                    let (path, _) = archive_of(&device, &dir, "archived_copy");
                    let hooks = device.artifact_hooks().expect("artifact hooks");
                    let handle = hooks
                        .create_pipeline_from_archive_path(
                            ARCHIVE_MSL,
                            "archived_copy",
                            MslMathMode::Safe,
                            &path,
                            true,
                        )
                        .expect("create from archive");
                    let buf = buffer_of(&device, &[0; 4]);
                    let mut enc = device.create_encoder();
                    let id = enc.__backend_id();
                    let r = device.dispatch_ex(
                        &mut enc,
                        &handle,
                        &[Binding::new(0, &buf, 0)],
                        &[],
                        [1, 1, 1],
                        [1, 1, 1],
                        &[],
                    );
                    match r {
                        Err(ContractError::InvalidArgument(m)) => {
                            assert!(m.contains("is not in the binding layout"), "{m}");
                        }
                        other => panic!("unexpected: {other:?}"),
                    }
                    assert_eq!(record_count(&device, id), Some(0));
                }

                /// metallib から作った GEMM と ReLU を `dispatch_checked` で投入し、CPU の参照と比べる device の試験。
                #[cfg(feature = "test-support")]
                mod parity_gemm_relu {
                    use super::*;
                    use fandhe_silicon_core::parity::case_inputs::{CaseId, CaseInput};
                    use fandhe_silicon_core::parity::special_values::compare_f32_with_special_values;
                    use fandhe_silicon_core::parity::tolerance::T1;

                    fn u32_of(v: usize) -> u32 {
                        u32::try_from(v).expect("fits in u32")
                    }

                    /// ビット列を保ったまま f32 の列を置く。
                    fn f32_buffer(device: &MetalDevice, values: &[f32]) -> BufferHandle {
                        let bits: Vec<u32> = values.iter().map(|v| v.to_bits()).collect();
                        buffer_of(device, &bits)
                    }

                    fn read_f32s(device: &MetalDevice, h: &BufferHandle, len: usize) -> Vec<f32> {
                        read_u32s(device, h, len as u64)
                            .into_iter()
                            .map(f32::from_bits)
                            .collect()
                    }

                    /// 参照と実測を T1 で比べ、外れた要素が 0 でなければケースの名前と最初の位置を添えて落とす。
                    fn assert_matches_reference(name: &str, reference: &[f32], actual: &[f32]) {
                        let cmp = compare_f32_with_special_values(reference, actual, T1)
                            .expect("compare");
                        if !cmp.is_match() {
                            let at = cmp.first_mismatch();
                            let pair = at.map(|i| (reference.get(i), actual.get(i)));
                            panic!("{name}: mismatch at {at:?}: (reference, actual) = {pair:?}");
                        }
                    }

                    /// ART-17 のポインタ。metallib から作った GEMM を G-01〜G-13・E-1 の 14 件で投入し、CPU の参照と外れた要素 0 で一致する。
                    #[test]
                    #[ignore = "touches the Metal device; run only in a dedicated device session"]
                    fn gemm_from_the_metallib_matches_the_cpu_reference_for_14_cases() {
                        let device = new_device();
                        let dir = ScratchDir::new("parity-gemm");
                        let lib = build_metallib(&dir, "gemm", GEMM_MSL);
                        let ids = [
                            CaseId::G01,
                            CaseId::G02,
                            CaseId::G03,
                            CaseId::G04,
                            CaseId::G05,
                            CaseId::G06,
                            CaseId::G07,
                            CaseId::G08,
                            CaseId::G09,
                            CaseId::G10,
                            CaseId::G11,
                            CaseId::G12,
                            CaseId::G13,
                            CaseId::E1,
                        ];
                        for id in ids {
                            let CaseInput::Gemm(g) = id.input().expect("input") else {
                                panic!("{}: not a gemm input", id.name());
                            };
                            let output = id.reference().expect("reference");
                            let reference = output.as_f32().expect("f32 reference");
                            let (m, k, n) = (g.m(), g.k(), g.n());
                            let (sa, sb, sc) = (
                                [m as u64, k as u64],
                                [k as u64, n as u64],
                                [m as u64, n as u64],
                            );
                            let (ra, rb, rc) = ([k as i64, 1], [n as i64, 1], [n as i64, 1]);
                            let declared = [
                                spec(0, DType::F32, &sa, &ra),
                                spec(1, DType::F32, &sb, &rb),
                                spec(2, DType::F32, &sc, &rc),
                                spec(3, DType::U32, &[3], &[1]),
                            ];
                            let hooks = device.artifact_hooks().expect("artifact hooks");
                            let p = hooks
                                .create_pipeline_checked_from_metallib(&lib, "gemm_f32", &declared)
                                .expect("create from metallib");
                            let a = f32_buffer(&device, g.a());
                            let b = f32_buffer(&device, g.b());
                            let c = zeroed(&device, (m * n) as u64);
                            let dims = to_bytes(&[u32_of(m), u32_of(k), u32_of(n)]);
                            let mut enc = device.create_encoder();
                            let enc_id = enc.__backend_id();
                            let r = device.dispatch_checked(
                                &mut enc,
                                &p,
                                &[
                                    Binding::new(0, &a, 0),
                                    Binding::new(1, &b, 0),
                                    Binding::new(2, &c, 0),
                                ],
                                &[ConstantBinding::new(3, &dims)],
                                &declared,
                                [u32_of(n.div_ceil(8)), u32_of(m.div_ceil(8)), 1],
                                [8, 8, 1],
                                &[],
                            );
                            assert_eq!(r, Ok(()), "{}", id.name());
                            assert_eq!(record_count(&device, enc_id), Some(1), "{}", id.name());
                            submit_and_wait(&device, enc);
                            let actual = read_f32s(&device, &c, m * n);
                            assert_matches_reference(id.name(), reference, &actual);
                        }
                    }

                    /// ART-17 のポインタ。metallib から作った ReLU を E-2 で投入し、CPU の参照と外れた要素 0 で一致する。
                    #[test]
                    #[ignore = "touches the Metal device; run only in a dedicated device session"]
                    fn relu_from_the_metallib_matches_the_cpu_reference() {
                        let device = new_device();
                        let dir = ScratchDir::new("parity-relu");
                        let lib = build_metallib(&dir, "relu", RELU_MSL);
                        let id = CaseId::E2;
                        let CaseInput::Relu(x) = id.input().expect("input") else {
                            panic!("{}: not a relu input", id.name());
                        };
                        let output = id.reference().expect("reference");
                        let reference = output.as_f32().expect("f32 reference");
                        let len = x.len();
                        let shape = [len as u64];
                        let declared = [
                            spec(0, DType::F32, &shape, &[1]),
                            spec(1, DType::F32, &shape, &[1]),
                            spec(2, DType::U32, &[1], &[1]),
                        ];
                        let hooks = device.artifact_hooks().expect("artifact hooks");
                        let p = hooks
                            .create_pipeline_checked_from_metallib(&lib, "relu_f32", &declared)
                            .expect("create from metallib");
                        let xb = f32_buffer(&device, &x);
                        let yb = zeroed(&device, len as u64);
                        let consts = to_bytes(&[u32_of(len)]);
                        let mut enc = device.create_encoder();
                        let enc_id = enc.__backend_id();
                        let r = device.dispatch_checked(
                            &mut enc,
                            &p,
                            &[Binding::new(0, &xb, 0), Binding::new(1, &yb, 0)],
                            &[ConstantBinding::new(2, &consts)],
                            &declared,
                            [u32_of(len.div_ceil(64)), 1, 1],
                            [64, 1, 1],
                            &[],
                        );
                        assert_eq!(r, Ok(()), "{}", id.name());
                        assert_eq!(record_count(&device, enc_id), Some(1), "{}", id.name());
                        submit_and_wait(&device, enc);
                        let actual = read_f32s(&device, &yb, len);
                        assert_matches_reference(id.name(), reference, &actual);
                    }
                }

                /// metallib から作った部分 Top-k を `dispatch_checked` で投入し、CPU の参照と比べる device の試験。
                #[cfg(feature = "test-support")]
                mod parity_topk {
                    use super::*;
                    use fandhe_silicon_core::parity::case_inputs::{CaseId, CaseInput};
                    use fandhe_silicon_core::parity::topk::{TopKEntry, compare_topk};

                    fn u32_of(v: usize) -> u32 {
                        u32::try_from(v).expect("fits in u32")
                    }

                    /// ビット列を保ったまま f32 の列を置く。
                    fn f32_buffer(device: &MetalDevice, values: &[f32]) -> BufferHandle {
                        let bits: Vec<u32> = values.iter().map(|v| v.to_bits()).collect();
                        buffer_of(device, &bits)
                    }

                    /// ART-17 のポインタ。metallib から作った部分 Top-k を E-3・E-4 で投入し、CPU の参照と外れた件数 0 で一致する。
                    #[test]
                    #[ignore = "touches the Metal device; run only in a dedicated device session"]
                    fn topk_from_the_metallib_matches_the_cpu_reference_for_2_cases() {
                        let device = new_device();
                        let dir = ScratchDir::new("parity-topk");
                        let lib = build_metallib(&dir, "topk", TOPK_MSL);
                        for id in [CaseId::E3, CaseId::E4] {
                            let CaseInput::TopK { scores, k } = id.input().expect("input") else {
                                panic!("{}: not a top-k input", id.name());
                            };
                            let output = id.reference().expect("reference");
                            let reference = output.as_topk().expect("top-k reference");
                            let len = scores.len();
                            let (s_len, s_k) = ([len as u64], [k as u64]);
                            let declared = [
                                spec(0, DType::F32, &s_len, &[1]),
                                spec(1, DType::U32, &s_k, &[1]),
                                spec(2, DType::F32, &s_k, &[1]),
                                spec(3, DType::U32, &[1], &[1]),
                                spec(4, DType::U32, &[2], &[1]),
                            ];
                            let hooks = device.artifact_hooks().expect("artifact hooks");
                            let p = hooks
                                .create_pipeline_checked_from_metallib(&lib, "topk_f32", &declared)
                                .expect("create from metallib");
                            let sb = f32_buffer(&device, &scores);
                            let slots = zeroed(&device, k as u64);
                            let outs = zeroed(&device, k as u64);
                            let count = zeroed(&device, 1);
                            let params = to_bytes(&[u32_of(len), u32_of(k)]);
                            let mut enc = device.create_encoder();
                            let enc_id = enc.__backend_id();
                            let r = device.dispatch_checked(
                                &mut enc,
                                &p,
                                &[
                                    Binding::new(0, &sb, 0),
                                    Binding::new(1, &slots, 0),
                                    Binding::new(2, &outs, 0),
                                    Binding::new(3, &count, 0),
                                ],
                                &[ConstantBinding::new(4, &params)],
                                &declared,
                                [1, 1, 1],
                                [1, 1, 1],
                                &[],
                            );
                            assert_eq!(r, Ok(()), "{}", id.name());
                            assert_eq!(record_count(&device, enc_id), Some(1), "{}", id.name());
                            submit_and_wait(&device, enc);
                            let n = read_u32s(&device, &count, 1)
                                .first()
                                .copied()
                                .and_then(|c| usize::try_from(c).ok())
                                .expect("count");
                            assert_eq!(n, reference.len(), "{}", id.name());
                            let actual: Vec<TopKEntry> = read_u32s(&device, &slots, k as u64)
                                .into_iter()
                                .zip(read_u32s(&device, &outs, k as u64))
                                .take(n)
                                .map(|(slot, bits)| TopKEntry::new(slot, f32::from_bits(bits)))
                                .collect();
                            let cmp = compare_topk(reference, &actual).expect("compare");
                            if !cmp.is_match() {
                                let at = cmp.first_mismatch();
                                panic!(
                                    "{}: mismatch at {at:?}: reference {:?}, actual {:?}",
                                    id.name(),
                                    at.and_then(|i| reference.get(i)),
                                    at.and_then(|i| actual.get(i)),
                                );
                            }
                        }
                    }
                }
            }
        }

        /// DEV-80・TASK-2932 のポインタ。`fault_hooks` の口から模擬のロストを呼ぶ。
        #[cfg(feature = "fault-injection")]
        mod lost {
            use super::*;
            use fandhe_silicon_core::fault::{HookFault, inject};

            fn seen_by(device: &MetalDevice) -> Arc<Mutex<Vec<DeviceLostReason>>> {
                let seen = Arc::new(Mutex::new(Vec::new()));
                let sink = Arc::clone(&seen);
                DeviceLostNotify::set_lost_callback(
                    device,
                    Box::new(move |reason| sink.lock().unwrap().push(reason)),
                );
                seen
            }

            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn the_hooks_are_present_and_only_the_loss_is_overridden() {
                let device = new_device();
                assert!(device.fault_hooks().is_some());
                for fault in [
                    HookFault::ArmGate,
                    HookFault::OpenGate,
                    HookFault::RuntimeError("x".into()),
                    HookFault::ValidationReport("x".into()),
                ] {
                    let r = inject(device.fault_hooks(), fault);
                    assert!(
                        matches!(r, Err(ContractError::UnsupportedExtensionDefault { .. })),
                        "{r:?}"
                    );
                }
            }

            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn a_pending_completion_callback_gets_lost_once_after_the_loss() {
                let device = new_device();
                // 確定を遅らせて、未完了の投入を決定的に作る。
                device.hold_commits();
                let token = device.submit(device.create_encoder(), &[]).expect("submit");
                assert_eq!(device.poll(token), CompletionState::Pending);
                let seen = Arc::new(Mutex::new(Vec::new()));
                let sink = Arc::clone(&seen);
                device.on_complete(
                    token,
                    Box::new(move |state| sink.lock().unwrap().push(state)),
                );

                inject(
                    device.fault_hooks(),
                    HookFault::DeviceLost(DeviceLostReason::Unknown),
                )
                .expect("loss");
                assert_eq!(*seen.lock().unwrap(), [CompletionState::Lost]);
                assert_eq!(device.poll(token), CompletionState::Lost);

                // 確定して完了させ、駆動を重ねても `Complete` は届かず、二重にも呼ばれない。
                for buffer in device.release_held_commits() {
                    buffer.waitUntilCompleted();
                }
                device.drive_callbacks(device.current_generation());
                assert_eq!(*seen.lock().unwrap(), [CompletionState::Lost]);
            }

            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn a_handle_from_before_the_loss_can_still_be_freed_once() {
                let device = new_device();
                let handle = device.allocate(64, BufferUsage::empty()).expect("allocate");
                inject(
                    device.fault_hooks(),
                    HookFault::DeviceLost(DeviceLostReason::Unknown),
                )
                .expect("loss");
                // ロストの後も、`free` 以外はロスト前のハンドルを拒む。
                assert!(device.upload(&handle, 0, &[1]).is_err());
                assert_eq!(device.lock_buffers().map.len(), 1);

                let again = BufferHandle::__backend_new(
                    handle.__backend_generation(),
                    handle.__backend_id(),
                    handle.__backend_device_id(),
                    64,
                    BufferUsage::empty(),
                );
                device.free(handle);
                assert_eq!(device.lock_buffers().map.len(), 0);
                assert_eq!(device.lock_buffers().pool.entries, 1);
                // 外れた後の `free` は何も起きない。
                device.free(again);
                assert_eq!(device.lock_buffers().pool.entries, 1);
            }

            #[test]
            #[ignore = "touches the Metal device; run only in a dedicated device session"]
            fn each_reason_reaches_the_callback_once_and_the_generation_moves_once() {
                for reason in [
                    DeviceLostReason::Destroyed,
                    DeviceLostReason::Unknown,
                    DeviceLostReason::ValidationError("v".into()),
                ] {
                    let device = new_device();
                    let seen = seen_by(&device);
                    let before = device.generation();

                    inject(device.fault_hooks(), HookFault::DeviceLost(reason.clone()))
                        .expect("first loss");
                    let after = device.generation();
                    assert!(after > before, "{before} -> {after}");
                    assert_eq!(*seen.lock().unwrap(), [reason]);

                    inject(
                        device.fault_hooks(),
                        HookFault::DeviceLost(DeviceLostReason::Unknown),
                    )
                    .expect("second loss");
                    assert_eq!(device.generation(), after);
                    assert_eq!(seen.lock().unwrap().len(), 1);
                }
            }
        }

        /// この device の上で `pipeline_cache_key` の版の欄に入るはずの値。
        fn device_key_version(device: &MetalDevice) -> u64 {
            let os_version = NSProcessInfo::processInfo()
                .operatingSystemVersionString()
                .to_string();
            cache_key_version(&os_version, &device.device.name().to_string())
        }

        /// `MslWithOptions`（`Safe`）を同じ入力で 2 回呼ぶと、同じ `Some` のキーになる（ART-37 のポインタ）。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn pipeline_cache_key_of_safe_msl_with_options_is_stable() {
            let device = new_device();
            let input = PipelineInput::MslWithOptions {
                source: KERNEL,
                entry_point: "k",
                math_mode: MslMathMode::Safe,
            };
            let a = device.pipeline_cache_key(&input, true).expect("a");
            let b = device.pipeline_cache_key(&input, true).expect("b");
            assert_eq!(a, b);
            assert_eq!(a.compiler_version, device_key_version(&device));
            assert_eq!(a.driver_version, Some(a.compiler_version));
            assert!(device.lock_pipelines().map.is_empty());
        }

        /// 数学モード・ソース・`bounds_checked` を 1 つだけ変えると、対応する欄だけが変わる（ART-38 のポインタ）。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn pipeline_cache_key_changes_only_the_changed_field() {
            const OTHER: &str = "kernel void k(device float* out [[buffer(0)]]) { out[0] = 2.0; }";
            let device = new_device();
            let input = |source, math_mode| PipelineInput::MslWithOptions {
                source,
                entry_point: "k",
                math_mode,
            };
            let base = device
                .pipeline_cache_key(&input(KERNEL, MslMathMode::Safe), false)
                .expect("base");
            let fast = device
                .pipeline_cache_key(&input(KERNEL, MslMathMode::Fast), false)
                .expect("fast");
            assert_ne!(base.options_hash, fast.options_hash);
            assert_eq!(
                CacheKey {
                    options_hash: base.options_hash,
                    ..fast
                },
                base
            );
            let other = device
                .pipeline_cache_key(&input(OTHER, MslMathMode::Safe), false)
                .expect("other");
            assert_ne!(base.source_hash, other.source_hash);
            assert_eq!(
                CacheKey {
                    source_hash: base.source_hash,
                    ..other
                },
                base
            );
            let checked = device
                .pipeline_cache_key(&input(KERNEL, MslMathMode::Safe), true)
                .expect("checked");
            assert_eq!(
                CacheKey {
                    bounds_checked: false,
                    ..checked
                },
                base
            );
        }

        /// 4 つの MSL の入力のキーが、組み立ての関数とこの device の版の値から決まる値と同じで、
        /// MSL でない入力は `None`（TASK-501 のポインタ）。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn pipeline_cache_key_of_each_msl_input_matches_the_pure_key() {
            let device = new_device();
            let version = device_key_version(&device);
            let constants = [FunctionConstant {
                index: 0,
                value: FunctionConstantValue::U32(4),
            }];
            let inputs = [
                msl("k"),
                PipelineInput::MslSpecialized {
                    source: KERNEL,
                    entry_point: "k",
                    function_constants: &constants,
                },
                PipelineInput::MslWithOptions {
                    source: KERNEL,
                    entry_point: "k",
                    math_mode: MslMathMode::Fast,
                },
                PipelineInput::MslSpecializedWithOptions {
                    source: KERNEL,
                    entry_point: "k",
                    function_constants: &constants,
                    math_mode: MslMathMode::Safe,
                    float_functions: MslFloatFunctions::Precise,
                },
            ];
            for input in &inputs {
                for bounds_checked in [false, true] {
                    assert_eq!(
                        device.pipeline_cache_key(input, bounds_checked),
                        metal_cache_key(input, bounds_checked, version),
                        "{input:?}"
                    );
                }
            }
            let wgsl = PipelineInput::Wgsl {
                source: "",
                entry_point: "k",
            };
            assert!(device.pipeline_cache_key(&wgsl, false).is_none());
            assert!(device.lock_pipelines().map.is_empty());
        }
    }
}

#[cfg(target_os = "macos")]
pub use imp::MetalDeviceProvider;

#[cfg(all(target_os = "macos", feature = "fault-injection"))]
pub use imp::{MetalCommandQueueNoneProvider, MetalDefaultDeviceNoneProvider};

#[cfg(test)]
mod pending_release_tests {
    use super::{BufState, GroupState, PendingReleases, Submissions, group_state_of};

    #[allow(clippy::ptr_arg)] // `Submissions<Vec<_>>` の分類関数は `&Vec` を受ける
    fn classify(group: &Vec<BufState>) -> GroupState {
        group_state_of(group.iter().copied())
    }

    #[test]
    fn an_empty_list_takes_nothing() {
        let mut list: PendingReleases<&str> = PendingReleases::new();
        assert!(list.take_finished(|_| true).is_empty());
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn pushed_items_are_counted() {
        let mut list = PendingReleases::new();
        list.push(1, "a");
        list.push(2, "b");
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn only_finished_items_are_taken_in_pushed_order_and_the_rest_keep_their_order() {
        let mut list = PendingReleases::new();
        for (seq, name) in [(1, "a"), (2, "b"), (3, "c"), (4, "d"), (5, "e")] {
            list.push(seq, name);
        }
        let taken = list.take_finished(|seq| seq % 2 == 1);
        assert_eq!(taken, vec!["a", "c", "e"]);
        assert_eq!(list.len(), 2);
        let rest = list.take_finished(|_| true);
        assert_eq!(rest, vec!["b", "d"]);
    }

    #[test]
    fn nothing_is_taken_when_none_is_finished_and_all_when_all_are() {
        let mut list = PendingReleases::new();
        list.push(1, "a");
        list.push(2, "b");
        assert!(list.take_finished(|_| false).is_empty());
        assert_eq!(list.len(), 2);
        assert_eq!(list.take_finished(|_| true), vec!["a", "b"]);
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn a_second_take_does_not_return_items_taken_before() {
        let mut list = PendingReleases::new();
        list.push(1, "a");
        list.push(2, "b");
        assert_eq!(list.take_finished(|seq| seq == 1), vec!["a"]);
        assert!(list.take_finished(|seq| seq == 1).is_empty());
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn is_finished_is_false_for_running_and_true_for_completed_and_failed_groups() {
        let mut subs: Submissions<Vec<BufState>> = Submissions::new();
        subs.pending.insert(1, vec![BufState::Running]);
        subs.pending.insert(2, vec![BufState::Done]);
        subs.pending.insert(3, vec![BufState::Failed]);
        subs.next_sequence = 4;
        assert!(!subs.is_finished(1, classify));
        assert!(subs.is_finished(2, classify));
        assert!(subs.is_finished(3, classify));
    }

    #[test]
    fn is_finished_is_true_at_or_below_the_watermark_and_for_the_failed_set_without_reclaiming() {
        let mut subs: Submissions<Vec<BufState>> = Submissions::new();
        subs.watermark = 2;
        subs.failed.insert(3);
        subs.next_sequence = 5;
        assert!(subs.is_finished(1, classify));
        assert!(subs.is_finished(2, classify));
        assert!(subs.is_finished(3, classify));
        assert_eq!(subs.watermark, 2);
        assert!(subs.pending.is_empty());
    }
}

#[cfg(test)]
mod precheck_tests {
    //! device に触らない検査の固定（どの OS でも流れる）。
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::AtomicU8;

    const DEVICE_ID: u64 = 7;
    const GENERATION: u64 = 3;
    /// 登録の表の試験で使う、今の世代と登録の世代（同じ値）。
    const GEN: u64 = 0;

    fn semaphore() -> ImportedSemaphoreHandle {
        ImportedSemaphoreHandle::__backend_new(1)
    }

    fn token(generation: u64, device_id: u64) -> SubmitToken {
        SubmitToken::__backend_new(generation, 1, device_id)
    }

    /// `host_range` の境界。device に触らない。
    #[test]
    fn host_range_passes_in_range_and_rejects_the_rest() {
        assert_eq!(host_range(0, 16, 16).unwrap(), (0, 16));
        assert_eq!(host_range(8, 8, 16).unwrap(), (8, 8));
        assert_eq!(host_range(16, 0, 16).unwrap(), (16, 0));
        assert_eq!(host_range(0, 0, 16).unwrap(), (0, 0));
        for (offset, len) in [
            (8, 9),
            (0, 17),
            (17, 0),
            (u64::MAX, 1),
            (1, u64::MAX),
            (u64::MAX, u64::MAX),
        ] {
            let r = host_range(offset, len, 16);
            assert!(
                matches!(r, Err(ContractError::InvalidArgument(_))),
                "offset={offset} len={len}: {r:?}"
            );
        }
    }

    #[test]
    fn host_range_rejects_an_overflowing_end_even_for_the_largest_allocation() {
        let r = host_range(u64::MAX, 1, u64::MAX);
        assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
        assert_eq!(host_range(0, 0, 0).unwrap(), (0, 0));
    }

    #[test]
    fn empty_wait_passes_and_non_empty_wait_is_unsupported() {
        assert!(precheck_submit(&[]).is_ok());
        let r = precheck_submit(&[semaphore()]);
        assert!(matches!(r, Err(ContractError::Unsupported { .. })), "{r:?}");
    }

    #[test]
    fn submit_many_rejects_an_empty_list_before_looking_at_wait() {
        let r = precheck_submit_many(0, &[], &[semaphore()], DEVICE_ID, GENERATION);
        assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
    }

    #[test]
    fn submit_many_checks_wait_after_the_list_is_non_empty() {
        let r = precheck_submit_many(1, &[], &[semaphore()], DEVICE_ID, GENERATION);
        assert!(matches!(r, Err(ContractError::Unsupported { .. })), "{r:?}");
    }

    /// 複数の不正が重なるときの順は、`after` の各印、空の列、`wait`（CUDA の同名の検査と同じ）。
    #[test]
    fn submit_many_checks_after_then_empty_then_wait() {
        let foreign = token(GENERATION, DEVICE_ID + 1);
        let stale = token(GENERATION + 1, DEVICE_ID);
        let wait = [semaphore()];
        let r = precheck_submit_many(0, &[foreign], &wait, DEVICE_ID, GENERATION);
        assert_eq!(r, Err(ContractError::DeviceMismatch));
        let r = precheck_submit_many(0, &[stale], &wait, DEVICE_ID, GENERATION);
        assert_eq!(
            r,
            Err(ContractError::DeviceLost {
                generation: GENERATION + 1
            })
        );
        let r = precheck_submit_many(1, &[foreign], &wait, DEVICE_ID, GENERATION);
        assert_eq!(r, Err(ContractError::DeviceMismatch));
    }

    #[test]
    fn after_tokens_are_matched_by_device_then_generation() {
        let own = token(GENERATION, DEVICE_ID);
        assert!(precheck_submit_many(1, &[own], &[], DEVICE_ID, GENERATION).is_ok());

        let foreign = token(GENERATION, DEVICE_ID + 1);
        let r = precheck_submit_many(1, &[own, foreign], &[], DEVICE_ID, GENERATION);
        assert_eq!(r, Err(ContractError::DeviceMismatch));

        let stale = token(GENERATION + 1, DEVICE_ID);
        let r = precheck_submit_many(1, &[stale], &[], DEVICE_ID, GENERATION);
        assert_eq!(
            r,
            Err(ContractError::DeviceLost {
                generation: GENERATION + 1
            })
        );

        // device の識別子が違えば、世代も違う印は `DeviceMismatch` が先に出る。
        let both = token(GENERATION + 1, DEVICE_ID + 1);
        assert_eq!(
            check_token_owner(&both, DEVICE_ID, GENERATION),
            Err(ContractError::DeviceMismatch)
        );
    }

    use BufState::{Done, Failed, Running};

    #[allow(clippy::ptr_arg)] // `Submissions<Vec<_>>` の分類関数は `&Vec` を受ける
    fn classify(group: &Vec<BufState>) -> GroupState {
        group_state_of(group.iter().copied())
    }

    fn table(groups: &[Vec<BufState>]) -> Submissions<Vec<BufState>> {
        let mut subs = Submissions::new();
        for group in groups {
            subs.pending.insert(subs.next_sequence, group.clone());
            subs.next_sequence += 1;
        }
        subs
    }

    #[test]
    fn a_failed_group_is_lost_and_moves_to_the_failed_set() {
        let mut subs = table(&[vec![Done, Failed], vec![Done]]);
        assert_eq!(subs.state_of(1, classify), CompletionState::Lost);
        subs.reclaim(classify);
        assert!(subs.failed.contains(&1));
        assert_eq!(subs.watermark, 2);
        assert_eq!(subs.state_of(1, classify), CompletionState::Lost);
        assert_eq!(subs.state_of(2, classify), CompletionState::Complete);
    }

    #[test]
    fn zero_and_unissued_sequences_are_lost() {
        let mut subs = table(&[vec![Done]]);
        subs.reclaim(classify);
        assert_eq!(subs.state_of(0, classify), CompletionState::Lost);
        assert_eq!(subs.state_of(2, classify), CompletionState::Lost);
        let fresh: Submissions<Vec<BufState>> = Submissions::new();
        assert_eq!(fresh.state_of(0, classify), CompletionState::Lost);
    }

    #[test]
    fn poll_ex_covers_pending_complete_runtime_error_and_unknown_as_distinct_values() {
        let mut subs = table(&[vec![Running], vec![Done], vec![Done, Failed]]);
        subs.reclaim(classify);
        let all = [
            subs.state_ex_of(1, classify),
            subs.state_ex_of(2, classify),
            subs.state_ex_of(3, classify),
            subs.state_ex_of(4, classify),
        ];
        assert_eq!(
            all,
            [
                CompletionStateEx::Pending,
                CompletionStateEx::Complete,
                CompletionStateEx::RuntimeError,
                CompletionStateEx::Unknown,
            ]
        );
    }

    #[test]
    fn poll_ex_of_a_reclaimed_group_is_complete_and_of_a_reclaimed_failure_is_runtime_error() {
        let mut subs = table(&[vec![Done], vec![Failed], vec![Done]]);
        subs.reclaim(classify);
        assert!(subs.pending.is_empty());
        assert_eq!(subs.state_ex_of(1, classify), CompletionStateEx::Complete);
        assert_eq!(
            subs.state_ex_of(2, classify),
            CompletionStateEx::RuntimeError
        );
        assert_eq!(subs.state_ex_of(3, classify), CompletionStateEx::Complete);
    }

    #[test]
    fn poll_ex_of_zero_and_unissued_sequences_is_unknown_never_complete() {
        let mut subs = table(&[vec![Done]]);
        subs.reclaim(classify);
        assert_eq!(subs.state_ex_of(0, classify), CompletionStateEx::Unknown);
        assert_eq!(subs.state_ex_of(2, classify), CompletionStateEx::Unknown);
        assert_eq!(
            subs.state_ex_of(u64::MAX, classify),
            CompletionStateEx::Unknown
        );
        let fresh: Submissions<Vec<BufState>> = Submissions::new();
        assert_eq!(fresh.state_ex_of(1, classify), CompletionStateEx::Unknown);
    }

    #[test]
    fn poll_ex_agrees_with_poll_except_for_the_two_extra_values() {
        let mut subs = table(&[vec![Running], vec![Done], vec![Failed]]);
        subs.reclaim(classify);
        for seq in 0..=4u64 {
            let ex = subs.state_ex_of(seq, classify);
            match subs.state_of(seq, classify) {
                CompletionState::Pending => assert_eq!(ex, CompletionStateEx::Pending),
                CompletionState::Complete => assert_eq!(ex, CompletionStateEx::Complete),
                CompletionState::Lost => assert!(
                    matches!(
                        ex,
                        CompletionStateEx::RuntimeError | CompletionStateEx::Unknown
                    ),
                    "{seq}: {ex:?}"
                ),
            }
        }
    }

    #[test]
    fn an_unfinished_head_blocks_reclaiming_later_groups() {
        let mut subs = table(&[vec![Running], vec![Done]]);
        subs.reclaim(classify);
        assert_eq!(subs.watermark, 0);
        assert_eq!(subs.pending.len(), 2);
        assert_eq!(subs.state_of(1, classify), CompletionState::Pending);
        assert_eq!(subs.state_of(2, classify), CompletionState::Complete);
    }

    #[test]
    fn a_reclaimed_group_that_did_not_fail_is_complete() {
        let mut subs = table(&[vec![Done], vec![Done, Done]]);
        subs.reclaim(classify);
        assert!(subs.pending.is_empty());
        assert!(subs.failed.is_empty());
        assert_eq!(subs.state_of(1, classify), CompletionState::Complete);
        assert_eq!(subs.state_of(2, classify), CompletionState::Complete);
    }

    #[test]
    fn group_state_prefers_failure_over_running() {
        assert_eq!(group_state_of([Running, Failed]), GroupState::Failed);
        assert_eq!(group_state_of([Done, Running]), GroupState::Pending);
        assert_eq!(group_state_of([]), GroupState::Completed);
    }

    #[test]
    fn on_complete_only_ever_sees_a_terminal_state() {
        assert_eq!(
            terminal_state(CompletionState::Complete),
            CompletionState::Complete
        );
        assert_eq!(terminal_state(CompletionState::Lost), CompletionState::Lost);
        assert_eq!(
            terminal_state(CompletionState::Pending),
            CompletionState::Lost
        );
    }

    // ---- on_complete の登録の表（device に触らない）----

    /// 偽の列。0 = 実行中、1 = 完了、2 = 失敗。`wait` で完了にできる。
    type Fake = Arc<AtomicU8>;

    fn fake_state(group: &Fake) -> GroupState {
        match group.load(Ordering::SeqCst) {
            0 => GroupState::Pending,
            2 => GroupState::Failed,
            _ => GroupState::Completed,
        }
    }

    type Seen = Arc<Mutex<Vec<(u64, CompletionState)>>>;

    fn recorder(seen: &Seen, tag: u64) -> Callback {
        let seen = Arc::clone(seen);
        Box::new(move |state| seen.lock().expect("lock").push((tag, state)))
    }

    fn fake_table(states: &[u8]) -> (Submissions<Fake>, Vec<Fake>) {
        let mut subs = Submissions::new();
        let mut groups = Vec::new();
        for state in states {
            let group: Fake = Arc::new(AtomicU8::new(*state));
            subs.pending.insert(subs.next_sequence, Arc::clone(&group));
            subs.next_sequence += 1;
            groups.push(group);
        }
        (subs, groups)
    }

    fn call_all(ready: Vec<(Callback, u64, CompletionState)>) {
        let panic = run_callbacks(ready, || 0, |callback, state| callback(state));
        assert!(panic.is_none());
    }

    #[test]
    fn register_calls_back_at_once_only_for_a_terminal_sequence() {
        let seen: Seen = Arc::default();
        let (mut subs, _groups) = fake_table(&[1, 0]);
        // 回収しないので、完了済みの列は表に残ったままでも `Complete` が決まる。
        let ready = subs.register(1, GEN, GEN, recorder(&seen, 1), fake_state);
        assert!(matches!(ready, Some((_, CompletionState::Complete))));
        assert!(
            subs.register(2, GEN, GEN, recorder(&seen, 2), fake_state)
                .is_none()
        );
        // 払い出していない番号は `Lost` でその場。
        let ready = subs.register(9, GEN, GEN, recorder(&seen, 9), fake_state);
        assert!(matches!(ready, Some((_, CompletionState::Lost))));
        assert_eq!(subs.callbacks.len(), 1);
        assert!(seen.lock().expect("lock").is_empty());
    }

    #[test]
    fn take_terminal_returns_only_finished_registrations_exactly_once() {
        let seen: Seen = Arc::default();
        let (mut subs, groups) = fake_table(&[0, 0, 0]);
        for sequence in 1..=3 {
            assert!(
                subs.register(sequence, GEN, GEN, recorder(&seen, sequence), fake_state)
                    .is_none()
            );
        }
        assert!(subs.take_terminal(GEN, fake_state).is_empty());

        // 先頭が実行中のまま、後ろの 2 つが終わる。回収は止まったままでも後ろの登録は外れる。
        groups[2].store(1, Ordering::SeqCst);
        groups[1].store(2, Ordering::SeqCst);
        call_all(subs.take_terminal(GEN, fake_state));
        assert_eq!(subs.watermark, 0);
        assert_eq!(subs.callbacks.len(), 1);
        assert_eq!(
            *seen.lock().expect("lock"),
            vec![(2, CompletionState::Lost), (3, CompletionState::Complete)]
        );

        // 2 回目の駆動では何も呼ばれず、先頭が終わると 1 回だけ呼ばれる。
        call_all(subs.take_terminal(GEN, fake_state));
        groups[0].store(1, Ordering::SeqCst);
        call_all(subs.take_terminal(GEN, fake_state));
        call_all(subs.take_terminal(GEN, fake_state));
        assert_eq!(seen.lock().expect("lock").len(), 3);
        assert_eq!(
            seen.lock().expect("lock")[2],
            (1, CompletionState::Complete)
        );
        assert!(subs.callbacks.is_empty());
    }

    #[test]
    fn a_registration_for_a_reclaimed_sequence_is_resolved_by_the_watermark() {
        let seen: Seen = Arc::default();
        let (mut subs, groups) = fake_table(&[0]);
        assert!(
            subs.register(1, GEN, GEN, recorder(&seen, 1), fake_state)
                .is_none()
        );
        groups[0].store(1, Ordering::SeqCst);
        subs.reclaim(fake_state);
        assert!(subs.pending.is_empty());
        call_all(subs.take_terminal(GEN, fake_state));
        assert_eq!(
            *seen.lock().expect("lock"),
            vec![(1, CompletionState::Complete)]
        );
    }

    #[test]
    fn take_all_after_waiting_returns_the_actual_state_and_never_pending() {
        let seen: Seen = Arc::default();
        let (mut subs, groups) = fake_table(&[0, 0]);
        for sequence in 1..=2 {
            assert!(
                subs.register(sequence, GEN, GEN, recorder(&seen, sequence), fake_state)
                    .is_none()
            );
        }
        groups[1].store(2, Ordering::SeqCst);
        subs.wait_registered(|group| {
            if group.load(Ordering::SeqCst) == 0 {
                group.store(1, Ordering::SeqCst);
            }
        });
        call_all(subs.take_all(GEN, fake_state));
        assert_eq!(
            *seen.lock().expect("lock"),
            vec![(1, CompletionState::Complete), (2, CompletionState::Lost)]
        );
        assert!(subs.callbacks.is_empty());

        // 待たなかった（実行中のままの）列は `Lost` に寄せる。
        let (mut subs, _groups) = fake_table(&[0]);
        assert!(
            subs.register(1, GEN, GEN, recorder(&seen, 7), fake_state)
                .is_none()
        );
        call_all(subs.take_all(GEN, fake_state));
        assert_eq!(seen.lock().expect("lock")[2], (7, CompletionState::Lost));
    }

    #[test]
    fn a_panicking_callback_does_not_stop_the_rest_and_is_reported_once() {
        let seen: Seen = Arc::default();
        let ready: Vec<(Callback, u64, CompletionState)> = vec![
            (recorder(&seen, 1), 0, CompletionState::Complete),
            (
                Box::new(|_| panic!("callback failed")),
                0,
                CompletionState::Complete,
            ),
            (recorder(&seen, 3), 0, CompletionState::Lost),
        ];
        let payload = run_callbacks(ready, || 0, |callback, state| callback(state));
        assert!(payload.is_some());
        assert_eq!(
            *seen.lock().expect("lock"),
            vec![(1, CompletionState::Complete), (3, CompletionState::Lost)]
        );
    }

    /// 駆動の本体 `drive_table` を通す。callback の中から表の錠を取って登録・駆動しても詰まらない。
    #[test]
    fn callbacks_run_outside_the_lock_so_reentry_does_not_deadlock() {
        type Table = Mutex<Submissions<Fake>>;
        fn drive(table: &Table) {
            drive_table(
                table,
                GEN,
                || GEN,
                fake_state,
                |callback: Callback, state| callback(state),
            );
        }

        let (subs, groups) = fake_table(&[0, 0]);
        let table: Arc<Table> = Arc::new(Mutex::new(subs));
        let seen: Seen = Arc::default();
        let inner_seen = Arc::clone(&seen);
        let inner_table = Arc::clone(&table);
        let second_group = Arc::clone(&groups[1]);
        let outer: Callback = Box::new(move |state| {
            inner_seen.lock().expect("lock").push((1, state));
            // 錠が空いている。
            assert!(inner_table.try_lock().is_ok());
            // 再入: 登録と駆動を、この callback の中から行う。
            register_table(
                &inner_table,
                2,
                GEN,
                || GEN,
                recorder(&inner_seen, 22),
                fake_state,
                |callback: Callback, state| callback(state),
            );
            second_group.store(1, Ordering::SeqCst);
            drive(&inner_table);
        });
        lock_table(&table).callbacks.push((1, GEN, outer));
        drive(&table);
        assert!(seen.lock().expect("lock").is_empty());

        groups[0].store(1, Ordering::SeqCst);
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let worker_table = Arc::clone(&table);
        std::thread::spawn(move || {
            drive(&worker_table);
            done_tx.send(()).expect("send");
        });
        done_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("re-entering from a callback must not deadlock");
        assert_eq!(
            *seen.lock().expect("lock"),
            vec![
                (1, CompletionState::Complete),
                (22, CompletionState::Complete)
            ]
        );
        drive(&table);
        assert_eq!(seen.lock().expect("lock").len(), 2);
    }

    #[test]
    fn drive_table_runs_the_rest_then_propagates_the_first_panic() {
        let seen: Seen = Arc::default();
        let (subs, _groups) = fake_table(&[1, 1, 1]);
        let table = Mutex::new(subs);
        {
            let mut subs = lock_table(&table);
            subs.callbacks.push((1, GEN, recorder(&seen, 1)));
            subs.callbacks
                .push((2, GEN, Box::new(|_| panic!("callback failed"))));
            subs.callbacks.push((3, GEN, recorder(&seen, 3)));
        }
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            drive_table(
                &table,
                GEN,
                || GEN,
                fake_state,
                |callback: Callback, state| callback(state),
            );
        }));
        assert!(outcome.is_err());
        assert_eq!(seen.lock().expect("lock").len(), 2);
        // 登録は残らず、次の駆動で二重に呼ばれない。
        drive_table(
            &table,
            GEN,
            || GEN,
            fake_state,
            |callback: Callback, state| callback(state),
        );
        assert_eq!(seen.lock().expect("lock").len(), 2);
        assert!(lock_table(&table).callbacks.is_empty());
    }

    #[test]
    fn register_table_propagates_the_panic_of_an_at_once_callback_and_keeps_the_lock_usable() {
        let seen: Seen = Arc::default();
        // 番号 1 は完了済み（登録の時点で終端）、番号 2 は実行中。
        let (subs, groups) = fake_table(&[1, 0]);
        let table = Mutex::new(subs);
        let call = |callback: Callback, state| callback(state);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            register_table(
                &table,
                1,
                GEN,
                || GEN,
                Box::new(|_| panic!("callback failed")),
                fake_state,
                call,
            );
        }));
        // その場で呼んだ callback の panic は、登録の呼び出しから伝わる。
        assert!(outcome.is_err());
        // 錠は毒化せず放されていて、登録は残らない。
        assert!(table.try_lock().is_ok());
        assert!(lock_table(&table).callbacks.is_empty());
        // そのあとも、登録と駆動が使える。
        register_table(&table, 2, GEN, || GEN, recorder(&seen, 2), fake_state, call);
        assert!(seen.lock().expect("lock").is_empty());
        groups[1].store(1, Ordering::SeqCst);
        drive_table(&table, GEN, || GEN, fake_state, call);
        assert_eq!(
            *seen.lock().expect("lock"),
            vec![(2, CompletionState::Complete)]
        );
    }

    #[test]
    fn finish_table_waits_calls_the_rest_and_swallows_panics() {
        let seen: Seen = Arc::default();
        let (mut subs, groups) = fake_table(&[0, 0]);
        subs.callbacks
            .push((1, GEN, Box::new(|_| panic!("callback failed"))));
        subs.callbacks.push((2, GEN, recorder(&seen, 2)));
        finish_table(
            &mut subs,
            GEN,
            |group| group.store(1, Ordering::SeqCst),
            fake_state,
            |callback: Callback, state| callback(state),
        );
        assert_eq!(
            *seen.lock().expect("lock"),
            vec![(2, CompletionState::Complete)]
        );
        assert!(subs.callbacks.is_empty());
        assert_eq!(groups[0].load(Ordering::SeqCst), 1);
    }

    /// `Drop` が panic する panic の中身（`panic_any` に渡す）。
    struct PanicOnDrop;
    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            panic!("payload drop failed");
        }
    }

    fn drop_bomb() -> Callback {
        Box::new(|_| std::panic::panic_any(PanicOnDrop))
    }

    #[test]
    fn drive_table_calls_the_rest_when_a_discarded_payload_panics_on_drop() {
        let seen: Seen = Arc::default();
        let (subs, _groups) = fake_table(&[1, 1, 1, 1]);
        let table = Mutex::new(subs);
        {
            let mut subs = lock_table(&table);
            subs.callbacks.push((1, GEN, drop_bomb()));
            subs.callbacks.push((2, GEN, drop_bomb()));
            subs.callbacks.push((3, GEN, recorder(&seen, 3)));
            subs.callbacks.push((4, GEN, recorder(&seen, 4)));
        }
        let call = |callback: Callback, state| callback(state);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            drive_table(&table, GEN, || GEN, fake_state, call);
        }));
        // 最初の中身だけが伝わり、2 個目の中身の破棄の panic は止まる。後ろの callback も 1 回ずつ呼ばれる。
        let payload = outcome.expect_err("the first payload is propagated");
        assert!(payload.is::<PanicOnDrop>());
        std::mem::forget(payload);
        assert_eq!(
            *seen.lock().expect("lock"),
            vec![
                (3, CompletionState::Complete),
                (4, CompletionState::Complete)
            ]
        );
        assert!(lock_table(&table).callbacks.is_empty());
    }

    #[test]
    fn finish_table_does_not_unwind_when_a_discarded_payload_panics_on_drop() {
        let seen: Seen = Arc::default();
        let (mut subs, _groups) = fake_table(&[0, 0, 0]);
        subs.callbacks.push((1, GEN, drop_bomb()));
        subs.callbacks.push((2, GEN, drop_bomb()));
        subs.callbacks.push((3, GEN, recorder(&seen, 3)));
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            finish_table(
                &mut subs,
                GEN,
                |group| group.store(1, Ordering::SeqCst),
                fake_state,
                |callback: Callback, state| callback(state),
            );
        }));
        assert!(outcome.is_ok());
        assert_eq!(
            *seen.lock().expect("lock"),
            vec![(3, CompletionState::Complete)]
        );
        assert!(subs.callbacks.is_empty());
    }

    #[test]
    fn a_panicking_classify_does_not_lose_registrations() {
        let seen: Seen = Arc::default();
        let (mut subs, _groups) = fake_table(&[0]);
        assert!(
            subs.register(1, GEN, GEN, recorder(&seen, 1), fake_state)
                .is_none()
        );
        let boom = |_: &Fake| -> GroupState { panic!("classify failed") };
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            subs.take_terminal(GEN, boom)
        }));
        assert!(outcome.is_err());
        assert_eq!(subs.callbacks.len(), 1);
    }

    #[test]
    fn wait_result_does_not_report_a_failure_as_device_lost() {
        assert!(wait_result(CompletionState::Complete).is_ok());
        let lost = wait_result(CompletionState::Lost);
        assert!(
            matches!(lost, Err(ContractError::BackendFailure { .. })),
            "{lost:?}"
        );
        let pending = wait_result(CompletionState::Pending);
        assert!(
            matches!(pending, Err(ContractError::BackendFailure { .. })),
            "{pending:?}"
        );
    }

    fn available(start_seconds: f64, end_seconds: f64) -> GpuTiming {
        GpuTiming::Available {
            start_seconds,
            end_seconds,
        }
    }

    #[test]
    fn gpu_timing_covers_the_four_returns() {
        let mut subs = table(&[vec![Done], vec![Running], vec![Failed], vec![Done]]);
        subs.record_timing(1, 1.0, 2.5);
        assert_eq!(subs.timing_of(1, classify), available(1.0, 2.5));
        assert_eq!(subs.timing_of(2, classify), GpuTiming::Pending);
        assert_eq!(subs.timing_of(3, classify), GpuTiming::Unavailable);
        // 完了していて時刻を残していない投入。
        assert_eq!(subs.timing_of(4, classify), GpuTiming::Reclaimed);
    }

    #[test]
    fn gpu_timing_boundaries_follow_the_watermark_and_the_failed_set() {
        let mut subs = table(&[vec![Done], vec![Failed], vec![Done], vec![Running]]);
        subs.reclaim(classify);
        assert_eq!(subs.watermark, 3);
        assert_eq!(subs.timing_of(0, classify), GpuTiming::Unavailable);
        assert_eq!(subs.timing_of(1, classify), GpuTiming::Reclaimed);
        assert_eq!(subs.timing_of(2, classify), GpuTiming::Unavailable);
        assert_eq!(subs.timing_of(3, classify), GpuTiming::Reclaimed);
        assert_eq!(subs.timing_of(4, classify), GpuTiming::Pending);
        assert_eq!(subs.timing_of(5, classify), GpuTiming::Unavailable);
        assert_eq!(subs.timing_of(u64::MAX, classify), GpuTiming::Unavailable);
    }

    #[test]
    fn gpu_timing_keeps_a_recorded_time_after_reclaiming() {
        let mut subs = table(&[vec![Done]]);
        subs.record_timing(1, 3.0, 4.0);
        subs.reclaim(classify);
        assert_eq!(subs.timing_of(1, classify), available(3.0, 4.0));
    }

    #[test]
    fn the_timing_table_drops_the_smallest_sequence_past_1024_entries() {
        let mut subs: Submissions<Vec<BufState>> = Submissions::new();
        for sequence in 1..=1025u64 {
            subs.pending.insert(sequence, vec![Done]);
            subs.next_sequence = sequence + 1;
            subs.record_timing(sequence, sequence as f64, sequence as f64 + 0.5);
            subs.reclaim(classify);
        }
        assert_eq!(subs.timings.len(), 1024);
        assert_eq!(subs.timing_of(1, classify), GpuTiming::Reclaimed);
        assert_eq!(subs.timing_of(2, classify), available(2.0, 2.5));
        assert_eq!(subs.timing_of(1025, classify), available(1025.0, 1025.5));
    }

    #[test]
    fn gpu_timing_does_not_reclaim_or_change_the_table() {
        let subs = table(&[vec![Done], vec![Done]]);
        for sequence in 0..4 {
            let _ = subs.timing_of(sequence, classify);
        }
        assert_eq!(subs.watermark, 0);
        assert_eq!(subs.pending.len(), 2);
        assert!(subs.failed.is_empty());
        assert!(subs.timings.is_empty());
    }

    #[test]
    fn span_takes_the_earliest_start_and_the_latest_end() {
        assert_eq!(
            span_of([(2.0, 3.0), (1.0, 2.5), (2.5, 4.0)]),
            Some((1.0, 4.0))
        );
        assert_eq!(span_of([(1.0, 1.0)]), Some((1.0, 1.0)));
    }

    #[test]
    fn span_rejects_empty_non_finite_and_reversed_times() {
        assert_eq!(span_of([]), None);
        assert_eq!(span_of([(1.0, f64::NAN)]), None);
        assert_eq!(span_of([(f64::INFINITY, f64::INFINITY)]), None);
        assert_eq!(span_of([(1.0, 2.0), (3.0, 2.0)]), None);
    }

    #[test]
    fn span_rejects_zero_times_even_when_other_pairs_are_valid() {
        assert_eq!(span_of([(0.0, 0.0)]), None);
        assert_eq!(span_of([(0.0, 1.0)]), None);
        assert_eq!(span_of([(1.0, 0.0)]), None);
        assert_eq!(span_of([(1.0, 2.0), (0.0, 0.0)]), None);
        assert_eq!(span_of([(-1.0, 2.0)]), None);
    }

    #[test]
    fn complete_wait_records_only_for_the_unbounded_wait_with_a_valid_span() {
        let mut subs = table(&[]);
        assert!(!subs.complete_wait(1, Some((1.0, 2.0)), false));
        assert!(subs.timings.is_empty());
        assert!(!subs.complete_wait(2, None, true));
        assert!(subs.timings.is_empty());
        assert!(subs.complete_wait(3, Some((1.0, 2.0)), true));
        assert_eq!(subs.timing_of(3, classify), available(1.0, 2.0));
        assert_eq!(subs.timings.len(), 1);
    }

    #[test]
    fn reclaiming_never_writes_the_timing_table() {
        let mut subs = table(&[vec![Done], vec![Done]]);
        subs.reclaim(classify);
        assert!(subs.timings.is_empty());
        assert_eq!(subs.timing_of(1, classify), GpuTiming::Reclaimed);
    }

    /// 呼ぶたびに状態が進む分類。1 回目は `Pending`、2 回目以降は `Completed`（回収と答えの間の完了を模す）。
    fn completes_after_first_look(
        calls: &std::cell::Cell<u32>,
    ) -> impl Fn(&Vec<BufState>) -> GroupState + Copy + '_ {
        move |_| {
            calls.set(calls.get() + 1);
            if calls.get() == 1 {
                GroupState::Pending
            } else {
                GroupState::Completed
            }
        }
    }

    #[test]
    fn poll_state_of_a_stalled_head_is_pending_and_never_complete_while_in_the_table() {
        let mut subs = table(&[vec![Done]]);
        let state =
            subs.reclaim_and_state_of(1, completes_after_first_look(&std::cell::Cell::new(0)));
        assert_eq!(state, CompletionState::Pending);
        assert!(subs.pending.contains_key(&1));
        // 次の呼び出しで完了と分かれば、答えと同時に表から外れる。
        assert_eq!(
            subs.reclaim_and_state_of(1, classify),
            CompletionState::Complete
        );
        assert!(!subs.pending.contains_key(&1));
        assert_eq!(subs.timing_of(1, classify), GpuTiming::Reclaimed);
    }

    #[test]
    fn poll_ex_state_of_a_stalled_head_is_pending_and_never_complete_while_in_the_table() {
        let mut subs = table(&[vec![Done]]);
        let state =
            subs.reclaim_and_state_ex_of(1, completes_after_first_look(&std::cell::Cell::new(0)));
        assert_eq!(state, CompletionStateEx::Pending);
        assert!(subs.pending.contains_key(&1));
        assert_eq!(
            subs.reclaim_and_state_ex_of(1, classify),
            CompletionStateEx::Complete
        );
        assert!(!subs.pending.contains_key(&1));
    }

    #[test]
    fn a_non_pending_answer_for_a_head_always_comes_with_removal_from_the_table() {
        // 1 回目の分類が Pending なら Pending、そうでなければ表から外れている。
        for first in [
            GroupState::Pending,
            GroupState::Completed,
            GroupState::Failed,
        ] {
            let mut subs = table(&[vec![Done]]);
            let calls = std::cell::Cell::new(0_u32);
            let classify_fn = |_: &Vec<BufState>| {
                calls.set(calls.get() + 1);
                if calls.get() == 1 {
                    first
                } else {
                    GroupState::Completed
                }
            };
            let state = subs.reclaim_and_state_of(1, classify_fn);
            assert_eq!(
                state != CompletionState::Pending,
                !subs.pending.contains_key(&1),
                "{first:?} {state:?}"
            );
        }
    }

    #[test]
    fn a_complete_answer_behind_an_unfinished_head_stays_in_the_table() {
        // 先頭が未完了なら、後ろの完了済みの列は `Complete` と答えても表に残る（既存の設計）。
        let mut subs = table(&[vec![Running], vec![Done]]);
        assert_eq!(
            subs.reclaim_and_state_of(2, classify),
            CompletionState::Complete
        );
        assert!(subs.pending.contains_key(&2));
    }

    #[test]
    fn take_terminal_keeps_a_head_that_completes_between_reclaiming_and_delivery() {
        let mut subs = table(&[vec![Done]]);
        subs.callbacks.push((1, 1, Box::new(|_| {})));
        let calls = std::cell::Cell::new(0);
        let ready = subs.take_terminal(1, completes_after_first_look(&calls));
        assert!(ready.is_empty());
        assert_eq!(subs.callbacks.len(), 1);
        assert!(subs.pending.contains_key(&1));
    }

    #[test]
    fn claim_rejects_id_zero_unknown_ids_and_duplicates_without_changing_the_set() {
        let open = OpenEncoders::new();
        open.lock().extend([(5, Vec::new()), (6, Vec::new())]);
        for ids in [&[0_u64][..], &[5, 7][..], &[5, 5][..]] {
            let r = open.claim(ids);
            assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
            assert_eq!(open.lock().len(), 2);
        }
        assert!(open.claim(&[5, 6]).is_ok());
        assert!(open.lock().is_empty());
    }

    /// 記録 1 件（パイプラインの識別子だけを変える基準の値）。
    fn record_of(pipeline: u64) -> DispatchRecord {
        DispatchRecord {
            pipeline,
            bindings: Vec::new(),
            constants: Vec::new(),
            dispatch_dims: [1, 1, 1],
            threads_per_threadgroup: [1, 1, 1],
            threadgroup_memory: Vec::new(),
        }
    }

    #[test]
    fn record_appends_to_an_open_encoder_in_order() {
        let open = OpenEncoders::new();
        open.lock().insert(5, Vec::new());
        assert!(open.record(5, record_of(1)).is_ok());
        assert!(open.record(5, record_of(2)).is_ok());
        let claimed = open.claim(&[5]).expect("claim");
        assert_eq!(claimed, vec![vec![record_of(1), record_of(2)]]);
    }

    #[test]
    fn record_rejects_an_unopened_id_without_creating_it() {
        let open = OpenEncoders::new();
        open.lock().insert(5, Vec::new());
        for id in [0_u64, 6] {
            let r = open.record(id, record_of(1));
            assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
        }
        assert_eq!(open.lock().len(), 1);
        assert!(open.lock().get(&5).is_some_and(Vec::is_empty));
    }

    #[test]
    fn claim_returns_the_records_in_the_given_order_and_removes_them() {
        let open = OpenEncoders::new();
        open.lock().extend([
            (5, vec![record_of(1)]),
            (6, Vec::new()),
            (7, vec![record_of(3)]),
        ]);
        let claimed = open.claim(&[7, 5, 6]).expect("claim");
        assert_eq!(
            claimed,
            vec![vec![record_of(3)], vec![record_of(1)], Vec::new()]
        );
        assert!(open.lock().is_empty());
    }

    #[test]
    fn is_open_reports_membership_without_changing_the_set() {
        let open = OpenEncoders::new();
        open.lock().insert(5, Vec::new());
        assert!(!open.is_open(0));
        assert!(!open.is_open(6));
        assert!(open.is_open(5));
        assert!(open.is_open(5));
        assert_eq!(open.lock().len(), 1);
    }

    /// 全項目を受理する基準の入力（スロット 0 が束縛、1 が定数）。
    struct DispatchFixture {
        bindings: Vec<BindingArg>,
        constants: Vec<ConstantArg>,
        threadgroup: Vec<ThreadgroupArg>,
        layout: Vec<u32>,
        dims: [u32; 3],
        threads: [u32; 3],
        max_dims: [u32; 3],
        device_axes: [u64; 3],
        pipeline_total: u64,
        pipeline_static: u64,
        device_memory: u64,
    }

    impl DispatchFixture {
        fn new() -> Self {
            Self {
                bindings: vec![BindingArg {
                    slot: 0,
                    offset: 16,
                    buffer_len: 64,
                }],
                constants: vec![ConstantArg { slot: 1, len: 8 }],
                threadgroup: vec![ThreadgroupArg {
                    index: 0,
                    bytes: 32,
                }],
                layout: vec![0, 1],
                dims: [4, 2, 1],
                threads: [8, 4, 1],
                max_dims: [u32::MAX; 3],
                device_axes: [1024, 1024, 64],
                pipeline_total: 1024,
                pipeline_static: 0,
                device_memory: 32768,
            }
        }

        fn check(&self) -> Result<(), ContractError> {
            check_dispatch_args(&DispatchArgs {
                dispatch_dims: self.dims,
                threads_per_threadgroup: self.threads,
                bindings: &self.bindings,
                constants: &self.constants,
                threadgroup_memory: &self.threadgroup,
                layout_slots: &self.layout,
                pipeline_max_total_threads: self.pipeline_total,
                pipeline_static_threadgroup_memory: self.pipeline_static,
                device_max_threadgroup_memory: self.device_memory,
                device_max_threads_per_threadgroup: self.device_axes,
                max_dispatch_dims: self.max_dims,
            })
        }
    }

    /// 拒否が `InvalidArgument` で、文言に `needle` を含むこと。
    fn assert_invalid(r: Result<(), ContractError>, needle: &str) {
        match r {
            Err(ContractError::InvalidArgument(m)) => {
                assert!(m.contains(needle), "{m} should contain {needle}")
            }
            other => panic!("expected InvalidArgument({needle}), got {other:?}"),
        }
    }

    #[test]
    fn dispatch_args_baseline_is_accepted() {
        assert!(DispatchFixture::new().check().is_ok());
    }

    #[test]
    fn dispatch_dims_zero_and_over_limit_are_rejected() {
        let mut f = DispatchFixture::new();
        f.dims = [1, 0, 1];
        assert_invalid(f.check(), "dispatch_dims");
        let mut f = DispatchFixture::new();
        f.max_dims = [4, 2, 1];
        assert!(f.check().is_ok());
        f.dims = [4, 3, 1];
        assert_invalid(f.check(), "dispatch_dims");
    }

    #[test]
    fn threads_zero_axis_limit_and_product_overflow_are_rejected() {
        let mut f = DispatchFixture::new();
        f.threads = [0, 1, 1];
        assert_invalid(f.check(), "threads_per_threadgroup");
        let mut f = DispatchFixture::new();
        f.threads = [1025, 1, 1];
        assert_invalid(f.check(), "threads_per_threadgroup");
        let mut f = DispatchFixture::new();
        f.threads = [32, 32, 1];
        assert!(f.check().is_ok());
        f.pipeline_total = 1023;
        assert_invalid(f.check(), "threads_per_threadgroup");
        let mut f = DispatchFixture::new();
        f.device_axes = [u64::MAX; 3];
        f.threads = [u32::MAX; 3];
        f.pipeline_total = u64::MAX;
        assert_invalid(f.check(), "threads_per_threadgroup");
    }

    #[test]
    fn slot_limit_duplicates_and_layout_mismatch_are_rejected() {
        let mut f = DispatchFixture::new();
        f.bindings[0].slot = 30;
        f.layout = vec![30, 1];
        assert!(f.check().is_ok());
        f.bindings[0].slot = 31;
        f.layout = vec![31, 1];
        assert_invalid(f.check(), "31");

        let mut f = DispatchFixture::new();
        f.constants[0].slot = 0;
        f.layout = vec![0];
        assert_invalid(f.check(), "0");

        let mut f = DispatchFixture::new();
        f.layout = vec![0];
        assert_invalid(f.check(), "1");

        let mut f = DispatchFixture::new();
        f.layout = vec![0, 1, 7];
        assert_invalid(f.check(), "7");
    }

    #[test]
    fn binding_offset_accepts_the_length_and_rejects_one_more() {
        let mut f = DispatchFixture::new();
        f.bindings[0].offset = 64;
        assert!(f.check().is_ok());
        f.bindings[0].offset = 68;
        assert_invalid(f.check(), "exceeds the buffer length");
    }

    #[test]
    fn binding_offset_not_multiple_of_four_is_rejected() {
        let mut f = DispatchFixture::new();
        for offset in [1, 2, 3, 62] {
            f.bindings[0].offset = offset;
            assert_invalid(f.check(), "not a multiple of 4");
        }
        f.bindings[0].offset = 60;
        assert!(f.check().is_ok());
    }

    #[test]
    fn constant_length_bounds_and_empty_are_rejected() {
        let mut f = DispatchFixture::new();
        f.constants[0].len = 4096;
        assert!(f.check().is_ok());
        f.constants[0].len = 4097;
        assert_invalid(f.check(), "slot 1");
        f.constants[0].len = 0;
        assert_invalid(f.check(), "slot 1");
    }

    #[test]
    fn threadgroup_memory_index_alignment_duplicates_and_total_are_checked() {
        let mut f = DispatchFixture::new();
        f.threadgroup[0].index = 30;
        assert!(f.check().is_ok());
        f.threadgroup[0].index = 31;
        assert_invalid(f.check(), "31");

        let mut f = DispatchFixture::new();
        f.threadgroup[0].bytes = 24;
        assert_invalid(f.check(), "index 0");

        let mut f = DispatchFixture::new();
        f.threadgroup.push(ThreadgroupArg {
            index: 0,
            bytes: 16,
        });
        assert_invalid(f.check(), "index: 0");

        let mut f = DispatchFixture::new();
        f.pipeline_static = 100;
        f.device_memory = 132;
        assert!(f.check().is_ok());
        f.device_memory = 131;
        assert_invalid(f.check(), "threadgroup memory");
        f.threadgroup[0].bytes = 48;
        f.device_memory = 148;
        assert!(f.check().is_ok());
        f.threadgroup[0].bytes = 64;
        assert_invalid(f.check(), "threadgroup memory");
    }

    #[test]
    fn closing_removes_the_id_and_is_harmless_twice() {
        let open = OpenEncoders::new();
        open.lock().insert(9, vec![record_of(1)]);
        open.close(9);
        open.close(9);
        assert!(open.lock().is_empty());
    }

    /// 束縛 1 件だけを持つ記録（slot・識別子・世代を指定する）。
    fn bound(slot: u32, buffer_id: u64, generation: u64) -> DispatchRecord {
        DispatchRecord {
            bindings: vec![RecordedBinding {
                slot,
                buffer_id,
                generation,
                offset: 0,
            }],
            ..record_of(1)
        }
    }

    /// 識別子 1 に世代 3 のバッファだけがある表の引き方。
    fn table_with_buffer_one(id: u64) -> Option<u64> {
        (id == 1).then_some(3)
    }

    #[test]
    fn recorded_buffers_that_are_all_alive_are_accepted() {
        let records = vec![vec![bound(0, 1, 3)], vec![bound(1, 1, 3), record_of(2)]];
        assert!(check_recorded_buffers(&records, table_with_buffer_one, 3).is_ok());
        assert!(check_recorded_buffers(&[], table_with_buffer_one, 3).is_ok());
    }

    #[test]
    fn a_missing_stale_or_old_generation_buffer_is_rejected_with_its_slot() {
        let cases = [
            ("missing", bound(4, 2, 3), 3),
            ("entry generation", bound(5, 1, 2), 2),
            ("device generation", bound(6, 1, 3), 4),
        ];
        for (name, record, current) in cases {
            let slot = record.bindings[0].slot;
            let r = check_recorded_buffers(&[vec![record]], table_with_buffer_one, current);
            match r {
                Err(ContractError::InvalidArgument(msg)) => {
                    assert!(msg.contains(&format!("slot {slot}")), "{name}: {msg}");
                }
                other => panic!("{name}: {other:?}"),
            }
        }
    }

    #[test]
    fn a_failure_in_a_later_group_rejects_the_whole_set() {
        let records = vec![vec![bound(0, 1, 3)], vec![bound(7, 9, 3)]];
        let r = check_recorded_buffers(&records, table_with_buffer_one, 3);
        match r {
            Err(ContractError::InvalidArgument(msg)) => assert!(msg.contains("slot 7"), "{msg}"),
            other => panic!("{other:?}"),
        }
    }

    /// 計画の試験の入力（パイプラインの識別子と共有メモリの index）。
    #[derive(Debug, PartialEq)]
    struct Step(u64, Vec<u32>);

    fn plan(groups: Vec<Vec<Step>>) -> Vec<Vec<(Step, bool)>> {
        encode_plan(groups, |s| s.0, |s| s.1.as_slice())
    }

    fn summary(runs: &[Vec<(Step, bool)>]) -> Vec<Vec<(u64, bool)>> {
        runs.iter()
            .map(|run| run.iter().map(|(s, set)| (s.0, *set)).collect())
            .collect()
    }

    #[test]
    fn empty_groups_make_no_run_and_groups_do_not_share_a_run() {
        assert!(plan(vec![]).is_empty());
        assert!(plan(vec![vec![], vec![]]).is_empty());
        let runs = plan(vec![
            vec![Step(1, vec![])],
            vec![],
            vec![Step(1, vec![]), Step(1, vec![])],
        ]);
        assert_eq!(
            summary(&runs),
            vec![vec![(1, true)], vec![(1, true), (1, false)]]
        );
    }

    #[test]
    fn the_pipeline_is_set_again_only_when_it_changes() {
        let runs = plan(vec![vec![
            Step(1, vec![]),
            Step(1, vec![]),
            Step(2, vec![]),
            Step(1, vec![]),
        ]]);
        assert_eq!(
            summary(&runs),
            vec![vec![(1, true), (1, false), (2, true), (1, true)]]
        );
    }

    #[test]
    fn a_missing_threadgroup_index_ends_the_run_and_sets_the_pipeline_again() {
        let runs = plan(vec![vec![
            Step(1, vec![0]),
            Step(1, vec![0, 1]),
            Step(1, vec![0]),
            Step(1, vec![]),
        ]]);
        assert_eq!(
            summary(&runs),
            vec![
                vec![(1, true), (1, false)],
                vec![(1, true)],
                vec![(1, true)]
            ]
        );
    }

    #[test]
    fn every_record_comes_out_once_in_order() {
        let groups = vec![
            vec![Step(1, vec![2]), Step(2, vec![2]), Step(2, vec![])],
            vec![Step(3, vec![])],
        ];
        let flat: Vec<u64> = plan(groups)
            .into_iter()
            .flatten()
            .map(|(s, _)| s.0)
            .collect();
        assert_eq!(flat, vec![1, 2, 2, 3]);
    }

    #[test]
    fn pool_returns_the_same_slot_only_for_the_same_length() {
        let mut pool: SizePool<&str> = SizePool::new();
        assert!(pool.put(16, 7, "a").is_none());
        assert!(pool.take(32).is_none());
        assert_eq!(pool.entries, 1);
        assert_eq!(pool.take(16), Some((7, "a")));
        assert_eq!(pool.entries, 0);
        assert_eq!(pool.bytes, 0);
        assert!(pool.take(16).is_none());
    }

    #[test]
    fn pool_counts_reuses_and_slot_matches() {
        let mut pool: SizePool<u8> = SizePool::new();
        assert!(pool.take(16).is_none());
        assert_eq!(pool.reuse_count, 0);
        for round in 0..10_u8 {
            assert!(pool.put(16, 3, round).is_none());
            assert_eq!(pool.take(16), Some((3, round)));
        }
        assert_eq!(pool.reuse_count, 10);
        assert_eq!(pool.slot_match_count, 10);
        // 別の slot を入れた後に古い slot を取り出しても、一致には数えない。
        assert!(pool.put(16, 4, 0).is_none());
        assert!(pool.put(16, 5, 1).is_none());
        assert_eq!(pool.take(16), Some((5, 1)));
        assert_eq!(pool.take(16), Some((4, 0)));
        assert_eq!(pool.reuse_count, 12);
        assert_eq!(pool.slot_match_count, 11);
    }

    #[test]
    fn pool_rejects_past_the_entry_limit_and_keeps_its_contents() {
        let mut pool: SizePool<usize> = SizePool::new();
        for i in 0..POOL_MAX_ENTRIES {
            assert!(pool.put(8, i as u64, i).is_none());
        }
        assert_eq!(pool.put(8, 99, 99), Some(99));
        assert_eq!(pool.entries, POOL_MAX_ENTRIES);
        assert_eq!(pool.bytes, 8 * POOL_MAX_ENTRIES as u64);
        // 取り出すと空きができる。
        assert!(pool.take(8).is_some());
        assert!(pool.put(8, 99, 99).is_none());
    }

    #[test]
    fn pool_rejects_past_the_byte_limit_and_overflow() {
        let mut pool: SizePool<u8> = SizePool::new();
        assert!(pool.put(POOL_MAX_BYTES, 1, 0).is_none());
        assert_eq!(pool.put(1, 2, 1), Some(1));
        assert_eq!(pool.bytes, POOL_MAX_BYTES);
        assert_eq!(pool.entries, 1);
        let mut empty: SizePool<u8> = SizePool::new();
        assert_eq!(empty.put(POOL_MAX_BYTES + 1, 1, 0), Some(0));
        assert_eq!(empty.put(u64::MAX, 1, 0), Some(0));
        assert_eq!(empty.entries, 0);
        assert!(empty.by_len.is_empty());
    }

    #[test]
    fn would_accept_follows_the_limits_without_changing_the_pool() {
        let mut pool: SizePool<u8> = SizePool::new();
        assert!(pool.would_accept(1));
        assert!(pool.would_accept(POOL_MAX_BYTES));
        assert!(!pool.would_accept(POOL_MAX_BYTES + 1));
        assert!(!pool.would_accept(u64::MAX));
        assert_eq!(pool.entries, 0);
        assert!(pool.put(POOL_MAX_BYTES - 1, 1, 0).is_none());
        assert!(pool.would_accept(1));
        assert!(!pool.would_accept(2));
        for i in 0..POOL_MAX_ENTRIES - 1 {
            let _ = pool.put(0, i as u64, 0);
        }
        assert!(!pool.would_accept(0));
    }

    /// `retire` の試験用に、呼ばれた順を記録する。
    fn run_retire(
        pool: &mut SizePool<u8>,
        byte_len: u64,
        zero_ok: bool,
        calls: &mut Vec<&'static str>,
    ) -> Option<u8> {
        let pool = std::cell::RefCell::new(pool);
        let calls = std::cell::RefCell::new(calls);
        retire(
            byte_len,
            9_u8,
            |len| {
                calls.borrow_mut().push("accepts");
                pool.borrow().would_accept(len)
            },
            |_buffer: &mut u8| {
                calls.borrow_mut().push("zero");
                if zero_ok { Ok(()) } else { Err(()) }
            },
            |buffer| {
                calls.borrow_mut().push("put");
                pool.borrow_mut().put(byte_len, 1, buffer)
            },
        )
    }

    #[test]
    fn retire_zeroes_then_puts_when_the_pool_accepts() {
        let mut pool: SizePool<u8> = SizePool::new();
        let mut calls = Vec::new();
        assert_eq!(run_retire(&mut pool, 16, true, &mut calls), None);
        assert_eq!(calls, ["accepts", "zero", "put"]);
        assert_eq!(pool.take(16), Some((1, 9)));
    }

    #[test]
    fn retire_does_not_put_a_buffer_that_failed_to_zero() {
        let mut pool: SizePool<u8> = SizePool::new();
        let mut calls = Vec::new();
        assert_eq!(run_retire(&mut pool, 16, false, &mut calls), Some(9));
        assert_eq!(calls, ["accepts", "zero"]);
        assert_eq!(pool.entries, 0);
    }

    #[test]
    fn retire_does_not_zero_a_size_the_pool_will_not_accept() {
        let mut pool: SizePool<u8> = SizePool::new();
        let mut calls = Vec::new();
        assert_eq!(
            run_retire(&mut pool, POOL_MAX_BYTES + 1, true, &mut calls),
            Some(9)
        );
        assert_eq!(calls, ["accepts"]);
        assert_eq!(pool.entries, 0);
    }

    // ---- ロストの後の完了 callback（世代の違う登録。TASK-2932）----

    #[test]
    fn take_stale_splits_by_generation_and_returns_lost_once() {
        let seen: Seen = Arc::default();
        let (mut subs, _groups) = fake_table(&[0, 0, 0]);
        assert!(
            subs.register(1, 1, 1, recorder(&seen, 1), fake_state)
                .is_none()
        );
        assert!(
            subs.register(2, 2, 2, recorder(&seen, 2), fake_state)
                .is_none()
        );
        assert!(
            subs.register(3, 1, 1, recorder(&seen, 3), fake_state)
                .is_none()
        );

        call_all(subs.take_stale(2));
        assert_eq!(
            *seen.lock().unwrap(),
            [(1, CompletionState::Lost), (3, CompletionState::Lost)]
        );
        // 同じ世代の登録は残り、もう一度掃いても二重には呼ばれない。
        assert_eq!(subs.callbacks.len(), 1);
        call_all(subs.take_stale(2));
        assert_eq!(seen.lock().unwrap().len(), 2);
    }

    #[test]
    fn a_stale_registration_is_never_completed_even_when_the_group_is_done() {
        let seen: Seen = Arc::default();
        let (mut subs, groups) = fake_table(&[0]);
        assert!(
            subs.register(1, 1, 1, recorder(&seen, 1), fake_state)
                .is_none()
        );
        groups[0].store(1, Ordering::SeqCst);
        // 世代が違えば、列が完了していても `Complete` は渡さず `Lost` を渡す。
        call_all(subs.take_terminal(2, fake_state));
        assert_eq!(*seen.lock().unwrap(), [(1, CompletionState::Lost)]);
        assert!(subs.callbacks.is_empty());
    }

    #[test]
    fn a_stale_registration_is_lost_on_the_final_drain_too() {
        let seen: Seen = Arc::default();
        let (mut subs, groups) = fake_table(&[0]);
        assert!(
            subs.register(1, 1, 1, recorder(&seen, 1), fake_state)
                .is_none()
        );
        groups[0].store(1, Ordering::SeqCst);
        call_all(subs.take_all(2, fake_state));
        assert_eq!(*seen.lock().unwrap(), [(1, CompletionState::Lost)]);
    }

    #[test]
    fn sweeping_then_driving_calls_each_registration_exactly_once() {
        let seen: Seen = Arc::default();
        let (subs, groups) = fake_table(&[0, 0]);
        let table = Mutex::new(subs);
        let call = |callback: Callback, state| callback(state);
        register_table(&table, 1, 1, || 1, recorder(&seen, 1), fake_state, call);
        register_table(&table, 2, 2, || 1, recorder(&seen, 2), fake_state, call);
        groups[0].store(1, Ordering::SeqCst);
        groups[1].store(1, Ordering::SeqCst);

        sweep_stale_table(&table, 2, call);
        drive_table(&table, 2, || 2, fake_state, call);
        drive_table(&table, 2, || 2, fake_state, call);
        assert_eq!(
            *seen.lock().unwrap(),
            [(1, CompletionState::Lost), (2, CompletionState::Complete)]
        );
    }

    #[test]
    fn a_newer_registration_is_left_alone_by_an_older_reading() {
        let seen: Seen = Arc::default();
        let (subs, groups) = fake_table(&[1, 1]);
        let table = Mutex::new(subs);
        let call = |callback: Callback, state| callback(state);
        // 古い世代 1 の登録と、ロストの後に登録された新しい世代 2 の登録。列はどちらも完了済み。
        register_table(
            &table,
            1,
            1,
            || 1,
            recorder(&seen, 1),
            |_| GroupState::Pending,
            call,
        );
        register_table(
            &table,
            2,
            2,
            || 2,
            recorder(&seen, 2),
            |_| GroupState::Pending,
            call,
        );
        assert_eq!(lock_table(&table).callbacks.len(), 2);
        groups[0].store(1, Ordering::SeqCst);

        // 世代 1 を読んだ操作の駆動は、世代 2 の登録に触れない（`Lost` も `Complete` も渡さない）。
        drive_table(&table, 1, || 1, fake_state, call);
        assert_eq!(*seen.lock().unwrap(), [(1, CompletionState::Complete)]);
        assert_eq!(lock_table(&table).callbacks.len(), 1);
        // 世代 1 を読んだ掃き出しも同じ。
        sweep_stale_table(&table, 1, call);
        assert_eq!(lock_table(&table).callbacks.len(), 1);

        // 次の世代 2 の駆動で、通常の値が 1 回だけ届く。
        drive_table(&table, 2, || 2, fake_state, call);
        drive_table(&table, 2, || 2, fake_state, call);
        assert_eq!(
            *seen.lock().unwrap(),
            [
                (1, CompletionState::Complete),
                (2, CompletionState::Complete)
            ]
        );
    }

    #[test]
    fn stale_sweeping_and_the_final_drain_follow_the_same_ordering_rule() {
        let seen: Seen = Arc::default();
        let (mut subs, _groups) = fake_table(&[1, 1, 1]);
        assert!(
            subs.register(1, 1, 1, recorder(&seen, 1), |_| GroupState::Pending)
                .is_none()
        );
        assert!(
            subs.register(2, 2, 2, recorder(&seen, 2), |_| GroupState::Pending)
                .is_none()
        );
        assert!(
            subs.register(3, 3, 3, recorder(&seen, 3), |_| GroupState::Pending)
                .is_none()
        );
        // 世代 2 を読んだ掃き出しは、世代 1 だけを外す。
        call_all(subs.take_stale(2));
        assert_eq!(*seen.lock().unwrap(), [(1, CompletionState::Lost)]);
        assert_eq!(subs.callbacks.len(), 2);
        // 最後の掃き出し（Drop）では、世代 2 は今の世代と同じ扱いで通常の値、世代 3 も通常の値になる。
        call_all(subs.take_all(2, fake_state));
        assert_eq!(
            *seen.lock().unwrap(),
            [
                (1, CompletionState::Lost),
                (2, CompletionState::Complete),
                (3, CompletionState::Complete)
            ]
        );
        assert!(subs.callbacks.is_empty());
    }

    #[test]
    fn registering_with_an_older_generation_than_now_gets_lost_at_once() {
        let seen: Seen = Arc::default();
        let (subs, _groups) = fake_table(&[0]);
        let table = Mutex::new(subs);
        let call = |callback: Callback, state| callback(state);
        // 所有者の検査の後でロストと掃き出しが済んだ想定。錠の中で読んだ世代 2 が登録の世代 1 より大きい。
        register_table(&table, 1, 1, || 2, recorder(&seen, 1), fake_state, call);
        assert_eq!(*seen.lock().unwrap(), [(1, CompletionState::Lost)]);
        assert!(lock_table(&table).callbacks.is_empty());
    }

    /// 並行の試験で使う、各登録の呼ばれた回数と値。
    fn tally(seen: &Seen, count: u64) -> Vec<Vec<CompletionState>> {
        let mut by_tag = vec![Vec::new(); count as usize];
        for (tag, state) in seen.lock().unwrap().iter() {
            by_tag[*tag as usize].push(*state);
        }
        by_tag
    }

    #[test]
    fn a_concurrent_sweep_and_drives_call_each_registration_exactly_once() {
        const N: u64 = 16;
        for _ in 0..50 {
            let seen: Seen = Arc::default();
            let states = [1_u8; N as usize];
            let (subs, _groups) = fake_table(&states);
            let table = Mutex::new(subs);
            let generation = AtomicU64::new(1);
            let call = |callback: Callback, state| callback(state);
            for tag in 0..N {
                register_table(
                    &table,
                    tag + 1,
                    1,
                    || 1,
                    recorder(&seen, tag),
                    |_| GroupState::Pending,
                    call,
                );
            }
            let barrier = std::sync::Barrier::new(4);
            std::thread::scope(|scope| {
                scope.spawn(|| {
                    barrier.wait();
                    generation.store(2, Ordering::Release);
                    sweep_stale_table(&table, 2, call);
                });
                for _ in 0..3 {
                    scope.spawn(|| {
                        barrier.wait();
                        for _ in 0..4 {
                            drive_table(
                                &table,
                                generation.load(Ordering::Acquire),
                                || generation.load(Ordering::Acquire),
                                fake_state,
                                call,
                            );
                        }
                    });
                }
            });
            drive_table(&table, 2, || 2, fake_state, call);
            for calls in tally(&seen, N) {
                assert_eq!(calls.len(), 1, "{calls:?}");
                assert!(matches!(
                    calls[0],
                    CompletionState::Lost | CompletionState::Complete
                ));
            }
            assert!(lock_table(&table).callbacks.is_empty());
        }
    }

    #[test]
    #[cfg(feature = "fault-injection")]
    fn a_concurrent_loss_and_registrations_call_each_registration_exactly_once() {
        const N: u64 = 16;
        for _ in 0..50 {
            let seen: Seen = Arc::default();
            // 偶数番は完了済み、奇数番は未完了の列。
            let states: Vec<u8> = (0..N).map(|i| u8::from(i % 2 == 0)).collect();
            let (subs, _groups) = fake_table(&states);
            let table = Mutex::new(subs);
            let lost = LostState::default();
            let (generation, source) = (AtomicU64::new(1), AtomicU64::new(2));
            let call = |callback: Callback, state| callback(state);
            let barrier = std::sync::Barrier::new(3);
            std::thread::scope(|scope| {
                scope.spawn(|| {
                    barrier.wait();
                    if lost
                        .lose(DeviceLostReason::Unknown, &generation, &source)
                        .unwrap()
                    {
                        sweep_stale_table(&table, generation.load(Ordering::Acquire), call);
                    }
                });
                for half in 0..2_u64 {
                    let (seen, table, generation, barrier) = (&seen, &table, &generation, &barrier);
                    scope.spawn(move || {
                        barrier.wait();
                        for tag in (half * N / 2)..((half + 1) * N / 2) {
                            // 印の世代は 1（ロストの前に払い出された印）。
                            register_table(
                                table,
                                tag + 1,
                                1,
                                || generation.load(Ordering::Acquire),
                                recorder(seen, tag),
                                fake_state,
                                call,
                            );
                        }
                    });
                }
            });
            // 残りは最後の掃き出し。世代 2 を読む駆動で、古い世代の登録は `Lost` になる。
            drive_table(&table, 2, || 2, fake_state, call);
            for (tag, calls) in tally(&seen, N).into_iter().enumerate() {
                assert_eq!(calls.len(), 1, "tag {tag}: {calls:?}");
                if tag % 2 == 1 {
                    // 未完了の列は、ロストの前後どちらに登録しても `Lost`。
                    assert_eq!(calls[0], CompletionState::Lost);
                } else {
                    assert!(matches!(
                        calls[0],
                        CompletionState::Lost | CompletionState::Complete
                    ));
                }
            }
            assert!(lock_table(&table).callbacks.is_empty());
        }
    }

    #[test]
    fn the_generation_is_read_again_after_the_drive() {
        let generation = AtomicU64::new(1);
        let seen_by_drive = Mutex::new(None);
        let used = driven_generation(
            |g0| {
                *seen_by_drive.lock().unwrap() = Some(g0);
                // 駆動中の callback が再入して世代を進めた想定。
                generation.store(2, Ordering::Release);
            },
            || generation.load(Ordering::Acquire),
        );
        assert_eq!(*seen_by_drive.lock().unwrap(), Some(1));
        assert_eq!(used, 2);
        // 動かなければ同じ値。
        assert_eq!(driven_generation(|_| {}, || 5), 5);
    }

    #[test]
    fn remove_issued_matches_the_issue_generation_not_the_current_one() {
        // 項目は (払い出しの世代, 中身)。device の今の世代は引数に現れない。
        let mut map: HashMap<u64, (u64, &str)> = HashMap::new();
        map.insert(1, (1, "a"));
        map.insert(2, (2, "b"));
        let generation_of = |entry: &(u64, &str)| entry.0;

        // 世代が違う・表に無い識別子は何も外さない。
        assert_eq!(remove_issued(&mut map, 1, 2, generation_of), None);
        assert_eq!(remove_issued(&mut map, 9, 1, generation_of), None);
        assert_eq!(map.len(), 2);
        // 一致すれば外れ、2 回目は何も起きない。別の項目は残る。
        assert_eq!(remove_issued(&mut map, 1, 1, generation_of), Some((1, "a")));
        assert_eq!(remove_issued(&mut map, 1, 1, generation_of), None);
        assert_eq!(map.len(), 1);
        assert!(map.contains_key(&2));
    }

    #[test]
    fn remove_issued_ignores_forged_ids_and_an_empty_table() {
        let generation_of = |entry: &(u64, &str)| entry.0;
        // 空の表では、どの識別子・世代でも何も外さず、panic もしない。
        let mut empty: HashMap<u64, (u64, &str)> = HashMap::new();
        assert_eq!(remove_issued(&mut empty, 0, 0, generation_of), None);
        assert_eq!(
            remove_issued(&mut empty, u64::MAX, u64::MAX, generation_of),
            None
        );
        // 偽造の識別子は、表の別の項目を巻き込まない。
        let mut map: HashMap<u64, (u64, &str)> = HashMap::new();
        map.insert(1, (1, "a"));
        assert_eq!(remove_issued(&mut map, u64::MAX, 1, generation_of), None);
        assert_eq!(map.get(&1), Some(&(1, "a")));
    }

    #[test]
    fn a_reentrant_loss_in_the_first_callback_turns_the_second_into_lost() {
        let seen: Seen = Arc::default();
        let (mut subs, _groups) = fake_table(&[1, 1]);
        let generation = Arc::new(AtomicU64::new(1));
        // 1 件目は呼ばれたときに世代を進める（ロストへの再入の見立て）。
        let advance = {
            let (seen, generation) = (Arc::clone(&seen), Arc::clone(&generation));
            Box::new(move |state| {
                seen.lock().unwrap().push((1, state));
                generation.store(2, Ordering::Release);
            }) as Callback
        };
        subs.callbacks.push((1, 1, advance));
        subs.callbacks.push((2, 1, recorder(&seen, 2)));
        subs.callbacks.push((3, 2, recorder(&seen, 3)));

        // 世代 1 の読みで取り出す。世代 2 の登録は新しいので残る。
        let ready = subs.take_terminal(1, fake_state);
        assert_eq!(ready.len(), 2);
        assert_eq!(subs.callbacks.len(), 1);
        let read = || generation.load(Ordering::Acquire);
        assert!(run_callbacks(ready, read, |callback, state| callback(state)).is_none());

        // 1 件目は先に決めた `Complete` のまま、2 件目は `Lost`。各 1 回。
        assert_eq!(
            *seen.lock().unwrap(),
            [(1, CompletionState::Complete), (2, CompletionState::Lost)]
        );
    }

    #[test]
    fn take_next_does_not_move_an_exhausted_counter() {
        let counter = AtomicU64::new(u64::MAX);
        assert_eq!(take_next(&counter), None);
        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
    }

    // ---- create_pipeline の純粋な部分（TASK-773 のポインタ。device に触らない） ----

    fn constant(index: u32, value: FunctionConstantValue) -> FunctionConstant {
        FunctionConstant { index, value }
    }

    #[test]
    fn classify_accepts_msl_and_msl_specialized() {
        let (source, entry, constants, specialized) =
            classify_pipeline_input(&PipelineInput::Msl {
                source: "src",
                entry_point: "k",
            })
            .unwrap();
        assert_eq!(
            (source, entry, constants.len(), specialized),
            ("src", "k", 0, false)
        );

        let given = [constant(2, FunctionConstantValue::U32(5))];
        let (source, entry, constants, specialized) =
            classify_pipeline_input(&PipelineInput::MslSpecialized {
                source: "s2",
                entry_point: "e2",
                function_constants: &given,
            })
            .unwrap();
        assert_eq!(
            (source, entry, constants.len(), specialized),
            ("s2", "e2", 1, true)
        );
        assert_eq!(constants[0].index, 2);
    }

    /// 定数が空の `MslSpecialized` は、`Msl` と区別できる（特殊化の API へ進む印が立つ）。
    #[test]
    fn classify_distinguishes_empty_msl_specialized_from_msl() {
        let (_, _, msl_constants, msl_specialized) = classify_pipeline_input(&PipelineInput::Msl {
            source: "s",
            entry_point: "k",
        })
        .unwrap();
        let (_, _, spec_constants, spec_specialized) =
            classify_pipeline_input(&PipelineInput::MslSpecialized {
                source: "s",
                entry_point: "k",
                function_constants: &[],
            })
            .unwrap();
        assert!(msl_constants.is_empty() && spec_constants.is_empty());
        assert!(!msl_specialized);
        assert!(spec_specialized);
    }

    #[test]
    fn classify_marks_wgsl_unsupported() {
        let inputs = [PipelineInput::Wgsl {
            source: "s",
            entry_point: "k",
        }];
        for input in &inputs {
            let r = classify_pipeline_input(input);
            assert!(
                matches!(
                    r,
                    Err(ContractError::Unsupported {
                        op: "create_pipeline",
                        backend: BackendKind::Metal
                    })
                ),
                "{input:?}: {r:?}"
            );
        }
    }

    #[test]
    fn classify_accepts_option_variants_like_their_plain_forms() {
        let constants = [constant(0, FunctionConstantValue::U32(7))];
        let (source, entry, found, specialized) =
            classify_pipeline_input(&PipelineInput::MslWithOptions {
                source: "src-a",
                entry_point: "ka",
                math_mode: MslMathMode::Safe,
            })
            .expect("MslWithOptions");
        assert_eq!(
            (source, entry, found.len(), specialized),
            ("src-a", "ka", 0, false)
        );
        let (source, entry, found, specialized) =
            classify_pipeline_input(&PipelineInput::MslSpecializedWithOptions {
                source: "src-b",
                entry_point: "kb",
                function_constants: &constants,
                math_mode: MslMathMode::Fast,
                float_functions: MslFloatFunctions::Precise,
            })
            .expect("MslSpecializedWithOptions");
        assert_eq!(
            (source, entry, found.len(), specialized),
            ("src-b", "kb", 1, true)
        );
        let (_, _, found, specialized) =
            classify_pipeline_input(&PipelineInput::MslSpecializedWithOptions {
                source: "s",
                entry_point: "k",
                function_constants: &[],
                math_mode: MslMathMode::Fast,
                float_functions: MslFloatFunctions::Fast,
            })
            .expect("empty constants");
        assert!(found.is_empty() && specialized);
    }

    #[test]
    fn compile_plan_sets_only_safe_math_and_precise_functions() {
        // COMP-99
        let plan = |input: PipelineInput<'_>| compile_plan(&input).expect("plan");
        assert_eq!(
            plan(PipelineInput::Msl {
                source: "s",
                entry_point: "k"
            }),
            CompilePlan::default()
        );
        assert_eq!(
            plan(PipelineInput::MslSpecialized {
                source: "s",
                entry_point: "k",
                function_constants: &[]
            }),
            CompilePlan::default()
        );
        for (mode, safe_math) in [(MslMathMode::Safe, true), (MslMathMode::Fast, false)] {
            assert_eq!(
                plan(PipelineInput::MslWithOptions {
                    source: "s",
                    entry_point: "k",
                    math_mode: mode,
                }),
                CompilePlan {
                    safe_math,
                    precise_functions: false
                }
            );
            for (functions, precise_functions) in [
                (MslFloatFunctions::Precise, true),
                (MslFloatFunctions::Fast, false),
            ] {
                assert_eq!(
                    plan(PipelineInput::MslSpecializedWithOptions {
                        source: "s",
                        entry_point: "k",
                        function_constants: &[],
                        math_mode: mode,
                        float_functions: functions,
                    }),
                    CompilePlan {
                        safe_math,
                        precise_functions
                    }
                );
            }
        }
    }

    #[test]
    fn classify_rejects_other_languages_and_names_the_variant() {
        let words = [0_u32; 1];
        let bytes = [0_u8; 1];
        let cases = [
            (
                PipelineInput::SpirV {
                    words: &words,
                    entry_point: "k",
                },
                "SpirV",
            ),
            (
                PipelineInput::CudaC {
                    source: "s",
                    entry_point: "k",
                },
                "CudaC",
            ),
            (
                PipelineInput::Ptx {
                    source: "s",
                    entry_point: "k",
                },
                "Ptx",
            ),
            (
                PipelineInput::Dxil {
                    bytes: &bytes,
                    entry_point: "k",
                },
                "Dxil",
            ),
        ];
        for (input, name) in &cases {
            match classify_pipeline_input(input) {
                Err(ContractError::ArtifactRejected { kind, reason }) => {
                    assert_eq!(kind, "unsupported-input");
                    assert_eq!(
                        reason,
                        format!(
                            "pipeline input {name} is not accepted by the Metal backend; accepted inputs are Msl, MslSpecialized, MslWithOptions and MslSpecializedWithOptions"
                        )
                    );
                }
                other => panic!("{name}: {other:?}"),
            }
        }
    }

    #[test]
    fn slots_empty_and_distinct_pass_duplicates_are_invalid_argument() {
        assert!(check_unique_slots(&[]).is_ok());
        assert!(check_unique_slots(&[0, 1, 4, u32::MAX]).is_ok());
        for slots in [&[3_u32, 3][..], &[0, 1, 2, 1], &[7, 0, 7]] {
            let r = check_unique_slots(slots);
            assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
        }
        let Err(ContractError::InvalidArgument(msg)) = check_unique_slots(&[0, 5, 5]) else {
            panic!("expected InvalidArgument");
        };
        assert!(msg.contains('5'), "{msg}");
    }

    #[test]
    fn duplicate_constant_indexes_are_invalid_argument() {
        let ok = [
            constant(0, FunctionConstantValue::U32(1)),
            constant(1, FunctionConstantValue::Bool(true)),
        ];
        assert!(check_unique_constants(&[]).is_ok());
        assert!(check_unique_constants(&ok).is_ok());
        let dup = [
            constant(4, FunctionConstantValue::U32(1)),
            constant(4, FunctionConstantValue::Bool(false)),
        ];
        let r = check_unique_constants(&dup);
        assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
        assert!(matches!(
            prepare_constants(&dup),
            Err(ContractError::InvalidArgument(_))
        ));
    }

    #[test]
    fn constant_index_above_msl_limit_is_invalid_argument() {
        let at = |i| [constant(i, FunctionConstantValue::U32(1))];
        assert!(check_constant_index_range(&at(0)).is_ok());
        assert!(check_constant_index_range(&at(MAX_FUNCTION_CONSTANT_INDEX)).is_ok());
        assert!(prepare_constants(&at(65535)).is_ok());
        for bad in [65536, u32::MAX] {
            let r = check_constant_index_range(&at(bad));
            assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
            let r = prepare_constants(&at(bad));
            let Err(ContractError::InvalidArgument(msg)) = r else {
                panic!("{r:?}");
            };
            assert!(msg.contains(&bad.to_string()), "{msg}");
        }
        // 重複と範囲外の両方があるときは、範囲外を先に返す。
        let both = [
            constant(65536, FunctionConstantValue::U32(1)),
            constant(65536, FunctionConstantValue::U32(2)),
        ];
        let Err(ContractError::InvalidArgument(msg)) = prepare_constants(&both) else {
            panic!();
        };
        assert!(msg.contains("out of range"), "{msg}");
    }

    #[test]
    fn constant_wire_maps_u32_and_bool() {
        for v in [0_u32, 1, u32::MAX] {
            let (kind, bytes) = constant_wire(&FunctionConstantValue::U32(v)).unwrap();
            assert_eq!(kind, ConstantKind::UInt);
            assert_eq!(bytes, v.to_ne_bytes());
            assert_eq!(bytes.len(), kind.byte_len());
        }
        for (v, byte) in [(false, 0_u8), (true, 1)] {
            let (kind, bytes) = constant_wire(&FunctionConstantValue::Bool(v)).unwrap();
            assert_eq!(kind, ConstantKind::Bool);
            assert_eq!(bytes, [byte]);
            assert_eq!(bytes.len(), kind.byte_len());
        }
    }

    #[test]
    fn missing_required_lists_absent_required_indexes_in_order() {
        let declared = [(3, true), (0, true), (1, false), (2, true)];
        assert_eq!(missing_required_constants(&declared, &[2]), vec![0, 3]);
    }

    #[test]
    fn missing_required_is_empty_when_all_required_are_provided() {
        let declared = [(0, true), (1, true)];
        assert!(missing_required_constants(&declared, &[1, 0]).is_empty());
    }

    #[test]
    fn missing_required_ignores_optional_declarations() {
        let declared = [(0, false), (1, false)];
        assert!(missing_required_constants(&declared, &[]).is_empty());
    }

    #[test]
    fn missing_required_does_not_object_to_undeclared_provided_index() {
        let declared = [(0, true)];
        assert!(missing_required_constants(&declared, &[0, 7, 65535]).is_empty());
    }

    #[test]
    fn missing_required_with_no_declarations_is_empty() {
        assert!(missing_required_constants(&[], &[]).is_empty());
        assert!(missing_required_constants(&[], &[1]).is_empty());
    }

    #[test]
    fn missing_required_reason_names_the_indexes() {
        assert_eq!(
            missing_required_reason(&[1]),
            "required function constant at index 1 was not provided"
        );
        assert_eq!(
            missing_required_reason(&[0, 3]),
            "required function constant at index 0, 3 was not provided"
        );
    }

    #[test]
    fn prepare_constants_keeps_order_index_and_kind() {
        let given = [
            constant(9, FunctionConstantValue::Bool(true)),
            constant(0, FunctionConstantValue::U32(0)),
        ];
        let prepared = prepare_constants(&given).unwrap();
        assert_eq!(prepared.len(), 2);
        assert_eq!((prepared[0].0, prepared[0].1), (9, ConstantKind::Bool));
        assert_eq!((prepared[1].0, prepared[1].1), (0, ConstantKind::UInt));
        assert!(prepare_constants(&[]).unwrap().is_empty());
    }
    const KEY_VERSION: u64 = 0x1234_5678_9abc_def0;

    fn key(input: &PipelineInput<'_>, bounds_checked: bool) -> CacheKey {
        metal_cache_key(input, bounds_checked, KEY_VERSION).expect("an MSL input has a key")
    }

    fn with_options(source: &'static str, math_mode: MslMathMode) -> PipelineInput<'static> {
        PipelineInput::MslWithOptions {
            source,
            entry_point: "k",
            math_mode,
        }
    }

    /// 同じ入力で 2 回組み立てたキーが一致する（TASK-501 のポインタ）。
    #[test]
    fn cache_key_is_the_same_for_the_same_input() {
        let input = with_options("kernel void k() {}", MslMathMode::Safe);
        assert_eq!(key(&input, true), key(&input, true));
        assert_eq!(key(&input, false), key(&input, false));
    }

    /// 数学モード・ソース・`bounds_checked` を 1 つだけ変えると、対応する欄だけが変わる。
    #[test]
    fn changing_one_input_changes_only_its_field() {
        let base = key(
            &with_options("kernel void k() {}", MslMathMode::Safe),
            false,
        );
        let fast = key(
            &with_options("kernel void k() {}", MslMathMode::Fast),
            false,
        );
        assert_ne!(base.options_hash, fast.options_hash);
        assert_eq!(
            CacheKey {
                options_hash: base.options_hash,
                ..fast
            },
            base
        );
        let other = key(
            &with_options("kernel void k() { }", MslMathMode::Safe),
            false,
        );
        assert_ne!(base.source_hash, other.source_hash);
        assert_eq!(
            CacheKey {
                source_hash: base.source_hash,
                ..other
            },
            base
        );
        let checked = key(&with_options("kernel void k() {}", MslMathMode::Safe), true);
        assert!(checked.bounds_checked && !base.bounds_checked);
        assert_eq!(
            CacheKey {
                bounds_checked: false,
                ..checked
            },
            base
        );
    }

    /// 4 つの入力の欄の値（ART-83 のポインタ）。
    #[test]
    fn cache_key_fields_for_each_msl_input() {
        const SRC: &str = "kernel void k() {}";
        let constants = [
            FunctionConstant {
                index: 0,
                value: FunctionConstantValue::U32(7),
            },
            FunctionConstant {
                index: 3,
                value: FunctionConstantValue::Bool(true),
            },
        ];
        let cases: [(PipelineInput<'_>, &str, &str); 5] = [
            (
                PipelineInput::Msl {
                    source: SRC,
                    entry_point: "k",
                },
                "fast-math(default)",
                "entry=k",
            ),
            (
                PipelineInput::MslSpecialized {
                    source: SRC,
                    entry_point: "k",
                    function_constants: &constants,
                },
                "fast-math(default)",
                "entry=k;idx=0:U32(7);idx=3:Bool(true)",
            ),
            (
                with_options(SRC, MslMathMode::Safe),
                "math_mode=Safe",
                "entry=k",
            ),
            (
                PipelineInput::MslSpecializedWithOptions {
                    source: SRC,
                    entry_point: "k",
                    function_constants: &constants,
                    math_mode: MslMathMode::Safe,
                    float_functions: MslFloatFunctions::Precise,
                },
                "math_mode=Safe;float_functions=Precise",
                "entry=k;idx=0:U32(7);idx=3:Bool(true)",
            ),
            (
                PipelineInput::MslSpecializedWithOptions {
                    source: SRC,
                    entry_point: "k",
                    function_constants: &constants,
                    math_mode: MslMathMode::Fast,
                    float_functions: MslFloatFunctions::Fast,
                },
                "math_mode=Fast;float_functions=Fast",
                "entry=k;idx=0:U32(7);idx=3:Bool(true)",
            ),
        ];
        for (input, options, specialization) in &cases {
            let k = key(input, false);
            assert_eq!(k.compiler_id, "metal", "{input:?}");
            assert_eq!(k.target, "metal-msl", "{input:?}");
            assert_eq!(k.source_hash, sha256_u64(SRC.as_bytes()), "{input:?}");
            assert_eq!(k.options_hash, sha256_u64(options.as_bytes()), "{input:?}");
            assert_eq!(
                k.specialization_hash,
                sha256_u64(specialization.as_bytes()),
                "{input:?}"
            );
            assert_eq!(k.compiler_version, KEY_VERSION, "{input:?}");
            assert_eq!(k.driver_version, Some(KEY_VERSION), "{input:?}");
        }
    }

    /// `MslSpecializedWithOptions` のキーは、ほかの 3 つの入力のキーと別で、関数の精度だけを変えても別になる。
    #[test]
    fn specialized_with_options_keys_do_not_mix_with_other_inputs() {
        const SRC: &str = "kernel void k() {}";
        let swo = |math_mode, float_functions| {
            key(
                &PipelineInput::MslSpecializedWithOptions {
                    source: SRC,
                    entry_point: "k",
                    function_constants: &[],
                    math_mode,
                    float_functions,
                },
                false,
            )
        };
        let others = [
            key(
                &PipelineInput::Msl {
                    source: SRC,
                    entry_point: "k",
                },
                false,
            ),
            key(
                &PipelineInput::MslSpecialized {
                    source: SRC,
                    entry_point: "k",
                    function_constants: &[],
                },
                false,
            ),
            key(&with_options(SRC, MslMathMode::Safe), false),
            key(&with_options(SRC, MslMathMode::Fast), false),
        ];
        for mode in [MslMathMode::Safe, MslMathMode::Fast] {
            let precise = swo(mode, MslFloatFunctions::Precise);
            let fast = swo(mode, MslFloatFunctions::Fast);
            assert_ne!(precise, fast, "{mode:?}");
            for other in &others {
                assert_ne!(&precise, other, "{mode:?}");
                assert_ne!(&fast, other, "{mode:?}");
            }
        }
    }

    /// MSL でない入力にはキーが無い。
    #[test]
    fn non_msl_inputs_have_no_cache_key() {
        let inputs = [
            PipelineInput::SpirV {
                words: &[],
                entry_point: "k",
            },
            PipelineInput::CudaC {
                source: "",
                entry_point: "k",
            },
            PipelineInput::Ptx {
                source: "",
                entry_point: "k",
            },
            PipelineInput::Dxil {
                bytes: &[],
                entry_point: "k",
            },
            PipelineInput::Wgsl {
                source: "",
                entry_point: "k",
            },
        ];
        for input in &inputs {
            assert!(
                metal_cache_key(input, false, KEY_VERSION).is_none(),
                "{input:?}"
            );
        }
    }

    /// 版の欄の値は、OS の版の文字列と device 名を NUL でつないだ文字列の `sha256_u64`。
    #[test]
    fn cache_key_version_joins_os_version_and_device_name_with_nul() {
        assert_eq!(cache_key_version("os", "gpu"), sha256_u64(b"os\0gpu"));
        assert_ne!(
            cache_key_version("os", "gpu"),
            cache_key_version("os2", "gpu")
        );
        assert_ne!(
            cache_key_version("os", "gpu"),
            cache_key_version("os", "gpu2")
        );
        assert_ne!(cache_key_version("a", "bc"), cache_key_version("ab", "c"));
    }
}

#[cfg(all(test, feature = "fault-injection"))]
mod lost_state_tests {
    //! 模擬のロストの状態 `LostState` の固定（device に触らない。どの OS でも流れる。TASK-2932）。
    use super::*;
    use std::sync::Arc;

    type Seen = Arc<Mutex<Vec<DeviceLostReason>>>;

    fn recorder(seen: &Seen) -> LostCallback {
        let seen = Arc::clone(seen);
        Box::new(move |reason| seen.lock().unwrap().push(reason))
    }

    fn all_reasons() -> [DeviceLostReason; 4] {
        [
            DeviceLostReason::Destroyed,
            DeviceLostReason::Unknown,
            DeviceLostReason::ValidationError("v".into()),
            DeviceLostReason::Unrecoverable("u".into()),
        ]
    }

    #[test]
    fn the_callback_runs_once_and_a_second_lose_is_ignored() {
        let state = LostState::default();
        let (generation, source) = (AtomicU64::new(1), AtomicU64::new(5));
        let seen = Seen::default();
        state.set_callback(recorder(&seen));
        assert!(!state.is_lost());

        assert_eq!(
            state.lose(DeviceLostReason::Unknown, &generation, &source),
            Ok(true)
        );
        assert_eq!(
            state.lose(DeviceLostReason::Destroyed, &generation, &source),
            Ok(false)
        );
        assert!(state.is_lost());
        assert_eq!(*seen.lock().unwrap(), [DeviceLostReason::Unknown]);
    }

    #[test]
    fn every_reason_is_passed_through_unchanged() {
        for reason in all_reasons() {
            let state = LostState::default();
            let (generation, source) = (AtomicU64::new(1), AtomicU64::new(2));
            let seen = Seen::default();
            state.set_callback(recorder(&seen));
            state.lose(reason.clone(), &generation, &source).unwrap();
            assert_eq!(*seen.lock().unwrap(), [reason]);
        }
    }

    #[test]
    fn the_generation_is_taken_once() {
        let state = LostState::default();
        let (generation, source) = (AtomicU64::new(1), AtomicU64::new(7));
        state
            .lose(DeviceLostReason::Unknown, &generation, &source)
            .unwrap();
        assert_eq!(generation.load(Ordering::Relaxed), 7);
        assert_eq!(source.load(Ordering::Relaxed), 8);
        state
            .lose(DeviceLostReason::Destroyed, &generation, &source)
            .unwrap();
        assert_eq!(generation.load(Ordering::Relaxed), 7);
        assert_eq!(source.load(Ordering::Relaxed), 8);
    }

    #[test]
    fn an_exhausted_source_leaves_the_state_unchanged() {
        let state = LostState::default();
        let (generation, source) = (AtomicU64::new(1), AtomicU64::new(u64::MAX));
        let seen = Seen::default();
        state.set_callback(recorder(&seen));
        assert_eq!(
            state.lose(DeviceLostReason::Unknown, &generation, &source),
            Err(GenerationExhausted)
        );
        assert!(!state.is_lost());
        assert_eq!(generation.load(Ordering::Relaxed), 1);
        assert_eq!(source.load(Ordering::Relaxed), u64::MAX);
        assert!(seen.lock().unwrap().is_empty());

        // 源が戻れば、保持されたままのコールバックでロストできる。
        source.store(3, Ordering::Relaxed);
        state
            .lose(DeviceLostReason::Destroyed, &generation, &source)
            .unwrap();
        assert_eq!(*seen.lock().unwrap(), [DeviceLostReason::Destroyed]);
        assert_eq!(generation.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn a_callback_registered_after_the_loss_is_kept_but_not_called() {
        let state = LostState::default();
        let (generation, source) = (AtomicU64::new(1), AtomicU64::new(2));
        state
            .lose(DeviceLostReason::Unknown, &generation, &source)
            .unwrap();
        let seen = Seen::default();
        state.set_callback(recorder(&seen));
        state
            .lose(DeviceLostReason::Destroyed, &generation, &source)
            .unwrap();
        assert!(seen.lock().unwrap().is_empty());
        assert!(state.lock().callback.is_some());
    }

    #[test]
    fn a_loss_without_a_callback_then_a_registration_never_calls_it() {
        let state = LostState::default();
        let (generation, source) = (AtomicU64::new(1), AtomicU64::new(2));
        state
            .lose(DeviceLostReason::Unknown, &generation, &source)
            .unwrap();
        let seen = Seen::default();
        state.set_callback(recorder(&seen));
        state
            .lose(DeviceLostReason::Destroyed, &generation, &source)
            .unwrap();
        assert!(seen.lock().unwrap().is_empty());
    }

    /// 捨てられるときに、同じ状態の `set_callback` と `lose` を呼ぶ値。
    struct ReentrantDrop {
        state: Arc<LostState>,
        generation: Arc<AtomicU64>,
        source: Arc<AtomicU64>,
        done: Arc<Mutex<bool>>,
    }

    impl Drop for ReentrantDrop {
        fn drop(&mut self) {
            self.state.set_callback(Box::new(|_| {}));
            let _ = self
                .state
                .lose(DeviceLostReason::Unknown, &self.generation, &self.source);
            *self.done.lock().unwrap() = true;
        }
    }

    #[test]
    fn dropping_an_overwritten_callback_may_reenter_without_deadlock() {
        let state = Arc::new(LostState::default());
        let generation = Arc::new(AtomicU64::new(1));
        let source = Arc::new(AtomicU64::new(2));
        let done = Arc::new(Mutex::new(false));
        let guard = ReentrantDrop {
            state: Arc::clone(&state),
            generation: Arc::clone(&generation),
            source: Arc::clone(&source),
            done: Arc::clone(&done),
        };
        state.set_callback(Box::new(move |_| {
            let _keep = &guard;
        }));
        // 上書きで古いコールバックが捨てられ、その `Drop` が再入する。錠を握ったままなら詰まる。
        state.set_callback(Box::new(|_| {}));
        assert!(*done.lock().unwrap());
        assert!(state.is_lost());
    }

    #[test]
    fn concurrent_loses_call_the_callback_once_and_advance_the_source_once() {
        const THREADS: usize = 8;
        let state = LostState::default();
        let (generation, source) = (AtomicU64::new(1), AtomicU64::new(10));
        let seen = Seen::default();
        state.set_callback(recorder(&seen));
        let barrier = std::sync::Barrier::new(THREADS);
        std::thread::scope(|scope| {
            for _ in 0..THREADS {
                scope.spawn(|| {
                    barrier.wait();
                    state
                        .lose(DeviceLostReason::Unknown, &generation, &source)
                        .unwrap();
                });
            }
        });
        assert_eq!(seen.lock().unwrap().len(), 1);
        assert_eq!(source.load(Ordering::Relaxed), 11);
        assert_eq!(generation.load(Ordering::Relaxed), 10);
    }

    #[test]
    fn a_later_registration_replaces_the_earlier_one() {
        let state = LostState::default();
        let (generation, source) = (AtomicU64::new(1), AtomicU64::new(2));
        let (first, second) = (Seen::default(), Seen::default());
        state.set_callback(recorder(&first));
        state.set_callback(recorder(&second));
        state
            .lose(DeviceLostReason::Unknown, &generation, &source)
            .unwrap();
        assert!(first.lock().unwrap().is_empty());
        assert_eq!(second.lock().unwrap().len(), 1);
    }

    #[test]
    fn the_callback_runs_outside_the_lock() {
        let state = Arc::new(LostState::default());
        let (generation, source) = (Arc::new(AtomicU64::new(1)), Arc::new(AtomicU64::new(2)));
        let reentered = Arc::new(Mutex::new(None));
        {
            let (state2, generation2, source2, reentered) = (
                Arc::clone(&state),
                Arc::clone(&generation),
                Arc::clone(&source),
                Arc::clone(&reentered),
            );
            state.set_callback(Box::new(move |_| {
                // 錠を持ったまま呼ばれていれば、ここで詰まる。
                state2.set_callback(Box::new(|_| {}));
                let again = state2.lose(DeviceLostReason::Unknown, &generation2, &source2);
                *reentered.lock().unwrap() = Some((state2.is_lost(), again));
            }));
        }
        state
            .lose(DeviceLostReason::Destroyed, &generation, &source)
            .unwrap();
        assert_eq!(*reentered.lock().unwrap(), Some((true, Ok(false))));
    }

    #[test]
    fn a_poisoned_lock_does_not_panic() {
        let state = Arc::new(LostState::default());
        let poisoner = Arc::clone(&state);
        let _ = std::thread::spawn(move || {
            let _guard = poisoner.inner.lock().unwrap();
            panic!("poison the lock");
        })
        .join();
        let (generation, source) = (AtomicU64::new(1), AtomicU64::new(2));
        let seen = Seen::default();
        state.set_callback(recorder(&seen));
        state
            .lose(DeviceLostReason::Unknown, &generation, &source)
            .unwrap();
        assert_eq!(seen.lock().unwrap().len(), 1);
    }
}

#[cfg(test)]
mod dispatch_checked_tests {
    use super::*;
    use fandhe_silicon_core::execution::DType;
    use std::cell::{Cell, RefCell};

    fn spec<'a>(slot: u32, dtype: DType, shape: &'a [u64]) -> OperandSpec<'a> {
        OperandSpec {
            slot,
            dtype,
            shape,
            stride: &[],
            alignment: 0,
        }
    }

    fn binding(slot: u32) -> OperandBinding {
        OperandBinding { slot, offset: 0 }
    }

    fn starts_with(r: Result<(), ContractError>, prefix: &str) -> bool {
        matches!(r, Err(ContractError::InvalidArgument(m)) if m.starts_with(prefix))
    }

    #[test]
    fn handle_record_returns_the_length_when_generation_and_size_match() {
        assert_eq!(check_handle_record(3, 16, 3, 16), Ok(16));
    }

    #[test]
    fn handle_record_rejects_a_different_generation() {
        assert!(matches!(
            check_handle_record(2, 16, 3, 16),
            Err(ContractError::InvalidArgument(_))
        ));
    }

    #[test]
    fn handle_record_rejects_a_larger_size() {
        assert!(matches!(
            check_handle_record(3, 400, 3, 16),
            Err(ContractError::InvalidArgument(_))
        ));
    }

    #[test]
    fn handle_record_rejects_a_smaller_size() {
        assert!(matches!(
            check_handle_record(3, 4, 3, 16),
            Err(ContractError::InvalidArgument(_))
        ));
    }

    /// BIND-50: 長さ 0 の次元は、束縛を表で引く前に C8 で拒む。
    #[test]
    fn zero_length_dimension_is_rejected_before_any_lookup() {
        let shape_a = [0u64, 4];
        let shape_b = [4u64, 4];
        let shape_c = [0u64, 4];
        let declared = [
            spec(0, DType::F32, &shape_a),
            spec(1, DType::F32, &shape_b),
            spec(2, DType::F32, &shape_c),
        ];
        let bindings = [binding(0), binding(1), binding(2)];
        let calls = Cell::new(0u32);
        let result = check_declared_operands(
            &declared,
            &bindings,
            &[],
            |_| {
                calls.set(calls.get() + 1);
                Err(ContractError::DeviceMismatch)
            },
            1 << 20,
        );
        assert!(starts_with(result, "C8:"));
        assert_eq!(calls.get(), 0);
    }

    /// BIND-52: 実際の長さを超える宣言は C4 で拒む。
    #[test]
    fn declaration_larger_than_the_buffer_length_is_rejected_by_c4() {
        let shape = [100u64];
        let declared = [spec(0, DType::F32, &shape)];
        let result = check_declared_operands(&declared, &[binding(0)], &[], |_| Ok(16), 1 << 20);
        assert!(starts_with(result, "C4:"));
    }

    /// BIND-61: 上限を 1 バイト超える宣言は C8、上限ちょうどは通る。
    #[test]
    fn declaration_one_byte_over_the_limit_is_rejected_and_the_limit_passes() {
        let limit: u64 = 1 << 20;
        let over = [limit.checked_add(1).unwrap()];
        let exact = [limit];
        let result =
            check_declared_operands(&[spec(0, DType::Bool, &over)], &[], &[], |_| Ok(0), limit);
        assert!(starts_with(result, "C8:"));
        let result =
            check_declared_operands(&[spec(0, DType::Bool, &exact)], &[], &[], |_| Ok(0), limit);
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn lookup_errors_are_returned_as_they_are() {
        let shape = [4u64];
        let declared = [spec(0, DType::F32, &shape)];
        let result = check_declared_operands(
            &declared,
            &[binding(0)],
            &[],
            |_| Err(ContractError::DeviceMismatch),
            1 << 20,
        );
        assert_eq!(result, Err(ContractError::DeviceMismatch));
        let invalid = ContractError::InvalidArgument("lookup".into());
        let result = check_declared_operands(
            &declared,
            &[binding(0)],
            &[],
            |_| Err(invalid.clone()),
            1 << 20,
        );
        assert_eq!(result, Err(invalid));
    }

    #[test]
    fn every_binding_is_looked_up_once_when_the_declaration_passes() {
        let shape = [4u64];
        let declared = [spec(0, DType::F32, &shape), spec(1, DType::F32, &shape)];
        let calls = Cell::new(0u32);
        let result = check_declared_operands(
            &declared,
            &[binding(0), binding(1)],
            &[],
            |_| {
                calls.set(calls.get() + 1);
                Ok(16)
            },
            1 << 20,
        );
        assert_eq!(result, Ok(()));
        assert_eq!(calls.get(), 2);
    }

    fn aligned_spec<'a>(
        slot: u32,
        dtype: DType,
        shape: &'a [u64],
        alignment: u32,
    ) -> OperandSpec<'a> {
        OperandSpec {
            slot,
            dtype,
            shape,
            stride: &[],
            alignment,
        }
    }

    fn binding_at(slot: u32, offset: u64) -> OperandBinding {
        OperandBinding { slot, offset }
    }

    /// 容量の問い合わせで受けた位置を記録しながら、位置ごとの結果を返す。
    fn run(
        declared: &[OperandSpec<'_>],
        bindings: &[OperandBinding],
        consts: &[OperandConstant<'_>],
        limit: u64,
        answer: impl Fn(usize) -> Result<u64, ContractError>,
    ) -> (Result<(), ContractError>, Vec<usize>) {
        let seen: RefCell<Vec<usize>> = RefCell::new(Vec::new());
        let result = check_declared_operands(
            declared,
            bindings,
            consts,
            |pos| {
                seen.borrow_mut().push(pos);
                answer(pos)
            },
            limit,
        );
        (result, seen.into_inner())
    }

    #[test]
    fn misaligned_offset_is_rejected_by_c5_after_every_binding_is_looked_up() {
        let shape = [4u64];
        let declared = [
            aligned_spec(0, DType::F32, &shape, 0),
            aligned_spec(1, DType::F32, &shape, 16),
        ];
        let bindings = [binding_at(0, 0), binding_at(1, 4)];
        let (result, seen) = run(&declared, &bindings, &[], 1 << 20, |_| Ok(64));
        assert!(starts_with(result, "C5:"));
        assert_eq!(seen, vec![0, 1]);
    }

    #[test]
    fn constant_size_mismatch_is_rejected_by_c6() {
        let shape = [4u64];
        let shape_u32 = [3u64];
        let declared = [
            spec(0, DType::F32, &shape),
            spec(1, DType::F32, &shape),
            spec(2, DType::U32, &shape_u32),
        ];
        let bindings = [binding_at(0, 0), binding_at(1, 0)];
        let consts = [OperandConstant {
            slot: 2,
            bytes: &[0u8; 8],
        }];
        let (result, seen) = run(&declared, &bindings, &consts, 1 << 20, |_| Ok(16));
        assert!(starts_with(result, "C6:"));
        assert_eq!(seen, vec![0, 1]);
    }

    #[test]
    fn slot_mistakes_are_rejected_by_c7_without_any_lookup() {
        let shape = [4u64];
        let shape_one = [1u64];
        let four = [0u8; 4];
        let nine = [OperandConstant {
            slot: 9,
            bytes: &four,
        }];
        let same = [OperandConstant {
            slot: 0,
            bytes: &four,
        }];
        let one = [spec(0, DType::F32, &shape)];
        let one_u32 = [spec(0, DType::U32, &shape_one)];
        let cases: [(
            &[OperandSpec<'_>],
            Vec<OperandBinding>,
            &[OperandConstant<'_>],
        ); 4] = [
            (&one, vec![binding_at(0, 0), binding_at(5, 0)], &[]),
            (&one, vec![binding_at(0, 0), binding_at(0, 0)], &[]),
            (&one, vec![binding_at(0, 0)], &nine),
            (&one_u32, vec![binding_at(0, 0)], &same),
        ];
        for (declared, bindings, consts) in cases {
            let (result, seen) = run(declared, &bindings, consts, 1 << 20, |_| {
                Err(ContractError::DeviceMismatch)
            });
            assert!(starts_with(result, "C7:"));
            assert!(seen.is_empty());
        }
    }

    #[test]
    fn overflowing_declaration_is_rejected_by_c8_without_any_lookup() {
        let shape = [u64::MAX];
        let declared = [spec(0, DType::F32, &shape)];
        let (result, seen) = run(&declared, &[binding_at(0, 0)], &[], 1 << 20, |_| {
            Err(ContractError::DeviceMismatch)
        });
        assert!(starts_with(result, "C8:"));
        assert!(seen.is_empty());
    }

    #[test]
    fn c8_comes_before_a_lookup_error() {
        let zero = [0u64, 4];
        let shape = [4u64];
        let declared = [spec(0, DType::F32, &shape), spec(1, DType::F32, &zero)];
        let bindings = [binding_at(0, 0), binding_at(1, 0)];
        let (result, seen) = run(&declared, &bindings, &[], 1 << 20, |_| {
            Err(ContractError::InvalidArgument("lookup".into()))
        });
        assert!(starts_with(result, "C8:"));
        assert!(seen.is_empty());
    }

    #[test]
    fn c7_comes_before_a_lookup_error() {
        let shape = [4u64];
        let declared = [spec(0, DType::F32, &shape)];
        let bindings = [binding_at(0, 0), binding_at(5, 0)];
        let (result, seen) = run(&declared, &bindings, &[], 1 << 20, |_| {
            Err(ContractError::InvalidArgument("lookup".into()))
        });
        assert!(starts_with(result, "C7:"));
        assert!(seen.is_empty());
    }

    #[test]
    fn c4_stops_the_lookups_at_the_first_violation() {
        let shape = [100u64];
        let declared = [spec(0, DType::F32, &shape), spec(1, DType::F32, &shape)];
        let bindings = [binding_at(0, 0), binding_at(1, 0)];
        let (result, seen) = run(&declared, &bindings, &[], 1 << 20, |pos| {
            if pos == 0 {
                Ok(16)
            } else {
                Err(ContractError::DeviceMismatch)
            }
        });
        assert!(matches!(
            &result,
            Err(ContractError::InvalidArgument(m)) if m.starts_with("C4:") && m.contains("slot 0")
        ));
        assert_eq!(seen, vec![0]);
    }

    #[test]
    fn the_first_lookup_error_wins_over_a_later_c4() {
        let shape = [100u64];
        let declared = [spec(0, DType::F32, &shape), spec(1, DType::F32, &shape)];
        let bindings = [binding_at(0, 0), binding_at(1, 0)];
        let invalid = ContractError::InvalidArgument("lookup".into());
        let (result, _) = run(&declared, &bindings, &[], 1 << 20, |pos| {
            if pos == 0 {
                Err(ContractError::InvalidArgument("lookup".into()))
            } else {
                Ok(16)
            }
        });
        assert_eq!(result, Err(invalid));
    }

    #[test]
    fn a_lookup_error_wins_over_c5() {
        let shape = [4u64];
        let declared = [aligned_spec(0, DType::F32, &shape, 16)];
        let (result, _) = run(&declared, &[binding_at(0, 4)], &[], 1 << 20, |_| {
            Err(ContractError::DeviceMismatch)
        });
        assert_eq!(result, Err(ContractError::DeviceMismatch));
    }

    #[test]
    fn offset_counts_against_the_capacity() {
        let shape = [4u64];
        let declared = [spec(0, DType::F32, &shape)];
        let (result, _) = run(&declared, &[binding_at(0, 16)], &[], 1 << 20, |_| Ok(32));
        assert_eq!(result, Ok(()));
        let (result, _) = run(&declared, &[binding_at(0, 20)], &[], 1 << 20, |_| Ok(32));
        assert!(starts_with(result, "C4:"));
    }

    #[test]
    fn lookups_follow_the_binding_order_not_the_slot_order() {
        let shape = [4u64];
        let declared = [spec(1, DType::F32, &shape), spec(3, DType::F32, &shape)];
        let bindings = [binding_at(3, 0), binding_at(1, 0)];
        let (result, seen) = run(&declared, &bindings, &[], 1 << 20, |_| Ok(16));
        assert_eq!(result, Ok(()));
        assert_eq!(seen, vec![0, 1]);
    }

    #[test]
    fn extreme_values_return_a_result_without_panicking() {
        let big = [u64::MAX];
        let declared = [spec(0, DType::Bool, &big)];
        let (result, _) = run(&declared, &[binding_at(0, 0)], &[], u64::MAX, |_| {
            Ok(u64::MAX)
        });
        assert_eq!(result, Ok(()));
        let (result, _) = run(&declared, &[binding_at(0, u64::MAX)], &[], u64::MAX, |_| {
            Ok(u64::MAX)
        });
        assert!(starts_with(result, "C4:"));
        let declared = [spec(0, DType::F32, &big)];
        let (result, _) = run(&declared, &[binding_at(0, 0)], &[], u64::MAX, |_| {
            Ok(u64::MAX)
        });
        assert!(starts_with(result, "C8:"));
    }
}

#[cfg(test)]
mod reflection_value_tests {
    use super::*;

    /// ART-89: `DType` の 5 通りが reflection の型に写る。
    #[test]
    fn expected_data_type_maps_five_dtypes() {
        assert_eq!(
            expected_data_type(DType::F32),
            Some(ReflectedDataType::Float)
        );
        assert_eq!(
            expected_data_type(DType::F16),
            Some(ReflectedDataType::Half)
        );
        assert_eq!(
            expected_data_type(DType::U32),
            Some(ReflectedDataType::UInt)
        );
        assert_eq!(expected_data_type(DType::I32), Some(ReflectedDataType::Int));
        assert_eq!(
            expected_data_type(DType::Bool),
            Some(ReflectedDataType::Bool)
        );
    }

    /// 対応表の値と要素のバイト数の組を並べ、`Struct` と `Other` はどの `DType` にも当たらない。
    #[test]
    fn mapped_types_pair_with_byte_size_and_exclude_struct_other() {
        let table = [
            (DType::F32, ReflectedDataType::Float, 4),
            (DType::F16, ReflectedDataType::Half, 2),
            (DType::U32, ReflectedDataType::UInt, 4),
            (DType::I32, ReflectedDataType::Int, 4),
            (DType::Bool, ReflectedDataType::Bool, 1),
        ];
        for (dtype, expected, bytes) in table {
            assert_eq!(expected_data_type(dtype), Some(expected));
            assert_eq!(dtype.byte_size(), bytes);
            assert_ne!(expected, ReflectedDataType::Struct);
            assert_ne!(expected, ReflectedDataType::Other(0));
        }
    }

    /// バイト長 4 と `Float` は `Bytes(4)`・`Float` で、slot は index と同じ。
    #[test]
    fn classify_keeps_slot_size_and_type() {
        let got = classify_reflected_buffer(3, 4, ReflectedDataType::Float);
        assert_eq!(
            got,
            Ok(ReflectedBuffer {
                slot: 3,
                size: ReflectedSize::Bytes(4),
                data_type: ReflectedDataType::Float,
            })
        );
    }

    /// バイト長 0 は `Unchecked` で、型はそのまま。
    #[test]
    fn classify_zero_size_is_unchecked() {
        let got = classify_reflected_buffer(0, 0, ReflectedDataType::Half);
        assert_eq!(
            got,
            Ok(ReflectedBuffer {
                slot: 0,
                size: ReflectedSize::Unchecked,
                data_type: ReflectedDataType::Half,
            })
        );
    }

    /// `Struct` は型のまま持ち、バイト長 12 は `Bytes(12)`。
    #[test]
    fn classify_struct_keeps_size() {
        let got = classify_reflected_buffer(1, 12, ReflectedDataType::Struct);
        assert_eq!(
            got,
            Ok(ReflectedBuffer {
                slot: 1,
                size: ReflectedSize::Bytes(12),
                data_type: ReflectedDataType::Struct,
            })
        );
    }

    /// 対応表の外の型は `Other` の値のまま持つ。
    #[test]
    fn classify_other_type_is_kept() {
        let got = classify_reflected_buffer(2, 8, ReflectedDataType::Other(7));
        assert_eq!(got.map(|b| b.data_type), Ok(ReflectedDataType::Other(7)));
    }

    /// index が `u32` に収まらないと `BackendFailure`。
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn classify_index_over_u32_fails() {
        let over = usize::try_from(u64::from(u32::MAX) + 1).unwrap_or(usize::MAX);
        let got = classify_reflected_buffer(over, 4, ReflectedDataType::Float);
        assert!(matches!(
            got,
            Err(ContractError::BackendFailure {
                op: "create_pipeline",
                backend: BackendKind::Metal,
                ..
            })
        ));
    }

    /// index が `u32::MAX` ちょうどなら通る。
    #[test]
    fn classify_index_u32_max_passes() {
        let max = usize::try_from(u32::MAX).unwrap_or(usize::MAX);
        let got = classify_reflected_buffer(max, 4, ReflectedDataType::Int);
        assert_eq!(got.map(|b| b.slot), Ok(u32::MAX));
    }

    /// `classify_reflected_buffer` の結果で `PipelineReflection` を組み、両方の欄を読める。
    #[test]
    fn pipeline_reflection_holds_count_and_buffers() {
        let buffers: Vec<ReflectedBuffer> = [
            classify_reflected_buffer(0, 4, ReflectedDataType::Float),
            classify_reflected_buffer(2, 0, ReflectedDataType::Struct),
        ]
        .into_iter()
        .filter_map(Result::ok)
        .collect();
        let reflection = PipelineReflection {
            argument_count: 3,
            buffers: buffers.clone(),
        };
        assert_eq!(reflection.argument_count, 3);
        assert_eq!(reflection.buffers, buffers);
        assert_eq!(reflection.buffers.len(), 2);
    }
}

#[cfg(all(test, feature = "artifact-inspection"))]
mod artifact_path_tests {
    use super::*;
    use std::path::{Path, PathBuf};

    /// 試験ごとの一時ディレクトリ。落ちたときも `Drop` で消す。
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "fandhe-silicon-artifact-path-{}-{name}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn is_invalid(r: Result<&str, ContractError>) -> bool {
        matches!(r, Err(ContractError::InvalidArgument(_)))
    }

    fn is_rejected(r: Result<Vec<u8>, ContractError>) -> bool {
        matches!(
            r,
            Err(ContractError::ArtifactRejected {
                kind: "binary-archive",
                ..
            })
        )
    }

    #[test]
    fn path_str_accepts_absolute_utf8() {
        let p = Path::new("/tmp/fandhe-silicon-artifact.bin");
        assert_eq!(
            artifact_path_str(p).unwrap(),
            "/tmp/fandhe-silicon-artifact.bin"
        );
    }

    #[test]
    fn path_str_rejects_relative_and_empty() {
        assert!(is_invalid(artifact_path_str(Path::new(
            "relative/archive.bin"
        ))));
        assert!(is_invalid(artifact_path_str(Path::new(""))));
    }

    #[cfg(unix)]
    #[test]
    fn path_str_rejects_non_utf8() {
        use std::os::unix::ffi::OsStrExt as _;
        let p = Path::new(std::ffi::OsStr::from_bytes(b"/tmp/\xff"));
        assert!(is_invalid(artifact_path_str(p)));
    }

    #[test]
    fn read_returns_bytes_with_default_limit() {
        assert_eq!(ARTIFACT_READ_LIMIT, 1 << 28);
        let dir = TempDir::new("read-ok");
        let path = dir.0.join("a.bin");
        std::fs::write(&path, [1u8, 2, 3]).unwrap();
        assert_eq!(
            read_written_artifact(&path, ARTIFACT_READ_LIMIT).unwrap(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn read_enforces_limit_boundary() {
        let dir = TempDir::new("read-limit");
        let path = dir.0.join("a.bin");
        std::fs::write(&path, [1u8, 2, 3, 4]).unwrap();
        assert_eq!(read_written_artifact(&path, 4).unwrap().len(), 4);
        assert!(is_rejected(read_written_artifact(&path, 3)));
    }

    #[test]
    fn read_rejects_empty_missing_and_directory() {
        let dir = TempDir::new("read-bad");
        let empty = dir.0.join("empty.bin");
        std::fs::write(&empty, []).unwrap();
        assert!(is_rejected(read_written_artifact(
            &empty,
            ARTIFACT_READ_LIMIT
        )));
        let missing = dir.0.join("missing.bin");
        assert!(is_rejected(read_written_artifact(
            &missing,
            ARTIFACT_READ_LIMIT
        )));
        let sub = dir.0.join("sub");
        std::fs::create_dir(&sub).unwrap();
        assert!(is_rejected(read_written_artifact(
            &sub,
            ARTIFACT_READ_LIMIT
        )));
    }

    #[cfg(all(unix, any(target_os = "linux", target_os = "macos")))]
    #[test]
    fn read_open_does_not_follow_symlink() {
        // 検査後に通常のファイルがリンクへ差し替わった場合を、open だけで拒否することを確かめる。
        use std::os::unix::fs::OpenOptionsExt as _;
        let dir = TempDir::new("read-nofollow");
        let target = dir.0.join("target.bin");
        std::fs::write(&target, [1u8]).unwrap();
        let link = dir.0.join("link.bin");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        #[cfg(target_os = "macos")]
        options.custom_flags(0x4 | 0x100);
        #[cfg(all(target_os = "linux", any(target_arch = "arm", target_arch = "aarch64")))]
        options.custom_flags(0o4000 | 0o100_000);
        #[cfg(all(
            target_os = "linux",
            not(any(target_arch = "arm", target_arch = "aarch64"))
        ))]
        options.custom_flags(0o4000 | 0o400_000);
        assert!(options.open(&link).is_err());
        assert!(options.open(&target).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn read_rejects_symlink() {
        let dir = TempDir::new("read-link");
        let target = dir.0.join("target.bin");
        std::fs::write(&target, [1u8, 2, 3]).unwrap();
        let link = dir.0.join("link.bin");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(is_rejected(read_written_artifact(
            &link,
            ARTIFACT_READ_LIMIT
        )));
        assert_eq!(
            read_written_artifact(&target, ARTIFACT_READ_LIMIT).unwrap(),
            vec![1, 2, 3]
        );
    }
}
