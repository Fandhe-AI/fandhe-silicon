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

/// Drop の中で同じ部品を呼ぶ関数を置き換えても、デッドロックしない（時間の上限つき）。
#[test]
fn replacing_a_callback_whose_drop_calls_the_reporter_does_not_deadlock() {
    use std::sync::mpsc;
    use std::time::Duration;

    struct CallsReporterOnDrop(Arc<ValidationReporter>);
    impl Drop for CallsReporterOnDrop {
        fn drop(&mut self) {
            let _ = self.0.delivered_count();
            self.0.set_callback(Box::new(|_| {}));
        }
    }

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let r = Arc::new(ValidationReporter::new());
        let guard = CallsReporterOnDrop(Arc::clone(&r));
        r.set_validation_callback(Box::new(move |_| {
            let _keep = &guard;
        }));
        r.set_validation_callback(Box::new(|_| {}));
        tx.send(()).unwrap();
    });
    assert!(rx.recv_timeout(Duration::from_secs(10)).is_ok());
}

/// 実行中に再登録した後、古い関数の終わりで新しい登録が上書きされない。
#[test]
fn re_registration_during_a_run_survives_the_old_callbacks_end() {
    use std::sync::Barrier;

    let r = Arc::new(ValidationReporter::new());
    let started = Arc::new(Barrier::new(2));
    let registered = Arc::new(Barrier::new(2));
    let (s2, g2) = (Arc::clone(&started), Arc::clone(&registered));
    r.set_validation_callback(Box::new(move |_| {
        s2.wait();
        g2.wait();
    }));
    let r2 = Arc::clone(&r);
    let t = std::thread::spawn(move || {
        r2.report("old");
    });
    started.wait();
    let new_calls = Arc::new(Mutex::new(0u32));
    let n2 = Arc::clone(&new_calls);
    r.set_validation_callback(Box::new(move |_| *n2.lock().unwrap() += 1));
    registered.wait();
    t.join().unwrap();
    r.report("after");
    assert_eq!(*new_calls.lock().unwrap(), 1);
}

/// 新しい関数が別の報告で実行中のとき、古い関数が先に終わっても、新しい関数が後から戻る。
#[test]
fn newer_callback_running_elsewhere_is_restored_after_the_old_one_ends() {
    use std::sync::Barrier;

    let r = Arc::new(ValidationReporter::new());
    let a_started = Arc::new(Barrier::new(2));
    let a_release = Arc::new(Barrier::new(2));
    let (s, g) = (Arc::clone(&a_started), Arc::clone(&a_release));
    r.set_validation_callback(Box::new(move |_| {
        s.wait();
        g.wait();
    }));
    let ra = Arc::clone(&r);
    let ta = std::thread::spawn(move || {
        ra.report("a");
    });
    a_started.wait();

    let b_started = Arc::new(Barrier::new(2));
    let b_release = Arc::new(Barrier::new(2));
    let b_calls = Arc::new(Mutex::new(0u32));
    let (bs, bg, bc) = (
        Arc::clone(&b_started),
        Arc::clone(&b_release),
        Arc::clone(&b_calls),
    );
    r.set_validation_callback(Box::new(move |_| {
        *bc.lock().unwrap() += 1;
        bs.wait();
        bg.wait();
    }));
    let rb = Arc::clone(&r);
    let tb = std::thread::spawn(move || {
        rb.report("b");
    });
    b_started.wait();
    // A を先に終わらせる。B はまだ実行中で、A は戻らず破棄される。
    a_release.wait();
    ta.join().unwrap();
    b_release.wait();
    tb.join().unwrap();
    // B が戻っているので、次の報告は B に届く。
    let rc = Arc::clone(&r);
    let tc = std::thread::spawn(move || {
        rc.report("c");
    });
    b_started.wait();
    b_release.wait();
    tc.join().unwrap();
    assert_eq!(*b_calls.lock().unwrap(), 2);
}
