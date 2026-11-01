//! 契約の crate 側で、同一 GPU の判定 `check_same_physical_gpu` の組ごとの結果を確かめる（INTEROP-7〜INTEROP-10・INTEROP-65。TASK-719 のポインタ）。
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

/// INTEROP-8: `device_uuid` が同じで両方の `driver_uuid` が `Some` かつ違えば `Mismatch`。
#[test]
fn differing_driver_uuids_are_mismatch() {
    let (a, b) = (with_driver(Some([1; 16])), with_driver(Some([2; 16])));
    let err = check_same_physical_gpu(Some(&a), Some(&b)).unwrap_err();
    assert_eq!(err, mismatch(IdentityMismatchReason::Mismatch));
}

/// INTEROP-9: `driver_uuid` が片方だけ `None` なら（左右どちらでも）比べず `Ok`。
#[test]
fn one_sided_missing_driver_uuid_is_accepted() {
    let (some, none) = (with_driver(Some([1; 16])), with_driver(None));
    assert!(check_same_physical_gpu(Some(&some), Some(&none)).is_ok());
    assert!(check_same_physical_gpu(Some(&none), Some(&some)).is_ok());
}

/// INTEROP-65: 両方の `driver_uuid` が `None` なら `Ok`。
#[test]
fn both_missing_driver_uuid_is_accepted() {
    let none = with_driver(None);
    assert!(check_same_physical_gpu(Some(&none), Some(&none)).is_ok());
}

/// INTEROP-7: `device_uuid` の 16 バイトすべてが違う組は、引数の順によらず `Mismatch`。
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
