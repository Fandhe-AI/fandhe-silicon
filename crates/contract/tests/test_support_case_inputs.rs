//! contract の feature `test-support` だけで、core の 17 ケースの入力と参照の出力に届くことの確認（spec TASK-977 の 4/4）。
//! `make build-profiles` の `test-support` を含む各組で、背後実装の feature に依らず同じ結果になることも確かめる。
//! 置き場・feature の説明は docs/parity-case-inputs.md の「別 crate からの読み込み（4/4）」。

use fandhe_silicon_core::hash::sha256_bytes;
use fandhe_silicon_core::parity::case_inputs::{
    CaseId, CaseInput, CaseReferenceError, additional_topk_inputs, classify, total_output_len,
};
use fandhe_silicon_core::parity::cpu_reference::{ReferenceError, reference_gemm_f32};
use fandhe_silicon_core::parity::topk::{TopKReferenceError, reference_topk_f32};

fn same_bits_or_both_nan(a: f32, b: f32) -> bool {
    (a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits()
}

fn digest(v: &[f32]) -> String {
    let bytes: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
    sha256_bytes(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[test]
fn contract_test_support_enables_core_case_inputs() {
    assert_eq!(CaseId::ALL.len(), 17);
    let mut names: Vec<&str> = CaseId::ALL.iter().map(|c| c.name()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 17);
    for c in CaseId::ALL {
        assert!(c.input().is_ok(), "{c:?}");
        assert!(c.reference().is_ok(), "{c:?}");
    }
}

#[test]
fn reference_len_matches_output_len_under_any_backend_feature_set() {
    for c in CaseId::ALL {
        let want = c.input().unwrap().output_len().unwrap();
        assert_eq!(c.reference().unwrap().len(), want, "{c:?}");
    }
}

#[test]
fn total_output_len_is_3096_under_any_backend_feature_set() {
    assert_eq!(total_output_len().unwrap(), 3096);
    let sum: usize = CaseId::ALL
        .iter()
        .map(|c| c.reference().unwrap().len())
        .sum();
    assert_eq!(sum, 3096);
}

#[test]
fn gemm_reference_is_mul_add_accumulation_from_outside_core() {
    let mut gemm_cases = 0;
    for c in CaseId::ALL {
        let CaseInput::Gemm(g) = c.input().unwrap() else {
            continue;
        };
        gemm_cases += 1;
        let out = c.reference().unwrap();
        let got = out.as_f32().unwrap();
        let direct = reference_gemm_f32(g.a(), g.b(), g.m(), g.k(), g.n()).unwrap();
        assert_eq!(got.len(), g.m() * g.n(), "{c:?}");
        for i in 0..g.m() {
            for j in 0..g.n() {
                // 0.0 から k の昇順に f32::mul_add で積み上げる（試験側の独立した計算）。
                let mut acc = 0.0f32;
                for p in 0..g.k() {
                    acc = g.a()[i * g.k() + p].mul_add(g.b()[p * g.n() + j], acc);
                }
                let idx = i * g.n() + j;
                assert!(same_bits_or_both_nan(got[idx], acc), "{c:?}[{idx}]");
                assert!(same_bits_or_both_nan(got[idx], direct[idx]), "{c:?}[{idx}]");
                assert_eq!(classify(got[idx]), g.design()[idx], "{c:?}[{idx}]");
            }
        }
    }
    // GEMM は g-01〜g-13 の 13 件と、GEMM の形の e-1。
    assert_eq!(gemm_cases, 14);
}

#[test]
fn relu_reference_is_reachable_from_outside_core() {
    let CaseInput::Relu(input) = CaseId::E2.input().unwrap() else {
        panic!("e-2 is not relu")
    };
    let out = CaseId::E2.reference().unwrap();
    let got = out.as_f32().unwrap();
    assert_eq!(got.len(), input.len());
    for (i, (x, y)) in input.iter().zip(got).enumerate() {
        let want = if *x > 0.0 { *x } else { 0.0 };
        assert_eq!(y.to_bits(), want.to_bits(), "[{i}]");
    }
}

#[test]
fn topk_reference_is_reachable_from_outside_core() {
    for (c, len) in [(CaseId::E3, 5), (CaseId::E4, 8)] {
        let CaseInput::TopK { scores, k } = c.input().unwrap() else {
            panic!("{c:?} is not top-k")
        };
        let want = reference_topk_f32(&scores, k).unwrap();
        let out = c.reference().unwrap();
        let got = out.as_topk().unwrap();
        assert_eq!(got.len(), len, "{c:?}");
        assert_eq!(got.len(), want.len(), "{c:?}");
        for (g, w) in got.iter().zip(&want) {
            assert_eq!(
                (g.slot(), g.score_bits()),
                (w.slot(), w.score_bits()),
                "{c:?}"
            );
        }
    }
}

#[test]
fn additional_topk_inputs_are_reachable_from_outside_core() {
    let inputs = additional_topk_inputs();
    assert!(!inputs.is_empty());
    for t in inputs {
        let s = t.scores();
        let neg = s.iter().position(|x| x.to_bits() == (-0.0f32).to_bits());
        let pos = s.iter().rposition(|x| x.to_bits() == 0.0f32.to_bits());
        let (Some(n), Some(p)) = (neg, pos) else {
            panic!("{}: needs both zeros", t.name());
        };
        assert!(n < p, "{}", t.name());
        let want = t.expected();
        let got = reference_topk_f32(s, t.k()).unwrap();
        assert_eq!(got.len(), want.len(), "{}", t.name());
        for (g, w) in got.iter().zip(&want) {
            assert_eq!(
                (g.slot(), g.score_bits()),
                (w.slot(), w.score_bits()),
                "{}",
                t.name()
            );
        }
        assert!(
            CaseId::ALL.iter().all(|c| c.name() != t.name()),
            "{}",
            t.name()
        );
    }
}

#[test]
fn case_reference_error_is_reachable_from_outside_core() {
    let a: CaseReferenceError = ReferenceError::SizeOverflow.into();
    assert_eq!(a, CaseReferenceError::Gemm(ReferenceError::SizeOverflow));
    let b: CaseReferenceError = TopKReferenceError::SlotOverflow.into();
    assert_eq!(
        b,
        CaseReferenceError::TopK(TopKReferenceError::SlotOverflow)
    );
    assert!(!a.to_string().is_empty());
    assert!(!b.to_string().is_empty());
}

/// 各ケースの入力（A→B の順、または Relu・Top-k のスコア）の f32 のビット列（LE）の sha256。
/// 値は core の `inputs_are_fixed_by_sha256` と同じ。背後実装の feature の組で入力が変わらないことの確かめ。
#[test]
fn inputs_are_identical_under_any_backend_feature_set() {
    let expected: [(CaseId, &str); 17] = [
        (
            CaseId::G01,
            "993b476c2a937a05c7b0d64790b5613dd33c9842452c1b21b3fe12f707f3207c",
        ),
        (
            CaseId::G02,
            "ada18d5d50cb71094eba24dcf1d756c04234b94fce73ff723cf51078c1ea1088",
        ),
        (
            CaseId::G03,
            "0c1be1a8a877eb2db6f3b068af08c06aab51738132e7ff108bbdca7634f65d63",
        ),
        (
            CaseId::G04,
            "f33ac85c35ef821529d1d97d46cb71d781871f09b43ee3449ad18d2bd976bc9b",
        ),
        (
            CaseId::G05,
            "8774c3dcf916c79979162dfdadbeefbc111a7c95fc9ef19dd395027fa008f406",
        ),
        (
            CaseId::G06,
            "26de6ac6f0e6f6356a8a6538506f96c847d6e6dc301d5c6cf6b9094c71d9df35",
        ),
        (
            CaseId::G07,
            "d8ccb8a65d072ba02c384fe3b06b6dfcec447dc6da4757c6d8baaab727ee65a3",
        ),
        (
            CaseId::G08,
            "77062b3e0918248ee7b69dcb8b9f80456f0c906260d1e54c212123f4a8522734",
        ),
        (
            CaseId::G09,
            "f325d011383a5c98c4118e03c4c47151b2ec2d79e2fc1dc907a914a5a289f9e9",
        ),
        (
            CaseId::G10,
            "8f0c4a93fe4b91d6b16fed5e04b2821ca6eed1ac3838eac3dbbc97b1bb499b73",
        ),
        (
            CaseId::G11,
            "8948d94c78960285ebec0999a1e9496a453795b3bd51e6b2c9b5d1385f431313",
        ),
        (
            CaseId::G12,
            "87fcecbae83e841ce778749bd7f7c404681452a70f46f0b449c906ee572e5e1d",
        ),
        (
            CaseId::G13,
            "6d20f99cbe0b2ab0fe1a6c56fcd3d623b087bc094d2872b461e801d03c3f2dda",
        ),
        (
            CaseId::E1,
            "698144f236cf9f0e4fc70880e2e3bb5596faf716c3ea512c1124c4861c6a55cd",
        ),
        (
            CaseId::E2,
            "f3973b98178c51aecfe234068e78dccd4b0cc17e682d3f1176279297c71d21c6",
        ),
        (
            CaseId::E3,
            "037f422cc160e7f2f7e940b66409641bf5cdb1516202a671366be426f3f23598",
        ),
        (
            CaseId::E4,
            "38b632eafbbd18de1ba0f25cf69ab8dfcc0badea38d57876373b9b14a6a27cc2",
        ),
    ];
    for (c, want) in expected {
        let all: Vec<f32> = match c.input().unwrap() {
            CaseInput::Gemm(g) => g.a().iter().chain(g.b()).copied().collect(),
            CaseInput::Relu(v) => v,
            CaseInput::TopK { scores, .. } => scores,
        };
        assert_eq!(digest(&all), want, "{c:?}");
    }
}
