//! device のロストの通知先を登録する口（DEV-136。TASK-138 のポインタ）。
//!
//! 背後実装の device や試験のスタブが `DeviceLostNotify` を実装し、利用側（下-2・上）が
//! `set_lost_callback` で通知先の関数を登録する。関数に渡す理由は `error::DeviceLostReason`。
//! 関数の型 `LostCallback` は `cfg` で出し分ける。native は `Send` を求め、wasm32 は求めない。
//!
//! 型の確認は `tests/device_lost_notify.rs`。wasm32 の型検査が CI で効くのは `wasm/` の glue の試験
//! （`make wasm-verify` の wasm32 の clippy）で、core 側の同名の試験は
//! `cargo check --target wasm32-unknown-unknown --test device_lost_notify` の手元の確かめ。
//!
//! native では `Send` でない関数を渡すとコンパイルできない。
//!
//! ```
//! use std::sync::{Arc, Mutex};
//! use fandhe_silicon_core::device_lost::{DeviceLostNotify, LostCallback};
//!
//! fn register(n: &dyn DeviceLostNotify) {
//!     let seen = Arc::new(Mutex::new(0_u32));
//!     let cb: LostCallback = Box::new(move |_| *seen.lock().unwrap() += 1);
//!     n.set_lost_callback(cb);
//! }
//! # let _ = register;
//! ```
//!
//! 同じ形で `Rc` を捕まえる関数にすると、native ではコンパイルできない。
//!
//! ```compile_fail
//! use std::rc::Rc;
//! use fandhe_silicon_core::device_lost::{DeviceLostNotify, LostCallback};
//!
//! fn register(n: &dyn DeviceLostNotify) {
//!     let seen = Rc::new(0_u32);
//!     let cb: LostCallback = Box::new(move |_| drop(seen));
//!     n.set_lost_callback(cb);
//! }
//! ```

use crate::error::DeviceLostReason;

/// ロストを受ける関数。ロストが起きたとき、理由を渡して 1 回だけ呼ばれる。native は `Send` を要求する。
#[cfg(not(target_arch = "wasm32"))]
pub type LostCallback = Box<dyn FnOnce(DeviceLostReason) + Send>;

/// `LostCallback` の wasm32 版。単一スレッドなので `Send` を要求しない。
#[cfg(target_arch = "wasm32")]
pub type LostCallback = Box<dyn FnOnce(DeviceLostReason)>;

/// ロストの通知先を登録できる側が実装するトレイト。`dyn` で使える。
pub trait DeviceLostNotify {
    /// ロストを受ける関数を登録する。登録済みなら置き換える。
    fn set_lost_callback(&self, callback: LostCallback);
}
