//! 17 ケースの入力の確認（spec TASK-977 の 2/4）。
//! feature `test-support` の裏にだけ存在する検証用の道具を対象にする。
//!
//! sha256 の値は、本リポの製品のコードが出す既知の出力として固定する
//! （`parity_gemm_inputs.rs` と同じ流儀）。参照の出力そのものは固定しない。

use fandhe_silicon_core::hash::sha256_bytes;
use fandhe_silicon_core::parity::case_inputs::{CaseId, CaseInput, OutputClass, classify};
use fandhe_silicon_core::parity::cpu_reference::reference_gemm_f32;
use fandhe_silicon_core::test_frame::TestId;

fn hex(d: [u8; 32]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

fn digest(v: &[f32]) -> String {
    let bytes: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
    hex(sha256_bytes(&bytes))
}

fn gemm(id: CaseId) -> fandhe_silicon_core::parity::case_inputs::GemmCase {
    match id.input().unwrap() {
        CaseInput::Gemm(g) => g,
        other => panic!("{id:?} is not gemm: {other:?}"),
    }
}

#[test]
fn all_has_seventeen_unique_ids() {
    assert_eq!(CaseId::ALL.len(), 17);
    let names: Vec<&str> = CaseId::ALL.iter().map(|c| c.name()).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 17);
    assert_eq!(names[0], "g-01");
    assert_eq!(names[12], "g-13");
    assert_eq!(names[13], "e-1");
    assert_eq!(names[16], "e-4");
}

#[test]
fn ids_are_test_ids() {
    for c in CaseId::ALL {
        assert!(TestId::new(c.name()).is_ok(), "{}", c.name());
    }
}

#[test]
fn ops_and_shapes_are_fixed() {
    for c in CaseId::ALL {
        let input = c.input().unwrap();
        match (c, &input) {
            (CaseId::G10, CaseInput::Gemm(g)) => assert_eq!((g.m(), g.k(), g.n()), (1, 1, 1)),
            (CaseId::G12 | CaseId::E1, CaseInput::Gemm(g)) => {
                assert_eq!((g.m(), g.k(), g.n()), (37, 53, 41));
                assert_eq!((g.a().len(), g.b().len()), (37 * 53, 53 * 41));
            }
            (_, CaseInput::Gemm(g)) => {
                assert_eq!((g.m(), g.k(), g.n()), (1, 4, 1), "{c:?}");
                assert_eq!((g.a().len(), g.b().len()), (4, 4));
            }
            (CaseId::E2, CaseInput::Relu(v)) => assert_eq!(v.len(), 37),
            (CaseId::E3, CaseInput::TopK { scores, k }) => assert_eq!((scores.len(), *k), (37, 5)),
            (CaseId::E4, CaseInput::TopK { scores, k }) => assert_eq!((scores.len(), *k), (16, 8)),
            _ => panic!("unexpected kind for {c:?}"),
        }
    }
}

#[test]
fn output_lens_are_fixed() {
    for c in CaseId::ALL {
        let want = match c {
            CaseId::G12 | CaseId::E1 => 1517,
            CaseId::E2 => 37,
            CaseId::E3 => 5,
            CaseId::E4 => 8,
            _ => 1,
        };
        assert_eq!(c.input().unwrap().output_len().unwrap(), want, "{c:?}");
    }
}

#[test]
fn gemm_b_has_no_zero_or_non_finite_except_first() {
    for c in CaseId::ALL {
        let CaseInput::Gemm(g) = c.input().unwrap() else {
            continue;
        };
        let skip = usize::from(matches!(c, CaseId::G06 | CaseId::G07 | CaseId::G13));
        for (i, x) in g.b().iter().enumerate().skip(skip) {
            assert!(x.is_finite() && *x != 0.0, "{c:?} b[{i}]");
        }
    }
}

#[test]
fn gemm_design_matches_reference_class() {
    for c in CaseId::ALL {
        let CaseInput::Gemm(g) = c.input().unwrap() else {
            continue;
        };
        let out = reference_gemm_f32(g.a(), g.b(), g.m(), g.k(), g.n()).unwrap();
        let got: Vec<OutputClass> = out.iter().copied().map(classify).collect();
        assert_eq!(got, g.design(), "{c:?}");
        assert_eq!(g.design().len(), g.m() * g.n());
    }
}

#[test]
fn g12_differs_from_e1_only_in_replaced_rows() {
    let g = gemm(CaseId::G12);
    let e = gemm(CaseId::E1);
    assert_eq!(g.b(), e.b());
    let diff: Vec<usize> = (0..g.a().len())
        .filter(|&i| g.a()[i].to_bits() != e.a()[i].to_bits())
        .collect();
    assert!(diff.iter().all(|&i| i / 53 < 5));
    // 行 0〜2 は 1 要素ずつ、行 3・4 は全要素が置き換わる。
    assert!(diff.contains(&0) && diff.contains(&(53 + 1)) && diff.contains(&(2 * 53 + 2)));
    assert!((0..53).all(|c| diff.contains(&(3 * 53 + c)) && diff.contains(&(4 * 53 + c))));
    assert_eq!(diff.len(), 3 + 53 * 2);
}

#[test]
fn special_values_are_placed() {
    let CaseInput::Relu(v) = CaseId::E2.input().unwrap() else {
        panic!()
    };
    assert!(v[0].is_nan() && v[6].is_nan());
    assert_eq!(v[1], f32::INFINITY);
    assert_eq!(v[2], f32::NEG_INFINITY);
    assert_eq!(v[3].to_bits(), (-0.0f32).to_bits());
    assert_eq!(v[4].to_bits(), 0.0f32.to_bits());
    assert_eq!(v[5], 2.5 - 10.0);
    let CaseInput::TopK { scores, .. } = CaseId::E4.input().unwrap() else {
        panic!()
    };
    assert!(scores[0].is_nan());
    assert_eq!(scores[1], f32::INFINITY);
    assert_eq!(scores[2], f32::NEG_INFINITY);
    assert_eq!(scores[3].to_bits(), (-0.0f32).to_bits());
    assert_eq!(scores[4].to_bits(), 0.0f32.to_bits());
    assert!(scores[5..].iter().all(|x| *x == 1.0));
}

#[test]
fn classify_covers_five_classes() {
    assert_eq!(classify(f32::NAN), OutputClass::Nan);
    assert_eq!(classify(f32::from_bits(0xFFC0_0001)), OutputClass::Nan);
    assert_eq!(classify(f32::INFINITY), OutputClass::PosInf);
    assert_eq!(classify(f32::NEG_INFINITY), OutputClass::NegInf);
    assert_eq!(classify(0.0), OutputClass::Zero);
    assert_eq!(classify(-0.0), OutputClass::Zero);
    assert_eq!(classify(1e-40), OutputClass::NonzeroFinite);
    assert_eq!(classify(-3.5), OutputClass::NonzeroFinite);
}

/// 各ケースの入力（A→B の順、または Relu・Top-k のスコア）の f32 のビット列（LE）の sha256。
#[test]
fn inputs_are_fixed_by_sha256() {
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
