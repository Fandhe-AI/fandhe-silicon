//! フィールドを持つエラー値の読み戻しを、別の crate から照合する公開テスト（TASK-269 の 5/8）。
//!
//! 仕組み: 構築した値を、全フィールドを `..` なしで束縛して読み戻す。フィールドが増える・
//! 名前が変わると束縛がコンパイルで落ちる（ERR-53。ポインタのみ）。1 フィールドだけ変えた値とは
//! 等しくならないことも見て、`PartialEq` に各フィールドが効いていることを確かめる。
//!
//! 対象外: `{:?}` の書式と `ValidationFailed` の定義の確かめ、`Display`・`Error`、
//! `capability` モジュールの 3 型の拡張性（TASK-136 の持ち分。ポインタのみ）。

use fandhe_silicon_core::capability::{BackendKind, CapabilityLevel, FeatureId};
use fandhe_silicon_core::error::{ContractError, DeviceLostReason, IdentityMismatchReason};

const BACKENDS: [BackendKind; 4] = [
    BackendKind::Metal,
    BackendKind::Vulkan,
    BackendKind::Cuda,
    BackendKind::Dx12Compute,
];

const MULTILINE: &str = "line one\nline two: 失敗 \u{1F980}\n";

/// 文字列 1 つを持つタプル値の中身を取り出す。他の値なら panic する。
fn string_payload(v: ContractError) -> String {
    match v {
        ContractError::InvalidCombination(s)
        | ContractError::DeviceUnavailable(s)
        | ContractError::InvalidArgument(s)
        | ContractError::ValidationFailed(s) => s,
        other => panic!("not a string-carrying value: {other:?}"),
    }
}

#[test]
fn string_tuple_values_round_trip() {
    for text in ["", "plain", MULTILINE] {
        assert_eq!(
            string_payload(ContractError::InvalidCombination(text.to_string())),
            text
        );
        assert_eq!(
            string_payload(ContractError::DeviceUnavailable(text.to_string())),
            text
        );
        assert_eq!(
            string_payload(ContractError::InvalidArgument(text.to_string())),
            text
        );
        assert_eq!(
            string_payload(ContractError::ValidationFailed(text.to_string())),
            text
        );
    }
}

#[test]
fn device_lost_generation_round_trips() {
    for g in [0u64, 7, u64::MAX] {
        let v = ContractError::DeviceLost { generation: g };
        let ContractError::DeviceLost { generation } = v else {
            panic!("expected DeviceLost");
        };
        assert_eq!(generation, g);
    }
}

#[test]
fn allocation_failed_bytes_round_trip() {
    for n in [0u64, 16, u64::MAX] {
        let v = ContractError::AllocationFailed { requested_bytes: n };
        let ContractError::AllocationFailed { requested_bytes } = v else {
            panic!("expected AllocationFailed");
        };
        assert_eq!(requested_bytes, n);
    }
}

#[test]
fn artifact_rejected_fields_round_trip() {
    let v = ContractError::ArtifactRejected {
        kind: "test-kind",
        reason: MULTILINE.to_string(),
    };
    let ContractError::ArtifactRejected { kind, reason } = v else {
        panic!("expected ArtifactRejected");
    };
    assert_eq!(kind, "test-kind");
    assert_eq!(reason, MULTILINE);
}

#[test]
fn unsupported_fields_round_trip_for_every_backend() {
    for b in BACKENDS {
        let v = ContractError::Unsupported {
            op: "test-op",
            backend: b,
        };
        let ContractError::Unsupported { op, backend } = v else {
            panic!("expected Unsupported");
        };
        assert_eq!(op, "test-op");
        assert_eq!(backend, b);
    }
}

#[test]
fn capability_insufficient_fields_round_trip() {
    let pairs = [
        (CapabilityLevel::S3, CapabilityLevel::S1),
        (CapabilityLevel::S1, CapabilityLevel::S3),
        (CapabilityLevel::S0, CapabilityLevel::Unsupported),
        (CapabilityLevel::Unsupported, CapabilityLevel::S0),
    ];
    for (req, act) in pairs {
        let v = ContractError::CapabilityInsufficient {
            feature: FeatureId("test-feature"),
            required: req,
            actual: act,
        };
        let ContractError::CapabilityInsufficient {
            feature,
            required,
            actual,
        } = v
        else {
            panic!("expected CapabilityInsufficient");
        };
        assert_eq!(feature.0, "test-feature");
        assert_eq!(required, req);
        assert_eq!(actual, act);
    }
}

#[test]
fn device_identity_mismatch_reason_round_trips() {
    for r in [
        IdentityMismatchReason::Mismatch,
        IdentityMismatchReason::Undeterminable,
    ] {
        let v = ContractError::DeviceIdentityMismatch { reason: r };
        let ContractError::DeviceIdentityMismatch { reason } = v else {
            panic!("expected DeviceIdentityMismatch");
        };
        assert_eq!(reason, r);
    }
}

#[test]
fn unsupported_extension_default_op_round_trips() {
    let v = ContractError::UnsupportedExtensionDefault { op: "test-op" };
    let ContractError::UnsupportedExtensionDefault { op } = v else {
        panic!("expected UnsupportedExtensionDefault");
    };
    assert_eq!(op, "test-op");
}

#[test]
fn backend_failure_fields_round_trip() {
    for b in BACKENDS {
        let v = ContractError::BackendFailure {
            op: "test-op",
            backend: b,
            detail: MULTILINE.to_string(),
        };
        let ContractError::BackendFailure {
            op,
            backend,
            detail,
        } = v
        else {
            panic!("expected BackendFailure");
        };
        assert_eq!(op, "test-op");
        assert_eq!(backend, b);
        assert_eq!(detail, MULTILINE);
    }
}

#[test]
fn capacity_exceeded_fields_round_trip() {
    for b in BACKENDS {
        for c in [0u32, 5, u32::MAX] {
            let v = ContractError::CapacityExceeded {
                op: "test-op",
                backend: b,
                capacity: c,
            };
            let ContractError::CapacityExceeded {
                op,
                backend,
                capacity,
            } = v
            else {
                panic!("expected CapacityExceeded");
            };
            assert_eq!(op, "test-op");
            assert_eq!(backend, b);
            assert_eq!(capacity, c);
        }
    }
}

#[test]
fn device_lost_reason_strings_round_trip() {
    let DeviceLostReason::ValidationError(a) = DeviceLostReason::ValidationError(MULTILINE.into())
    else {
        panic!("expected ValidationError");
    };
    assert_eq!(a, MULTILINE);
    let DeviceLostReason::Unrecoverable(b) = DeviceLostReason::Unrecoverable(MULTILINE.into())
    else {
        panic!("expected Unrecoverable");
    };
    assert_eq!(b, MULTILINE);
    assert_ne!(
        DeviceLostReason::ValidationError("x".into()),
        DeviceLostReason::Unrecoverable("x".into())
    );
}

#[test]
fn changing_one_field_breaks_equality() {
    let base = ContractError::CapabilityInsufficient {
        feature: FeatureId("f"),
        required: CapabilityLevel::S3,
        actual: CapabilityLevel::S1,
    };
    assert_eq!(base, base.clone());
    for other in [
        ContractError::CapabilityInsufficient {
            feature: FeatureId("g"),
            required: CapabilityLevel::S3,
            actual: CapabilityLevel::S1,
        },
        ContractError::CapabilityInsufficient {
            feature: FeatureId("f"),
            required: CapabilityLevel::S2,
            actual: CapabilityLevel::S1,
        },
        ContractError::CapabilityInsufficient {
            feature: FeatureId("f"),
            required: CapabilityLevel::S3,
            actual: CapabilityLevel::S2,
        },
    ] {
        assert_ne!(base, other);
    }

    let base = ContractError::BackendFailure {
        op: "o",
        backend: BackendKind::Metal,
        detail: "d".into(),
    };
    for other in [
        ContractError::BackendFailure {
            op: "p",
            backend: BackendKind::Metal,
            detail: "d".into(),
        },
        ContractError::BackendFailure {
            op: "o",
            backend: BackendKind::Cuda,
            detail: "d".into(),
        },
        ContractError::BackendFailure {
            op: "o",
            backend: BackendKind::Metal,
            detail: "e".into(),
        },
    ] {
        assert_ne!(base, other);
    }

    let base = ContractError::CapacityExceeded {
        op: "o",
        backend: BackendKind::Vulkan,
        capacity: 4,
    };
    for other in [
        ContractError::CapacityExceeded {
            op: "p",
            backend: BackendKind::Vulkan,
            capacity: 4,
        },
        ContractError::CapacityExceeded {
            op: "o",
            backend: BackendKind::Cuda,
            capacity: 4,
        },
        ContractError::CapacityExceeded {
            op: "o",
            backend: BackendKind::Vulkan,
            capacity: 5,
        },
    ] {
        assert_ne!(base, other);
    }
}

/// フィールドを持つ 13 値を 1 つの配列に並べ、件数と、各値が読み戻しのどれかに当たることを見る。
#[test]
fn all_field_carrying_values_are_covered() {
    let all = [
        ContractError::InvalidCombination("a".into()),
        ContractError::DeviceUnavailable("a".into()),
        ContractError::DeviceLost { generation: 1 },
        ContractError::InvalidArgument("a".into()),
        ContractError::AllocationFailed { requested_bytes: 1 },
        ContractError::ArtifactRejected {
            kind: "k",
            reason: "r".into(),
        },
        ContractError::ValidationFailed("a".into()),
        ContractError::Unsupported {
            op: "o",
            backend: BackendKind::Metal,
        },
        ContractError::CapabilityInsufficient {
            feature: FeatureId("f"),
            required: CapabilityLevel::S1,
            actual: CapabilityLevel::S0,
        },
        ContractError::DeviceIdentityMismatch {
            reason: IdentityMismatchReason::Mismatch,
        },
        ContractError::UnsupportedExtensionDefault { op: "o" },
        ContractError::BackendFailure {
            op: "o",
            backend: BackendKind::Cuda,
            detail: "d".into(),
        },
        ContractError::CapacityExceeded {
            op: "o",
            backend: BackendKind::Vulkan,
            capacity: 1,
        },
    ];
    assert_eq!(all.len(), 13);
    for v in all {
        match v {
            ContractError::InvalidCombination(_)
            | ContractError::DeviceUnavailable(_)
            | ContractError::InvalidArgument(_)
            | ContractError::ValidationFailed(_)
            | ContractError::DeviceLost { .. }
            | ContractError::AllocationFailed { .. }
            | ContractError::ArtifactRejected { .. }
            | ContractError::Unsupported { .. }
            | ContractError::CapabilityInsufficient { .. }
            | ContractError::DeviceIdentityMismatch { .. }
            | ContractError::UnsupportedExtensionDefault { .. }
            | ContractError::BackendFailure { .. }
            | ContractError::CapacityExceeded { .. } => {}
            other => panic!("unexpected field-less value: {other:?}"),
        }
    }
}
