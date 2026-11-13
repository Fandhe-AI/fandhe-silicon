//! バッチの積和と部分 Top-k の device の要る試験と、その純粋な部品（TASK-485 のポインタ）。
//!
//! 契約の crate の単体試験のビルドの中にだけ置く。クエリ×文書の内積（得点）を 1 段目、
//! クエリごとの部分 Top-k を 2 段目として Metal の device に投入し、CPU の参照と比べる。
//! カーネルはこのファイルの文字列の定数で、`Msl` として作る。入力の生成・得点の参照・候補の併合・
//! 起動の形はどの OS でも流れる純粋な部分で、device の部分は macOS で `native-blocking-wait` を
//! 有効にしたビルドの `device` module にあり、`#[ignore]` 付きで専用の実機の回でだけ流す。
//! エラーは固定の英語の文言で、値を含めない。

use std::fmt;

use fandhe_silicon_core::parity::gemm_inputs::SplitMix64;
use fandhe_silicon_core::parity::topk::TopKEntry;

/// 得点の段の入口名。
const SCORES_ENTRY: &str = "scores_f32";
/// 部分 Top-k の段の入口名。
const TOPK_ENTRY: &str = "topk_partial_f32";

/// 入力の生成の seed。
const INPUT_SEED: u64 = 0x5EED_0B47_0000_0001;
/// 取り出す上位の件数。
const TOP_K: usize = 10;
/// 部分 Top-k の局所の配列の上限。MSL の `MAX_K` と同じ値。
const MAX_K: usize = 16;
/// 部分 Top-k の threadgroup あたりのスレッド数。
const TOPK_THREADS: u32 = 256;
/// 得点の threadgroup の一辺のスレッド数。
const SCORE_TILE: u32 = 16;

/// 部分 Top-k が埋まらない枠に書く番号。
const SENTINEL_INDEX: u32 = u32::MAX;

/// 試験の規模。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Scale {
    /// 文書の数。
    n_docs: usize,
    /// 次元。
    dim: usize,
    /// クエリの数。
    n_queries: usize,
}

/// 小さい方の規模。
const SMALL: Scale = Scale {
    n_docs: 20_000,
    dim: 128,
    n_queries: 64,
};
/// 大きい方の規模。
const LARGE: Scale = Scale {
    n_docs: 100_000,
    dim: 128,
    n_queries: 64,
};

/// 部品の失敗。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BatchError {
    /// 要素数・バイト数・添字が表せる範囲を超えた。
    SizeOverflow,
    /// 規模に 0 がある。
    EmptyScale,
    /// 列の長さが期待と違う。
    LengthMismatch,
    /// `k` が 0 か上限を超えた。
    InvalidK,
    /// バイト列の長さが 4 の倍数でない。
    InvalidBytes,
}

impl fmt::Display for BatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            Self::SizeOverflow => "size exceeds the supported range",
            Self::EmptyScale => "scale has a zero dimension",
            Self::LengthMismatch => "sequence length does not match",
            Self::InvalidK => "k is outside the supported range",
            Self::InvalidBytes => "byte length is not a multiple of four",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for BatchError {}

impl Scale {
    /// 規模の各積が `u32` の添字に収まることを確かめる。0 の規模と、積のあふれは `Err`。
    fn check(&self, k: usize) -> Result<(), BatchError> {
        if self.n_docs == 0 || self.dim == 0 || self.n_queries == 0 {
            return Err(BatchError::EmptyScale);
        }
        check_k(k)?;
        let products = [
            self.n_queries.checked_mul(self.dim),
            self.n_docs.checked_mul(self.dim),
            self.n_queries.checked_mul(self.n_docs),
            self.n_queries
                .checked_mul(TOPK_THREADS as usize)
                .and_then(|v| v.checked_mul(k)),
        ];
        for p in products {
            let p = p.ok_or(BatchError::SizeOverflow)?;
            u32::try_from(p).map_err(|_| BatchError::SizeOverflow)?;
        }
        Ok(())
    }

    fn query_len(&self) -> Result<usize, BatchError> {
        self.n_queries
            .checked_mul(self.dim)
            .ok_or(BatchError::SizeOverflow)
    }

    fn doc_len(&self) -> Result<usize, BatchError> {
        self.n_docs
            .checked_mul(self.dim)
            .ok_or(BatchError::SizeOverflow)
    }

    #[cfg_attr(
        not(all(target_os = "macos", feature = "native-blocking-wait")),
        allow(dead_code)
    )]
    fn score_len(&self) -> Result<usize, BatchError> {
        self.n_queries
            .checked_mul(self.n_docs)
            .ok_or(BatchError::SizeOverflow)
    }

    /// 全クエリの候補の数（クエリ × 256 スレッド × `k`）。
    #[cfg_attr(
        not(all(target_os = "macos", feature = "native-blocking-wait")),
        allow(dead_code)
    )]
    fn candidate_len(&self, k: usize) -> Result<usize, BatchError> {
        (TOPK_THREADS as usize)
            .checked_mul(k)
            .and_then(|v| v.checked_mul(self.n_queries))
            .ok_or(BatchError::SizeOverflow)
    }
}

fn check_k(k: usize) -> Result<(), BatchError> {
    if k == 0 || k > MAX_K {
        return Err(BatchError::InvalidK);
    }
    Ok(())
}

/// クエリ（Q×D）と文書（N×D）。いずれも行優先。
#[derive(Debug, Clone, PartialEq)]
struct BatchInputs {
    queries: Vec<f32>,
    docs: Vec<f32>,
}

/// 同じ流れから、クエリ、続けて文書を作る。
fn generate_inputs(scale: Scale, seed: u64) -> Result<BatchInputs, BatchError> {
    let q_len = scale.query_len()?;
    let d_len = scale.doc_len()?;
    let mut rng = SplitMix64::new(seed);
    let queries: Vec<f32> = (0..q_len).map(|_| rng.next_signed_unit_f32()).collect();
    let docs: Vec<f32> = (0..d_len).map(|_| rng.next_signed_unit_f32()).collect();
    Ok(BatchInputs { queries, docs })
}

/// 得点（Q×N）の CPU の参照。各要素は `0.0` から始め、`d` の昇順に `mul_add` で積む。
fn reference_scores(scale: Scale, inputs: &BatchInputs) -> Result<Vec<f32>, BatchError> {
    if scale.n_docs == 0 || scale.dim == 0 || scale.n_queries == 0 {
        return Err(BatchError::EmptyScale);
    }
    if inputs.queries.len() != scale.query_len()? || inputs.docs.len() != scale.doc_len()? {
        return Err(BatchError::LengthMismatch);
    }
    let mut out = Vec::with_capacity(scale.n_queries.saturating_mul(scale.n_docs));
    for q in inputs.queries.chunks_exact(scale.dim) {
        for doc in inputs.docs.chunks_exact(scale.dim) {
            let mut acc = 0.0f32;
            for (&a, &b) in q.iter().zip(doc) {
                acc = a.mul_add(b, acc);
            }
            out.push(acc);
        }
    }
    Ok(out)
}

/// 部分 Top-k が書いた候補を、クエリごとの上位 `k` 件にまとめる。
///
/// 有限でない値と番兵の番号は除く。値の大きい順（`f32::total_cmp`）、同じ値は番号の小さい順に並べる。
/// 2 つの列の長さが `n_queries × 256 × k` でなければ `Err`。
fn merge_candidates(
    vals: &[f32],
    idx: &[u32],
    n_queries: usize,
    k: usize,
) -> Result<Vec<Vec<TopKEntry>>, BatchError> {
    check_k(k)?;
    let stride = (TOPK_THREADS as usize)
        .checked_mul(k)
        .ok_or(BatchError::SizeOverflow)?;
    let total = stride
        .checked_mul(n_queries)
        .ok_or(BatchError::SizeOverflow)?;
    if vals.len() != total || idx.len() != total {
        return Err(BatchError::LengthMismatch);
    }
    let mut rows = Vec::with_capacity(n_queries);
    for (v_row, i_row) in vals.chunks_exact(stride).zip(idx.chunks_exact(stride)) {
        let mut entries: Vec<TopKEntry> = v_row
            .iter()
            .zip(i_row)
            .filter(|(v, i)| v.is_finite() && **i != SENTINEL_INDEX)
            .map(|(v, i)| TopKEntry::new(*i, *v))
            .collect();
        entries.sort_unstable_by(|a, b| {
            b.score()
                .total_cmp(&a.score())
                .then(a.slot().cmp(&b.slot()))
        });
        entries.truncate(k);
        rows.push(entries);
    }
    Ok(rows)
}

/// 1 回の起動の形。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Launch {
    /// threadgroup あたりのスレッド数。
    threads: [u32; 3],
    /// threadgroup の数。
    groups: [u32; 3],
    /// 束縛と定数の slot の並び。
    layout: Vec<u32>,
    /// 定数の slot とネイティブエンディアンのバイト列。
    constants: Vec<(u32, Vec<u8>)>,
}

fn to_u32(x: usize) -> Result<u32, BatchError> {
    u32::try_from(x).map_err(|_| BatchError::SizeOverflow)
}

/// 得点の段の起動の形。定数は `{ n_docs, dim, n_queries }`。
fn scores_launch(scale: Scale) -> Result<Launch, BatchError> {
    scale.check(1)?;
    let n = to_u32(scale.n_docs)?;
    let d = to_u32(scale.dim)?;
    let q = to_u32(scale.n_queries)?;
    Ok(Launch {
        threads: [SCORE_TILE, SCORE_TILE, 1],
        groups: [n.div_ceil(SCORE_TILE), q.div_ceil(SCORE_TILE), 1],
        layout: vec![0, 1, 2, 3],
        constants: vec![(3, u32_ne_bytes(&[n, d, q]))],
    })
}

/// 部分 Top-k の段の起動の形。定数は `{ n_docs, k }`。
fn topk_launch(scale: Scale, k: usize) -> Result<Launch, BatchError> {
    scale.check(k)?;
    let n = to_u32(scale.n_docs)?;
    let q = to_u32(scale.n_queries)?;
    let k = to_u32(k)?;
    Ok(Launch {
        threads: [TOPK_THREADS, 1, 1],
        groups: [q, 1, 1],
        layout: vec![0, 1, 2, 3],
        constants: vec![(3, u32_ne_bytes(&[n, k]))],
    })
}

/// `u32` の列をネイティブエンディアンのバイト列にする。
fn u32_ne_bytes(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_ne_bytes()).collect()
}

/// `f32` の列をネイティブエンディアンのバイト列にする。
fn f32_ne_bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_ne_bytes()).collect()
}

/// ネイティブエンディアンのバイト列を `f32` の列に戻す。長さが 4 の倍数でなければ `Err`。
fn f32_from_ne_bytes(bytes: &[u8]) -> Result<Vec<f32>, BatchError> {
    if !bytes.len().is_multiple_of(4) {
        return Err(BatchError::InvalidBytes);
    }
    let (chunks, _) = bytes.as_chunks::<4>();
    Ok(chunks.iter().map(|c| f32::from_ne_bytes(*c)).collect())
}

/// ネイティブエンディアンのバイト列を `u32` の列に戻す。長さが 4 の倍数でなければ `Err`。
fn u32_from_ne_bytes(bytes: &[u8]) -> Result<Vec<u32>, BatchError> {
    if !bytes.len().is_multiple_of(4) {
        return Err(BatchError::InvalidBytes);
    }
    let (chunks, _) = bytes.as_chunks::<4>();
    Ok(chunks.iter().map(|c| u32::from_ne_bytes(*c)).collect())
}

/// 2 つの列の要素ごとの差の絶対値の最大。NaN を含めば無限大。長さが違えば `None`。
fn max_abs_diff(a: &[f32], b: &[f32]) -> Option<f32> {
    if a.len() != b.len() {
        return None;
    }
    let mut max = 0.0f32;
    for (x, y) in a.iter().zip(b) {
        let d = (x - y).abs();
        if d.is_nan() {
            return Some(f32::INFINITY);
        }
        max = max.max(d);
    }
    Some(max)
}

/// 2 つの段のカーネル（Metal Shading Language）。
///
/// 得点: 1 スレッドが（クエリ, 文書）の内積を `fma` で次元の昇順に積む。
/// 部分 Top-k: threadgroup 1 つがクエリ 1 つ。スレッド `t` は文書 `t, t+256, …` を見て、
/// 値の大きい順（同じ値は先に見た小さい番号を前）の局所の上位 `k` 件を自分の枠に書く。
/// 埋まらない枠の番兵は、値の比べを使わずビット列で書く。
const MSL_SOURCE: &str = r#"
#include <metal_stdlib>
using namespace metal;

#define MAX_K 16

struct ScoreParams {
    uint n_docs;
    uint dim;
    uint n_queries;
};

struct TopkParams {
    uint n_docs;
    uint k;
};

kernel void scores_f32(
    device const float* queries [[buffer(0)]],
    device const float* docs [[buffer(1)]],
    device float* scores [[buffer(2)]],
    constant ScoreParams& p [[buffer(3)]],
    uint2 gid [[thread_position_in_grid]])
{
    uint doc = gid.x;
    uint q = gid.y;
    if (doc >= p.n_docs || q >= p.n_queries) {
        return;
    }
    float acc = 0.0f;
    for (uint d = 0; d < p.dim; ++d) {
        acc = fma(queries[q * p.dim + d], docs[doc * p.dim + d], acc);
    }
    scores[q * p.n_docs + doc] = acc;
}

kernel void topk_partial_f32(
    device const float* scores [[buffer(0)]],
    device float* cand_vals [[buffer(1)]],
    device uint* cand_idx [[buffer(2)]],
    constant TopkParams& p [[buffer(3)]],
    uint q [[threadgroup_position_in_grid]],
    uint t [[thread_position_in_threadgroup]])
{
    float vals[MAX_K];
    uint idxs[MAX_K];
    uint k = min(p.k, (uint)MAX_K);
    uint count = 0;
    device const float* row = scores + q * p.n_docs;
    for (uint doc = t; doc < p.n_docs; doc += 256) {
        float v = row[doc];
        uint pos = count;
        if (count == k) {
            if (!(v > vals[k - 1])) {
                continue;
            }
            pos = k - 1;
        } else {
            count += 1;
        }
        while (pos > 0 && v > vals[pos - 1]) {
            vals[pos] = vals[pos - 1];
            idxs[pos] = idxs[pos - 1];
            pos -= 1;
        }
        vals[pos] = v;
        idxs[pos] = doc;
    }
    uint base = (q * 256 + t) * p.k;
    for (uint j = 0; j < p.k; ++j) {
        if (j < count) {
            cand_vals[base + j] = vals[j];
            cand_idx[base + j] = idxs[j];
        } else {
            cand_vals[base + j] = as_type<float>(0xFF800000u);
            cand_idx[base + j] = 0xFFFFFFFFu;
        }
    }
}
"#;

/// device に投入して待ち、出力を読む部分と、それを使う `#[ignore]` 付きの試験。
#[cfg(all(target_os = "macos", feature = "native-blocking-wait"))]
mod device {
    use std::time::Duration;

    use fandhe_silicon_core::capability::BackendKind;
    use fandhe_silicon_core::device::{
        Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
    };
    use fandhe_silicon_core::error::{CompletionState, ContractError};
    use fandhe_silicon_core::execution::{
        Binding, BindingLayout, ConstantBinding, PipelineInput, ThreadgroupMemory,
    };
    use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};
    use fandhe_silicon_core::parity::tolerance::{T1, compare_f32};
    use fandhe_silicon_core::parity::topk::{compare_topk, reference_topk_f32};
    use fandhe_silicon_core::test_frame::{
        Declaration, Label, MachineCombination, RunResult, RunSetting, Subject, Verdict,
        judge_and_export,
    };

    use super::{
        BatchError, BatchInputs, INPUT_SEED, LARGE, Launch, MSL_SOURCE, SCORES_ENTRY, SMALL, Scale,
        TOP_K, TOPK_ENTRY, f32_from_ne_bytes, f32_ne_bytes, generate_inputs, max_abs_diff,
        merge_candidates, reference_scores, scores_launch, topk_launch, u32_from_ne_bytes,
    };
    use crate::backend_metal::MetalDeviceProvider;

    /// 失敗した段。
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Stage {
        Setup,
        Scores,
        TopK,
    }

    /// 段の中で失敗した手順。
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum StepKind {
        Request,
        NoCompute,
        Pipeline,
        Allocate,
        Upload,
        Dispatch,
        Submit,
        Wait,
        NotComplete,
        Download,
        Convert,
    }

    /// 口の失敗。`kind` は契約のエラーを写した固定の種類の文字列か、手順の固定の語。
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct RunError {
        stage: Stage,
        step: StepKind,
        kind: &'static str,
    }

    impl std::fmt::Display for RunError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "run failed: stage={:?} step={:?} kind={}",
                self.stage, self.step, self.kind
            )
        }
    }

    fn err(stage: Stage, step: StepKind, kind: &'static str) -> RunError {
        RunError { stage, step, kind }
    }

    fn convert(stage: Stage, e: BatchError) -> RunError {
        let kind = match e {
            BatchError::SizeOverflow => "size-overflow",
            BatchError::LengthMismatch => "length-mismatch",
            BatchError::InvalidBytes => "invalid-bytes",
            _ => "invalid-batch",
        };
        err(stage, StepKind::Convert, kind)
    }

    /// 契約のエラーを、値を含まない固定の種類の文字列に写す。
    fn error_kind(e: &ContractError) -> &'static str {
        match e {
            ContractError::InvalidCombination(_) => "invalid-combination",
            ContractError::DeviceUnavailable(_) => "device-unavailable",
            ContractError::DeviceLost { .. } => "device-lost",
            ContractError::DeviceMismatch => "device-mismatch",
            ContractError::InvalidArgument(_) => "invalid-argument",
            ContractError::AllocationFailed { .. } => "allocation-failed",
            ContractError::ArtifactRejected { kind, .. } => match *kind {
                "msl" => "artifact-rejected-msl",
                "compute-pipeline" => "artifact-rejected-pipeline",
                _ => "artifact-rejected",
            },
            ContractError::ValidationFailed(_) => "validation-failed",
            ContractError::Timeout => "timeout",
            ContractError::Unsupported { .. } => "unsupported",
            ContractError::CapabilityInsufficient { .. } => "capability-insufficient",
            ContractError::DeviceIdentityMismatch { .. } => "device-identity-mismatch",
            ContractError::UnsupportedExtensionDefault { .. } => "unsupported-extension-default",
            ContractError::BackendFailure { .. } => "backend-failure",
            ContractError::CapacityExceeded { .. } => "capacity-exceeded",
            _ => "other",
        }
    }

    /// 2 段の出力。
    struct BatchOutputs {
        scores: Vec<f32>,
        cand_vals: Vec<f32>,
        cand_idx: Vec<u32>,
    }

    /// Metal の device を 1 つ作る（ComputeOnly・ProductIndependent）。
    fn open_device() -> Result<Box<dyn Device>, RunError> {
        MetalDeviceProvider::request(DeviceRequest {
            backend: BackendKind::Metal,
            profile: Profile::ComputeOnly,
            ownership: DeviceOwnership::ProductIndependent,
        })
        .map_err(|e| err(Stage::Setup, StepKind::Request, error_kind(&e)))
    }

    /// 確保する。確保したバッファは `held` に積み、添字を返す。
    fn alloc(
        device: &dyn Device,
        stage: Stage,
        bytes: usize,
        held: &mut Vec<BufferHandle>,
    ) -> Result<usize, RunError> {
        let bytes =
            u64::try_from(bytes).map_err(|_| err(stage, StepKind::Convert, "size-overflow"))?;
        let usage = BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD;
        let buf = device
            .memory()
            .allocate(bytes, usage)
            .map_err(|e| err(stage, StepKind::Allocate, error_kind(&e)))?;
        held.push(buf);
        Ok(held.len() - 1)
    }

    /// 確保して、`data` を書き込む。
    fn alloc_with(
        device: &dyn Device,
        stage: Stage,
        data: &[f32],
        held: &mut Vec<BufferHandle>,
    ) -> Result<usize, RunError> {
        let bytes = f32_ne_bytes(data);
        let id = alloc(device, stage, bytes.len(), held)?;
        device
            .memory()
            .upload(&held[id], 0, &bytes)
            .map_err(|e| err(stage, StepKind::Upload, error_kind(&e)))?;
        Ok(id)
    }

    /// 1 段を投入して完了を待つ。束縛の slot は `buffers` の並びの番号。
    fn dispatch_once(
        device: &dyn Device,
        stage: Stage,
        entry: &str,
        launch: &Launch,
        buffers: &[&BufferHandle],
    ) -> Result<(), RunError> {
        let compute = device
            .compute()
            .ok_or_else(|| err(stage, StepKind::NoCompute, "no-compute"))?;
        let pipeline = compute
            .create_pipeline(
                PipelineInput::Msl {
                    source: MSL_SOURCE,
                    entry_point: entry,
                },
                &BindingLayout::new(launch.layout.clone()),
            )
            .map_err(|e| err(stage, StepKind::Pipeline, error_kind(&e)))?;
        let bindings: Vec<Binding<'_>> = buffers
            .iter()
            .enumerate()
            .map(|(slot, buf)| Binding::new(slot as u32, buf, 0))
            .collect();
        let consts: Vec<ConstantBinding<'_>> = launch
            .constants
            .iter()
            .map(|(slot, bytes)| ConstantBinding::new(*slot, bytes))
            .collect();
        let tg: Vec<ThreadgroupMemory> = Vec::new();

        let queue = device.queue();
        let mut encoder = queue.create_encoder();
        queue
            .dispatch_ex(
                &mut encoder,
                &pipeline,
                &bindings,
                &consts,
                launch.groups,
                launch.threads,
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
        Ok(())
    }

    /// 完了の確認の後にだけ呼ぶ。バッファの先頭から `len` バイトを読む。
    fn read_bytes(
        device: &dyn Device,
        stage: Stage,
        buf: &BufferHandle,
        len: usize,
    ) -> Result<Vec<u8>, RunError> {
        let bytes =
            u64::try_from(len).map_err(|_| err(stage, StepKind::Convert, "size-overflow"))?;
        device
            .memory()
            .download_blocking(buf, 0, bytes)
            .map_err(|e| err(stage, StepKind::Download, error_kind(&e)))
    }

    /// 2 段を流す。確保したバッファは成否に関わらず解放する。
    fn run_batch(
        device: &dyn Device,
        scale: Scale,
        k: usize,
        inputs: &BatchInputs,
    ) -> Result<BatchOutputs, RunError> {
        let mut held: Vec<BufferHandle> = Vec::new();
        let result = batch_body(device, scale, k, inputs, &mut held);
        for buf in held {
            device.memory().free(buf);
        }
        result
    }

    fn batch_body(
        device: &dyn Device,
        scale: Scale,
        k: usize,
        inputs: &BatchInputs,
        held: &mut Vec<BufferHandle>,
    ) -> Result<BatchOutputs, RunError> {
        let score_bytes = scale
            .score_len()
            .and_then(|n| n.checked_mul(4).ok_or(BatchError::SizeOverflow))
            .map_err(|e| convert(Stage::Scores, e))?;
        let cand_bytes = scale
            .candidate_len(k)
            .and_then(|n| n.checked_mul(4).ok_or(BatchError::SizeOverflow))
            .map_err(|e| convert(Stage::TopK, e))?;
        let score_launch = scores_launch(scale).map_err(|e| convert(Stage::Scores, e))?;
        let topk = topk_launch(scale, k).map_err(|e| convert(Stage::TopK, e))?;

        let q_id = alloc_with(device, Stage::Scores, &inputs.queries, held)?;
        let d_id = alloc_with(device, Stage::Scores, &inputs.docs, held)?;
        let s_id = alloc(device, Stage::Scores, score_bytes, held)?;
        dispatch_once(
            device,
            Stage::Scores,
            SCORES_ENTRY,
            &score_launch,
            &[&held[q_id], &held[d_id], &held[s_id]],
        )?;
        let scores = f32_from_ne_bytes(&read_bytes(
            device,
            Stage::Scores,
            &held[s_id],
            score_bytes,
        )?)
        .map_err(|e| convert(Stage::Scores, e))?;

        let v_id = alloc(device, Stage::TopK, cand_bytes, held)?;
        let i_id = alloc(device, Stage::TopK, cand_bytes, held)?;
        dispatch_once(
            device,
            Stage::TopK,
            TOPK_ENTRY,
            &topk,
            &[&held[s_id], &held[v_id], &held[i_id]],
        )?;
        let cand_vals =
            f32_from_ne_bytes(&read_bytes(device, Stage::TopK, &held[v_id], cand_bytes)?)
                .map_err(|e| convert(Stage::TopK, e))?;
        let cand_idx =
            u32_from_ne_bytes(&read_bytes(device, Stage::TopK, &held[i_id], cand_bytes)?)
                .map_err(|e| convert(Stage::TopK, e))?;
        Ok(BatchOutputs {
            scores,
            cand_vals,
            cand_idx,
        })
    }

    fn subject(test_id: &str, behavior: &str) -> Subject {
        Subject::from_parts(test_id, &[behavior], &["t-1", "t-2"]).expect("subject")
    }

    /// 試験の枠で判定して書き出し、OK でなければ失敗にする。
    fn judge(subject: &Subject, body: impl FnOnce() -> Verdict) {
        let declaration =
            Declaration::from_parts(&[MachineCombination::MetalMacMini], &[]).expect("declaration");
        let setting = RunSetting::from_env();
        let outcome = judge_and_export(subject, &declaration, &setting, |_| {
            RunResult::Verdict(body())
        })
        .expect("export failed");
        assert_eq!(outcome.label(), &Label::Ok);
    }

    /// 1 つの規模を流して、得点と全クエリの部分 Top-k が CPU の参照と一致したときだけ `Match`。
    fn batch_case(test_id: &str, behavior: &str, scale: Scale) {
        let k = TOP_K;
        let inputs = generate_inputs(scale, INPUT_SEED).expect("inputs");
        let expected_scores = reference_scores(scale, &inputs).expect("reference scores");
        judge(&subject(test_id, behavior), || {
            let device = match open_device() {
                Ok(d) => d,
                Err(e) => {
                    println!("{e}");
                    return Verdict::Mismatch;
                }
            };
            let out = match run_batch(&*device, scale, k, &inputs) {
                Ok(o) => o,
                Err(e) => {
                    println!("{e}");
                    return Verdict::Mismatch;
                }
            };
            let scores_verdict = match compare_f32(&expected_scores, &out.scores, T1) {
                Ok(cmp) => Verdict::from(&cmp),
                Err(_) => Verdict::Mismatch,
            };
            let merged = match merge_candidates(&out.cand_vals, &out.cand_idx, scale.n_queries, k) {
                Ok(m) => m,
                Err(e) => {
                    println!("merge failed: {e}");
                    return Verdict::Mismatch;
                }
            };
            let mut matched = 0usize;
            for (q, got) in merged.iter().enumerate() {
                let Some(row) = out.scores.get(q * scale.n_docs..(q + 1) * scale.n_docs) else {
                    continue;
                };
                let Ok(want) = reference_topk_f32(row, k) else {
                    continue;
                };
                if let Ok(cmp) = compare_topk(&want, got)
                    && cmp.is_match()
                {
                    matched += 1;
                }
            }
            let topk_ok = matched == scale.n_queries && merged.len() == scale.n_queries;
            let verdict = if scores_verdict == Verdict::Match && topk_ok {
                Verdict::Match
            } else {
                Verdict::Mismatch
            };
            println!(
                "scale={scale:?} k={k} scores_max_abs_diff={:?} topk_matched={matched}/{} verdict={verdict:?}",
                max_abs_diff(&expected_scores, &out.scores),
                scale.n_queries
            );
            verdict
        });
    }

    #[test]
    #[ignore = "touches the Metal device; run only in a dedicated device session"]
    fn scores_and_topk_small_match_reference() {
        // 項目 ID のポインタ: ART-79
        batch_case("batch_topk_small", "ART-79", SMALL);
    }

    #[test]
    #[ignore = "touches the Metal device; run only in a dedicated device session"]
    fn scores_and_topk_large_match_reference() {
        batch_case("batch_topk_large", "ART-79", LARGE);
    }
}

#[cfg(test)]
mod pure_tests {
    use super::*;

    fn tiny() -> Scale {
        Scale {
            n_docs: 3,
            dim: 2,
            n_queries: 2,
        }
    }

    #[test]
    fn scores_reference_is_ascending_mul_add() {
        let scale = Scale {
            n_docs: 1,
            dim: 2,
            n_queries: 1,
        };
        let e = 2.0f32.powi(-13);
        let inputs = BatchInputs {
            queries: vec![1.0, 1.0 + e],
            docs: vec![-1.0, 1.0 - e],
        };
        let out = reference_scores(scale, &inputs).expect("scores");
        assert_eq!(out, vec![-(2.0f32.powi(-26))]);
        // 積を丸めてから足す素朴な式では 0 になる。
        let naive = std::hint::black_box(-1.0f32) + std::hint::black_box(1.0 + e) * (1.0 - e);
        assert_eq!(naive, 0.0);
        assert_ne!(out[0], naive);
    }

    #[test]
    fn scores_reference_lays_out_query_major() {
        let inputs = BatchInputs {
            queries: vec![1.0, 0.0, 0.0, 1.0],
            docs: vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
        };
        let out = reference_scores(tiny(), &inputs).expect("scores");
        assert_eq!(out, vec![1.0, 3.0, 5.0, 2.0, 4.0, 6.0]);
    }

    #[test]
    fn scores_reference_rejects_bad_lengths_and_empty_scale() {
        let inputs = BatchInputs {
            queries: vec![1.0; 3],
            docs: vec![1.0; 6],
        };
        assert_eq!(
            reference_scores(tiny(), &inputs),
            Err(BatchError::LengthMismatch)
        );
        let zero = Scale {
            n_docs: 0,
            ..tiny()
        };
        assert_eq!(reference_scores(zero, &inputs), Err(BatchError::EmptyScale));
    }

    #[test]
    fn inputs_are_deterministic_and_sized() {
        let a = generate_inputs(tiny(), INPUT_SEED).expect("a");
        let b = generate_inputs(tiny(), INPUT_SEED).expect("b");
        assert_eq!(a, b);
        assert_eq!(a.queries.len(), 4);
        assert_eq!(a.docs.len(), 6);
        let c = generate_inputs(tiny(), INPUT_SEED ^ 1).expect("c");
        assert_ne!(a, c);
        assert!(
            a.queries
                .iter()
                .chain(&a.docs)
                .all(|v| (-1.0..1.0).contains(v))
        );
    }

    /// 1 クエリ分の候補（256 × k 件）を作る。先頭の `head` 以外は番兵。
    fn padded(head: &[(f32, u32)], k: usize) -> (Vec<f32>, Vec<u32>) {
        let total = TOPK_THREADS as usize * k;
        let mut vals = vec![f32::NEG_INFINITY; total];
        let mut idx = vec![SENTINEL_INDEX; total];
        for (i, (v, s)) in head.iter().enumerate() {
            vals[i] = *v;
            idx[i] = *s;
        }
        (vals, idx)
    }

    fn pairs(row: &[TopKEntry]) -> Vec<(u32, f32)> {
        row.iter().map(|e| (e.slot(), e.score())).collect()
    }

    #[test]
    fn merge_orders_by_value_then_index() {
        let (vals, idx) = padded(&[(1.0, 7), (3.0, 9), (3.0, 2), (2.0, 4), (3.0, 5)], 3);
        let rows = merge_candidates(&vals, &idx, 1, 3).expect("merge");
        assert_eq!(pairs(&rows[0]), vec![(2, 3.0), (5, 3.0), (9, 3.0)]);
    }

    #[test]
    fn merge_drops_non_finite_and_sentinels() {
        let (vals, idx) = padded(
            &[
                (f32::NAN, 1),
                (f32::INFINITY, 2),
                (f32::NEG_INFINITY, 3),
                (0.5, SENTINEL_INDEX),
                (-1.0, 8),
            ],
            2,
        );
        let rows = merge_candidates(&vals, &idx, 1, 2).expect("merge");
        assert_eq!(pairs(&rows[0]), vec![(8, -1.0)]);
    }

    #[test]
    fn merge_returns_fewer_than_k_when_candidates_run_out() {
        let (vals, idx) = padded(&[(4.0, 1), (5.0, 0)], 4);
        let rows = merge_candidates(&vals, &idx, 1, 4).expect("merge");
        assert_eq!(pairs(&rows[0]), vec![(0, 5.0), (1, 4.0)]);
    }

    #[test]
    fn merge_keeps_query_boundaries() {
        let k = 2;
        let (mut vals, mut idx) = padded(&[(1.0, 10)], k);
        let (v2, i2) = padded(&[(9.0, 20)], k);
        vals.extend(v2);
        idx.extend(i2);
        let rows = merge_candidates(&vals, &idx, 2, k).expect("merge");
        assert_eq!(rows.len(), 2);
        assert_eq!(pairs(&rows[0]), vec![(10, 1.0)]);
        assert_eq!(pairs(&rows[1]), vec![(20, 9.0)]);
    }

    #[test]
    fn merge_rejects_length_mismatch_and_bad_k() {
        let (vals, idx) = padded(&[], 2);
        assert_eq!(
            merge_candidates(&vals, &idx, 2, 2).err(),
            Some(BatchError::LengthMismatch)
        );
        assert_eq!(
            merge_candidates(&vals, &idx[1..], 1, 2).err(),
            Some(BatchError::LengthMismatch)
        );
        assert_eq!(
            merge_candidates(&vals, &idx, 1, 0).err(),
            Some(BatchError::InvalidK)
        );
        assert_eq!(
            merge_candidates(&vals, &idx, 1, MAX_K + 1).err(),
            Some(BatchError::InvalidK)
        );
    }

    fn words(bytes: &[u8]) -> Vec<u32> {
        u32_from_ne_bytes(bytes).expect("words")
    }

    #[test]
    fn scores_launch_follows_the_scale() {
        let small = scores_launch(SMALL).expect("small");
        assert_eq!(small.threads, [16, 16, 1]);
        assert_eq!(small.groups, [1250, 4, 1]);
        assert_eq!(small.layout, vec![0, 1, 2, 3]);
        assert_eq!(small.constants.len(), 1);
        assert_eq!(small.constants[0].0, 3);
        assert_eq!(small.constants[0].1.len(), 12);
        assert_eq!(words(&small.constants[0].1), vec![20_000, 128, 64]);

        let large = scores_launch(LARGE).expect("large");
        assert_eq!(large.groups, [6250, 4, 1]);
        assert_eq!(words(&large.constants[0].1), vec![100_000, 128, 64]);
    }

    #[test]
    fn scores_launch_rounds_groups_up() {
        let scale = Scale {
            n_docs: 17,
            dim: 3,
            n_queries: 33,
        };
        let l = scores_launch(scale).expect("launch");
        assert_eq!(l.groups, [2, 3, 1]);
    }

    #[test]
    fn topk_launch_uses_one_group_per_query() {
        let l = topk_launch(LARGE, TOP_K).expect("launch");
        assert_eq!(l.threads, [256, 1, 1]);
        assert_eq!(l.groups, [64, 1, 1]);
        assert_eq!(l.layout, vec![0, 1, 2, 3]);
        assert_eq!(l.constants.len(), 1);
        assert_eq!(l.constants[0].0, 3);
        assert_eq!(l.constants[0].1.len(), 8);
        assert_eq!(words(&l.constants[0].1), vec![100_000, 10]);
    }

    #[test]
    fn launch_rejects_bad_k_and_overflowing_scales() {
        assert_eq!(topk_launch(SMALL, 0), Err(BatchError::InvalidK));
        assert_eq!(topk_launch(SMALL, MAX_K + 1), Err(BatchError::InvalidK));
        assert!(topk_launch(SMALL, MAX_K).is_ok());
        let huge = Scale {
            n_docs: usize::MAX,
            dim: 2,
            n_queries: 1,
        };
        assert_eq!(scores_launch(huge), Err(BatchError::SizeOverflow));
        let wide = Scale {
            n_docs: 1 << 30,
            dim: 8,
            n_queries: 1,
        };
        assert_eq!(scores_launch(wide), Err(BatchError::SizeOverflow));
        let empty = Scale {
            n_queries: 0,
            ..SMALL
        };
        assert_eq!(scores_launch(empty), Err(BatchError::EmptyScale));
    }

    #[test]
    fn byte_conversions_round_trip_and_reject_odd_lengths() {
        let f = [1.5f32, -0.0, f32::MAX];
        assert_eq!(f32_from_ne_bytes(&f32_ne_bytes(&f)).expect("f"), f.to_vec());
        let u = [0u32, 7, u32::MAX];
        assert_eq!(u32_from_ne_bytes(&u32_ne_bytes(&u)).expect("u"), u.to_vec());
        assert_eq!(f32_from_ne_bytes(&[0; 3]), Err(BatchError::InvalidBytes));
        assert_eq!(u32_from_ne_bytes(&[0; 5]), Err(BatchError::InvalidBytes));
    }

    #[test]
    fn max_abs_diff_reports_nan_and_length() {
        assert_eq!(max_abs_diff(&[1.0, 2.0], &[1.5, 2.0]), Some(0.5));
        assert_eq!(max_abs_diff(&[f32::NAN], &[0.0]), Some(f32::INFINITY));
        assert_eq!(max_abs_diff(&[1.0], &[1.0, 2.0]), None);
    }

    #[test]
    fn candidate_and_score_lengths_follow_the_scale() {
        let scale = tiny();
        assert_eq!(scale.score_len(), Ok(6));
        assert_eq!(scale.candidate_len(4), Ok(2 * 256 * 4));
    }

    #[test]
    fn msl_source_defines_both_entries() {
        for entry in [SCORES_ENTRY, TOPK_ENTRY] {
            assert!(
                MSL_SOURCE.contains(&format!("kernel void {entry}(")),
                "{entry}"
            );
        }
        assert!(MSL_SOURCE.contains(&format!("#define MAX_K {MAX_K}")));
    }

    #[test]
    fn error_messages_are_fixed_without_values() {
        for e in [
            BatchError::SizeOverflow,
            BatchError::EmptyScale,
            BatchError::LengthMismatch,
            BatchError::InvalidK,
            BatchError::InvalidBytes,
        ] {
            let s = e.to_string();
            assert!(!s.is_empty());
            assert!(!s.chars().any(|c| c.is_ascii_digit()), "{s}");
        }
    }
}
