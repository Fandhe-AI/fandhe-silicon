//! 契約の crate 側で、同一 GPU の判定 `check_same_physical_gpu` の組ごとの結果を確かめる（INTEROP-7・INTEROP-10。TASK-719 のポインタ）。
//!
//! `fandhe-silicon-core` の `tests/interop_same_gpu.rs` の同じ組の試験を、契約の crate の側から行う。
//! 判定の関数と型は core のもので、この crate は再公開しないため core から直接取り込む。機材には触れない。

use fandhe_silicon_core::error::{ContractError, IdentityMismatchReason};
use fandhe_silicon_core::interop::{PhysicalGpuId, check_same_physical_gpu};

fn with_driver(driver_uuid: Option<[u8; 16]>) -> PhysicalGpuId {
    PhysicalGpuId {
        device_uuid: [7; 16],
        driver_uuid,
    }
}

fn mismatch(reason: IdentityMismatchReason) -> ContractError {
    ContractError::DeviceIdentityMismatch { reason }
}

#[test]
fn all_bytes_different_device_uuid_is_mismatch() {
    let a = PhysicalGpuId {
        device_uuid: [1; 16],
        driver_uuid: None,
    };
    let b = PhysicalGpuId {
        device_uuid: [2; 16],
        driver_uuid: None,
    };
    let expected = mismatch(IdentityMismatchReason::Mismatch);
    assert_eq!(
        check_same_physical_gpu(Some(&a), Some(&b)).unwrap_err(),
        expected
    );
    assert_eq!(
        check_same_physical_gpu(Some(&b), Some(&a)).unwrap_err(),
        expected
    );
}

/// INTEROP-10: 第 1 引数だけが `None` なら `Undeterminable`。
#[test]
fn first_argument_none_is_undeterminable() {
    let right = with_driver(None);
    let err = check_same_physical_gpu(None, Some(&right)).unwrap_err();
    assert_eq!(err, mismatch(IdentityMismatchReason::Undeterminable));
}

/// INTEROP-10: 第 2 引数だけが `None` なら `Undeterminable`。
#[test]
fn second_argument_none_is_undeterminable() {
    let left = with_driver(None);
    let err = check_same_physical_gpu(Some(&left), None).unwrap_err();
    assert_eq!(err, mismatch(IdentityMismatchReason::Undeterminable));
}

/// INTEROP-10: 両方の引数が `None` でも `Undeterminable`。
#[test]
fn both_arguments_none_is_undeterminable() {
    let err = check_same_physical_gpu(None, None).unwrap_err();
    assert_eq!(err, mismatch(IdentityMismatchReason::Undeterminable));
}
