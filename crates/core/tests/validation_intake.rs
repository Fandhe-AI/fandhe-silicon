//! 検証の報告の受け口 `ValidationReporter` の確認（ERR-8。TASK-554 のポインタ）。
//!
//! 受け口は feature の条件なしで公開される。登録した関数へ `ValidationFailed` が届くこと、
//! 未登録・関数の実行中の報告を数えること、既存の完了状態の型を変えていないことを見る。

use std::sync::{Arc, Mutex};

use fandhe_silicon_core::error::{CompletionState, ContractError};
use fandhe_silicon_core::validation::{ValidationIntake, ValidationReporter};

fn collecting(r: &ValidationReporter) -> Arc<Mutex<Vec<ContractError>>> {
    let got = Arc::new(Mutex::new(Vec::new()));
    let got2 = Arc::clone(&got);
    r.set_validation_callback(Box::new(move |e| got2.lock().unwrap().push(e)));
    got
}

#[test]
fn report_reaches_callback_as_validation_failed() {
    let r = ValidationReporter::new();
    let got = collecting(&r);
    let returned = r.report("bad dispatch");
    let expected = ContractError::ValidationFailed("bad dispatch".to_string());
    assert_eq!(returned, expected);
    assert_eq!(*got.lock().unwrap(), vec![expected]);
    assert_eq!(r.delivered_count(), 1);
    assert_eq!(r.undelivered_count(), 0);
}

#[test]
fn every_report_is_delivered_in_order() {
    let r = ValidationReporter::new();
    let got = collecting(&r);
    r.report("a");
    r.report("b");
    r.report("c");
    let details: Vec<_> = got
        .lock()
        .unwrap()
        .iter()
        .map(|e| match e {
            ContractError::ValidationFailed(s) => s.clone(),
            _ => panic!("not a validation failure"),
        })
        .collect();
    assert_eq!(details, ["a", "b", "c"]);
    assert_eq!(r.delivered_count(), 3);
}

#[test]
fn report_without_callback_is_counted_and_still_returned() {
    let r = ValidationReporter::new();
    let returned = r.report("x");
    assert_eq!(returned, ContractError::ValidationFailed("x".to_string()));
    assert_eq!(r.delivered_count(), 0);
    assert_eq!(r.undelivered_count(), 1);
}

#[test]
fn registering_again_replaces_the_callback() {
    let r = ValidationReporter::new();
    let first = collecting(&r);
    r.report("one");
    let second = collecting(&r);
    r.report("two");
    assert_eq!(first.lock().unwrap().len(), 1);
    assert_eq!(second.lock().unwrap().len(), 1);
}

#[test]
fn report_during_callback_does_not_deadlock_and_is_counted_undelivered() {
    let r = Arc::new(ValidationReporter::new());
    let r2 = Arc::clone(&r);
    let calls = Arc::new(Mutex::new(0u32));
    let calls2 = Arc::clone(&calls);
    r.set_validation_callback(Box::new(move |_| {
        *calls2.lock().unwrap() += 1;
        // 実行中の入れ子の報告は届けない。
        r2.report("nested");
    }));
    r.report("outer");
    assert_eq!(*calls.lock().unwrap(), 1);
    assert_eq!(r.delivered_count(), 1);
    assert_eq!(r.undelivered_count(), 1);
    // 実行が終われば関数は残り、次の報告は届く。
    r.report("again");
    assert_eq!(*calls.lock().unwrap(), 2);
}

#[test]
fn intake_works_through_dyn_trait() {
    let r = ValidationReporter::new();
    let intake: &dyn ValidationIntake = &r;
    let seen = Arc::new(Mutex::new(0u32));
    let seen2 = Arc::clone(&seen);
    intake.set_validation_callback(Box::new(move |_| *seen2.lock().unwrap() += 1));
    r.report("x");
    assert_eq!(*seen.lock().unwrap(), 1);
}

/// 受け口を足しても `CompletionState` は 3 値のままで、`_` なしの `match` が通る
/// （`on_complete` の引数を変えないこと。ERR-8）。
#[test]
fn completion_state_is_unchanged_by_the_intake() {
    fn name(s: CompletionState) -> &'static str {
        match s {
            CompletionState::Pending => "pending",
            CompletionState::Complete => "complete",
            CompletionState::Lost => "lost",
        }
    }
    assert_eq!(name(CompletionState::Pending), "pending");
}

/// 登録した関数が panic しても登録は外れず、次の報告がその関数に届く。
#[test]
fn panicking_callback_stays_registered() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let r = ValidationReporter::new();
    let calls = Arc::new(Mutex::new(0u32));
    let calls2 = Arc::clone(&calls);
    r.set_validation_callback(Box::new(move |_| {
        let mut n = calls2.lock().unwrap();
        *n += 1;
        if *n == 1 {
            drop(n);
            panic!("callback failure");
        }
    }));
    let first = catch_unwind(AssertUnwindSafe(|| r.report("one")));
    assert!(first.is_err());
    r.report("two");
    assert_eq!(*calls.lock().unwrap(), 2);
}
