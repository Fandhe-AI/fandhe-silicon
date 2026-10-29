//! ロストの通知先の関数の型 `LostCallback` の確認（DEV-136。TASK-138 のポインタ）。
//!
//! native では `Send` を要求し、wasm32 では要求しないことを型検査で見る。
//! native で `Send` でない関数が通らないことは `device_lost` モジュールの `compile_fail` の例が確かめる。
//! wasm32 側は `cargo check --target wasm32-unknown-unknown --test device_lost_notify` の手元の確かめ（実行はしない）。
//! CI で wasm32 の型検査が効くのは `wasm/crates/glue` の同名の試験（`make wasm-verify` の wasm32 の clippy）。

use std::cell::RefCell;
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use fandhe_silicon_core::device_lost::{DeviceLostNotify, LostCallback};
use fandhe_silicon_core::error::DeviceLostReason;

/// 登録された関数を保持し、`fire` で 1 回呼ぶだけの通知先。
#[derive(Default)]
struct Notifier {
    slot: RefCell<Option<LostCallback>>,
}

impl DeviceLostNotify for Notifier {
    fn set_lost_callback(&self, callback: LostCallback) {
        *self.slot.borrow_mut() = Some(callback);
    }
}

impl Notifier {
    fn fire(&self, reason: DeviceLostReason) {
        if let Some(cb) = self.slot.borrow_mut().take() {
            cb(reason);
        }
    }
}

#[test]
fn send_closure_is_accepted_through_dyn_and_receives_the_reason() {
    let n = Notifier::default();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen2 = Arc::clone(&seen);
    let dynamic: &dyn DeviceLostNotify = &n;
    dynamic.set_lost_callback(Box::new(move |r| seen2.lock().unwrap().push(r)));
    n.fire(DeviceLostReason::Destroyed);
    assert_eq!(*seen.lock().unwrap(), vec![DeviceLostReason::Destroyed]);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_callback_type_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<LostCallback>();
}

/// `Rc` を捕まえる（`Send` でない）関数が登録できること。wasm32 でだけコンパイルされる。
#[cfg(target_arch = "wasm32")]
#[test]
fn wasm32_accepts_a_closure_that_is_not_send() {
    let n = Notifier::default();
    let seen = Rc::new(RefCell::new(None));
    let seen2 = Rc::clone(&seen);
    n.set_lost_callback(Box::new(move |r| *seen2.borrow_mut() = Some(r)));
    n.fire(DeviceLostReason::Unknown);
    assert_eq!(*seen.borrow(), Some(DeviceLostReason::Unknown));
}
