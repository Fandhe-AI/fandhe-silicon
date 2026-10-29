//! 検証レイヤーの不正を利用側へ届ける受け口（ERR-8。TASK-554 のポインタ）。
//!
//! 背後実装（下-1）が検証レイヤーの報告を受けたとき、`ValidationReporter::report` に説明を渡す。
//! 利用側（下-2・上）は `ValidationIntake::set_validation_callback` で登録した関数から
//! `ContractError::ValidationFailed(String)` として受け取る。
//! `on_complete` の引数（`CompletionState`）は変えない。名前は仮。
//!
//! 報告だけでは device はロストしない。`DeviceLostReason::ValidationError(String)` は、
//! `fault` モジュールの `simulate_device_lost` が、指定された理由として渡す。

use std::sync::{Mutex, MutexGuard};

use crate::error::ContractError;

/// 検証の報告を受ける関数。報告ごとに `ValidationFailed` を持つ `ContractError` を渡して呼ばれる。
#[cfg(not(target_arch = "wasm32"))]
pub type ValidationCallback = Box<dyn FnMut(ContractError) + Send>;

/// `ValidationCallback` の wasm32 版。単一スレッドなので `Send` を要求しない。
#[cfg(target_arch = "wasm32")]
pub type ValidationCallback = Box<dyn FnMut(ContractError)>;

/// 検証の報告の受け口を持つ側が実装するトレイト。`dyn` で使える。
pub trait ValidationIntake {
    /// 検証の報告を受ける関数を登録する。登録済みなら置き換える。
    fn set_validation_callback(&self, callback: ValidationCallback);
}

#[derive(Default)]
struct State {
    callback: Option<ValidationCallback>,
    delivered: u64,
    undelivered: u64,
}

/// 受け口の部品。背後実装や試験のスタブが持ち、`ValidationIntake` の実装の中身にする。
#[derive(Default)]
pub struct ValidationReporter {
    state: Mutex<State>,
}

impl ValidationReporter {
    /// 関数が未登録の状態で作る。
    pub fn new() -> Self {
        Self::default()
    }

    /// 毒化していても中身を取り出す。panic しない。
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 関数を登録する。登録済みなら置き換える。
    pub fn set_callback(&self, callback: ValidationCallback) {
        self.lock().callback = Some(callback);
    }

    /// 検証の報告を 1 件受ける。`ValidationFailed(detail)` を作り、登録された関数があれば 1 回呼ぶ。
    /// 戻り値は同じ値の複製で、`Result` を返す操作（`wait_blocking` など）がそのまま返せる。
    ///
    /// 関数は lock の外で呼ぶので、関数の中から他の操作を呼んでもデッドロックしない。
    /// 関数の実行中に起きた報告と、関数が未登録のときの報告は届けず、`undelivered_count` に数える。
    pub fn report(&self, detail: impl Into<String>) -> ContractError {
        let err = ContractError::ValidationFailed(detail.into());
        let cb = {
            let mut st = self.lock();
            let cb = st.callback.take();
            if cb.is_some() {
                st.delivered += 1;
            } else {
                st.undelivered += 1;
            }
            cb
        };
        if let Some(cb) = cb {
            let mut guard = RestoreGuard {
                reporter: self,
                cb: Some(cb),
            };
            if let Some(cb) = guard.cb.as_mut() {
                cb(err.clone());
            }
        }
        err
    }

    /// 関数へ届けた報告の数。
    pub fn delivered_count(&self) -> u64 {
        self.lock().delivered
    }

    /// 届けなかった報告の数（未登録、または関数の実行中に起きたもの）。
    pub fn undelivered_count(&self) -> u64 {
        self.lock().undelivered
    }
}

/// 呼んでいる間だけ取り出した関数を、抜けるとき（panic の巻き戻しを含む）に登録へ戻す。
/// 呼んでいる間に新しい関数が登録されていたら、そちらを残す。
struct RestoreGuard<'a> {
    reporter: &'a ValidationReporter,
    cb: Option<ValidationCallback>,
}

impl Drop for RestoreGuard<'_> {
    fn drop(&mut self) {
        let mut st = self.reporter.lock();
        if st.callback.is_none() {
            st.callback = self.cb.take();
        }
    }
}

impl ValidationIntake for ValidationReporter {
    fn set_validation_callback(&self, callback: ValidationCallback) {
        self.set_callback(callback);
    }
}
