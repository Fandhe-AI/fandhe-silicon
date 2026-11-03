//! `cpu_ops` の公開の型と `CpuOps` の形を、外の crate として固定する試験（spec TASK-2457 のポインタ）。
//!
//! 使うのは公開のパス `fandhe_silicon_contract::cpu_ops::…` と `fandhe_silicon_core::error::ContractError` だけ。
//! `CpuOps` の署名は試験の中の型への実装で、メソッドの数と名前は `src/cpu_ops.rs` の文字列の判定で固定する。
//! 判定は純粋関数 `trait_method_names` に分け、実物を 1 か所だけ変えた入力を渡す陽性対照を常設する。
//! 条件のない試験（`cpu-ops` の有無は `[[test]]` の `required-features` で決まる）。

use fandhe_silicon_contract::cpu_ops::{
    CpuOps, DefaultCpuOps, ReferenceCpuOps, UnaryElementwiseOp,
};
use fandhe_silicon_core::error::ContractError;

/// 署名を固定するための、試験の中の実装。値は書かない。
struct ProbeOps;

impl CpuOps for ProbeOps {
    fn gemm_scalar(
        &self,
        _a: &[f32],
        _b: &[f32],
        _m: usize,
        _k: usize,
        _n: usize,
        _out: &mut [f32],
    ) -> Result<(), ContractError> {
        Ok(())
    }
    fn gemm_simd(
        &self,
        _a: &[f32],
        _b: &[f32],
        _m: usize,
        _k: usize,
        _n: usize,
        _out: &mut [f32],
    ) -> Result<(), ContractError> {
        Ok(())
    }
    fn elementwise_scalar(
        &self,
        _op: UnaryElementwiseOp,
        _a: &[f32],
        _out: &mut [f32],
    ) -> Result<(), ContractError> {
        Ok(())
    }
    fn elementwise_simd(
        &self,
        _op: UnaryElementwiseOp,
        _a: &[f32],
        _out: &mut [f32],
    ) -> Result<(), ContractError> {
        Ok(())
    }
    fn topk_partial_scalar(
        &self,
        _scores: &[f32],
        _k: usize,
        _out: &mut [(u32, f32)],
    ) -> Result<(), ContractError> {
        Ok(())
    }
    fn topk_partial_simd(
        &self,
        _scores: &[f32],
        _k: usize,
        _out: &mut [(u32, f32)],
    ) -> Result<(), ContractError> {
        Ok(())
    }
}

const TRAIT_OPEN: &str = "pub trait CpuOps {";
const EXPECTED_METHODS: [&str; 6] = [
    "gemm_scalar",
    "gemm_simd",
    "elementwise_scalar",
    "elementwise_simd",
    "topk_partial_scalar",
    "topk_partial_simd",
];

/// `cpu_ops.rs` の文字列から、`CpuOps` の本体のメソッド名を並び順に返す。
/// 開きの行がちょうど 1 行でない・閉じ（桁 0 の `}`）が無い場合は `Err`。
fn trait_method_names(text: &str) -> Result<Vec<String>, String> {
    let lines: Vec<&str> = text.lines().collect();
    let opens: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.trim() == TRAIT_OPEN)
        .map(|(i, _)| i)
        .collect();
    if opens.len() != 1 {
        return Err(format!("`{TRAIT_OPEN}` lines: {}", opens.len()));
    }
    let start = opens[0] + 1;
    let end = lines[start..]
        .iter()
        .position(|l| *l == "}")
        .map(|p| start + p)
        .ok_or_else(|| "closing `}` not found".to_string())?;
    let mut names = Vec::new();
    for line in &lines[start..end] {
        if let Some(rest) = line.trim_start().strip_prefix("fn ") {
            let name = rest
                .split('(')
                .next()
                .ok_or_else(|| "method name not found".to_string())?;
            names.push(name.trim().to_string());
        }
    }
    Ok(names)
}

fn real_cpu_ops_source() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/cpu_ops.rs");
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"))
}

/// `from` が 1 回だけ現れることを確かめてから `to` に置き換える。当たらなければ落とす。
fn replace_once(text: &str, from: &str, to: &str) -> String {
    assert_eq!(
        text.matches(from).count(),
        1,
        "pattern must match once: {from}"
    );
    text.replacen(from, to, 1)
}

fn needs_debug_clone_copy<T: core::fmt::Debug + Clone + Copy>(_: T) {}

// CPU-25
#[test]
fn trait_signatures_are_implementable() {
    let ops: &dyn CpuOps = &ProbeOps;
    let mut out = [0.0f32; 1];
    assert!(ops.gemm_simd(&[1.0], &[1.0], 1, 1, 1, &mut out).is_ok());
    let mut top = [(0u32, 0.0f32); 1];
    assert!(ops.topk_partial_simd(&[1.0], 1, &mut top).is_ok());
}

// CPU-25
#[test]
fn trait_has_six_methods() {
    let names = trait_method_names(&real_cpu_ops_source()).expect("trait body must be found");
    assert_eq!(names.len(), 6, "{names:?}");
    let mut got: Vec<&str> = names.iter().map(String::as_str).collect();
    got.sort_unstable();
    let mut want = EXPECTED_METHODS.to_vec();
    want.sort_unstable();
    assert_eq!(got, want);
}

#[test]
fn method_count_check_fails_when_a_method_is_added() {
    let real = real_cpu_ops_source();
    // 開きの直後に、インデントした本体つきの既定メソッドを足す（本体の `    }` を閉じと取り違えない）。
    let added = replace_once(
        &real,
        TRAIT_OPEN,
        "pub trait CpuOps {\n    fn extra(&self) -> usize {\n        0\n    }",
    );
    let names = trait_method_names(&added).expect("still parsable");
    assert_eq!(names.len(), 7, "{names:?}");
}

#[test]
fn method_count_check_fails_when_a_method_is_removed() {
    let real = real_cpu_ops_source();
    // 実装側にも同じ行があるので、trait の本体にある最初の 1 件だけを消す。
    let removed = real.replacen(
        "    fn topk_partial_simd(",
        "    // fn topk_partial_simd(",
        1,
    );
    assert_ne!(removed, real, "pattern must match");
    let names = trait_method_names(&removed).expect("still parsable");
    assert_eq!(names.len(), 5, "{names:?}");
    assert!(!names.iter().any(|n| n == "topk_partial_simd"));
}

#[test]
fn method_count_check_fails_when_open_line_is_missing() {
    let real = real_cpu_ops_source();
    let changed = replace_once(&real, TRAIT_OPEN, "pub trait CpuOpsRenamed {");
    assert!(trait_method_names(&changed).is_err());
    assert!(trait_method_names("").is_err());
}

// CPU-25
#[test]
fn implementors_are_object_safe() {
    let all: [&dyn CpuOps; 3] = [&DefaultCpuOps, &ReferenceCpuOps, &ProbeOps];
    for ops in all {
        let mut prod = [0.0f32; 1];
        assert!(ops.gemm_scalar(&[2.0], &[3.0], 1, 1, 1, &mut prod).is_ok());
        let mut act = [0.0f32; 2];
        assert!(
            ops.elementwise_scalar(UnaryElementwiseOp::Relu, &[-1.0, 2.0], &mut act)
                .is_ok()
        );
    }
    for ops in [&DefaultCpuOps as &dyn CpuOps, &ReferenceCpuOps] {
        let mut prod = [0.0f32; 1];
        ops.gemm_scalar(&[2.0], &[3.0], 1, 1, 1, &mut prod).unwrap();
        assert_eq!(prod[0].to_bits(), 6.0f32.to_bits());
        let mut act = [9.0f32; 2];
        ops.elementwise_scalar(UnaryElementwiseOp::Relu, &[-1.0, 2.0], &mut act)
            .unwrap();
        assert_eq!(act[0].to_bits(), 0.0f32.to_bits());
        assert_eq!(act[1].to_bits(), 2.0f32.to_bits());
    }
}

// CPU-25
#[test]
fn product_and_reference_types_are_in_cpu_ops() {
    let d = fandhe_silicon_contract::cpu_ops::DefaultCpuOps;
    let r = fandhe_silicon_contract::cpu_ops::ReferenceCpuOps;
    needs_debug_clone_copy(d);
    needs_debug_clone_copy(r);
    assert!(!format!("{d:?}").is_empty());
    assert!(!format!("{r:?}").is_empty());
}

// CPU-25
#[test]
fn reference_sentinel() {
    let s = ReferenceCpuOps::SENTINEL;
    assert_eq!(s.0, u32::MAX);
    assert_eq!(s.1.to_bits(), f32::NEG_INFINITY.to_bits());
}

/// 外の crate からは `#[non_exhaustive]` のため `_` の腕が要る。
fn kind(op: UnaryElementwiseOp) -> Option<(&'static str, u32)> {
    match op {
        UnaryElementwiseOp::Relu => Some(("relu", 0)),
        UnaryElementwiseOp::AddScalar(c) => Some(("add_scalar", c.to_bits())),
        _ => None,
    }
}

// CPU-25
#[test]
fn elementwise_op_values() {
    assert_eq!(kind(UnaryElementwiseOp::Relu), Some(("relu", 0)));
    assert_eq!(
        kind(UnaryElementwiseOp::AddScalar(1.5)),
        Some(("add_scalar", 1.5f32.to_bits()))
    );
    needs_debug_clone_copy(UnaryElementwiseOp::Relu);
    let a = UnaryElementwiseOp::AddScalar(1.0);
    let b = a;
    assert_eq!(a, b);
    assert_ne!(UnaryElementwiseOp::Relu, UnaryElementwiseOp::AddScalar(0.0));
}
