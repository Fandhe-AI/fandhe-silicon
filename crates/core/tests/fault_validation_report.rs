//! 検証の報告を模す差し込みと、ロストの理由 `ValidationError` の経路の確認（ERR-8。TASK-554 のポインタ）。
//!
//! 報告は受け口へ `ValidationFailed` として届き、device をロストさせない。
//! `ValidationError` は、理由を指定した模擬のロストでだけ渡る（実際の報告からは作らない）。

use std::sync::{Arc, Mutex};

use fandhe_silicon_core::error::{ContractError, DeviceLostReason};
use fandhe_silicon_core::fault::decorator::{FaultInjecting, FaultScript};
use fandhe_silicon_core::fault::stub::FaultStub;
use fandhe_silicon_core::fault::{FaultHooks, HookFault, inject};
use fandhe_silicon_core::validation::ValidationIntake;

fn collect_reports(stub: &FaultStub) -> Arc<Mutex<Vec<ContractError>>> {
    let got = Arc::new(Mutex::new(Vec::new()));
    let got2 = Arc::clone(&got);
    stub.set_validation_callback(Box::new(move |e| got2.lock().unwrap().push(e)));
    got
}

fn collect_lost(stub: &FaultStub) -> Arc<Mutex<Vec<DeviceLostReason>>> {
    let got = Arc::new(Mutex::new(Vec::new()));
    let got2 = Arc::clone(&got);
    stub.set_lost_callback(Box::new(move |r| got2.lock().unwrap().push(r)));
    got
}

#[test]
fn injected_report_reaches_the_intake_as_validation_failed() {
    let stub = FaultStub::new();
    let got = collect_reports(&stub);
    inject(
        Some(&stub),
        HookFault::ValidationReport("layer says no".to_string()),
    )
    .unwrap();
    assert_eq!(
        *got.lock().unwrap(),
        vec![ContractError::ValidationFailed("layer says no".to_string())]
    );
    assert_eq!(stub.validation_delivered_count(), 1);
}

#[test]
fn report_alone_does_not_lose_the_device() {
    let stub = FaultStub::new();
    let _reports = collect_reports(&stub);
    let lost = collect_lost(&stub);
    stub.simulate_validation_report("r".to_string()).unwrap();
    assert!(!stub.is_lost());
    assert_eq!(stub.lost_reason(), None);
    assert!(lost.lock().unwrap().is_empty());
}

#[test]
fn validation_caused_loss_is_notified_as_validation_error() {
    let stub = FaultStub::new();
    let reports = collect_reports(&stub);
    let lost = collect_lost(&stub);
    inject(
        Some(&stub),
        HookFault::ValidationReport("cause".to_string()),
    )
    .unwrap();
    inject(
        Some(&stub),
        HookFault::DeviceLost(DeviceLostReason::ValidationError("cause".to_string())),
    )
    .unwrap();
    assert_eq!(reports.lock().unwrap().len(), 1);
    assert_eq!(
        *lost.lock().unwrap(),
        vec![DeviceLostReason::ValidationError("cause".to_string())]
    );
    assert_eq!(
        stub.lost_reason(),
        Some(DeviceLostReason::ValidationError("cause".to_string()))
    );
    assert_eq!(stub.lost_callback_count(), 1);
}

#[test]
fn a_real_style_loss_stays_unknown_even_after_a_report() {
    let stub = FaultStub::new();
    let reports = collect_reports(&stub);
    let lost = collect_lost(&stub);
    stub.simulate_validation_report("r".to_string()).unwrap();
    stub.simulate_device_lost(DeviceLostReason::Unknown)
        .unwrap();
    assert_eq!(reports.lock().unwrap().len(), 1);
    assert_eq!(*lost.lock().unwrap(), vec![DeviceLostReason::Unknown]);
}

/// 新しい口を知らない既存の実装は、変更なしでコンパイルでき、既定の実装が `Err` を返す。
#[test]
fn hooks_without_the_new_method_compile_and_default_to_error() {
    struct OldStyle;
    impl FaultHooks for OldStyle {
        fn arm_gate(&self) -> Result<(), ContractError> {
            Ok(())
        }
    }
    assert_eq!(
        inject(Some(&OldStyle), HookFault::ValidationReport("x".into())),
        Err(ContractError::UnsupportedExtensionDefault {
            op: "simulate_validation_report"
        })
    );
}

#[test]
fn decorator_forwards_the_report_to_the_inner_hooks() {
    let inner = FaultStub::new();
    let got = collect_reports(&inner);
    let d = FaultInjecting::new(inner, FaultScript::PassThrough);
    inject(Some(&d), HookFault::ValidationReport("fwd".to_string())).unwrap();
    assert_eq!(
        *got.lock().unwrap(),
        vec![ContractError::ValidationFailed("fwd".to_string())]
    );
}
