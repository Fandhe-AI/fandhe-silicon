//! 捕捉されなかったエラーの通知の口と、エラーの範囲の種類の型と、それを支える状態を持つ部品の置き場（TASK-140 のポインタ）。
//!
//! 戻り値の経路が無いエラーは、`UncapturedErrorNotify` で登録した関数へ `ContractError` として渡す。
//! 関数が未登録のあいだは `ErrorRouter` が上限つきで値を持ち、`take_uncaptured_errors` で読める。
//! 背後実装の API は呼ばない。名前は仮。

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard};

use crate::error::ContractError;

/// エラーの範囲の種類。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorFilter {
    /// 検証の種類。
    Validation,
    /// メモリ不足の種類。
    OutOfMemory,
    /// 内部の種類。
    Internal,
}

/// 捕捉されなかったエラーを受ける関数。エラーごとに `ContractError` を渡して呼ばれる。
#[cfg(not(target_arch = "wasm32"))]
pub type UncapturedErrorCallback = Box<dyn FnMut(ContractError) + Send>;

/// `UncapturedErrorCallback` の wasm32 版。単一スレッドなので `Send` を要求しない。
#[cfg(target_arch = "wasm32")]
pub type UncapturedErrorCallback = Box<dyn FnMut(ContractError)>;

/// 関数が未登録のあいだ持っておく値と、次の呼び出しで関数へ渡す列の、合わせた件数の上限。
pub const UNCAPTURED_RETENTION_LIMIT: usize = 64;

/// 捕捉されなかったエラーの口を持つ側が実装するトレイト。`dyn` で使える。
pub trait UncapturedErrorNotify {
    /// 捕捉されなかったエラーを受ける関数を登録する。登録済みなら置き換える。
    fn set_uncaptured_error_callback(&self, callback: UncapturedErrorCallback);

    /// 持っていた値を古い順に返し、上限を超えて数えただけの件数を返す。どちらも空にする。
    fn take_uncaptured_errors(&self) -> (Vec<ContractError>, u64);
}

#[derive(Default)]
struct State {
    callback: Option<UncapturedErrorCallback>,
    /// 登録のたびに増える世代。
    generation: u64,
    /// 関数を取り出して呼んでいる間 true。
    in_call: bool,
    /// 未登録のあいだに持った値（`take` で読める）。
    retained: VecDeque<ContractError>,
    /// 次の呼び出しで関数へ渡す列（`take` では読めない）。
    pending: VecDeque<ContractError>,
    /// 上限を超えて値を持たずに数えた件数。
    overflow: u64,
}

impl State {
    fn is_full(&self) -> bool {
        self.retained.len() + self.pending.len() >= UNCAPTURED_RETENTION_LIMIT
    }

    /// 上限に余裕があれば積み、無ければ件数だけ数える。
    fn push_bounded(&mut self, to_pending: bool, error: ContractError) {
        if self.is_full() {
            self.overflow = self.overflow.saturating_add(1);
        } else if to_pending {
            self.pending.push_back(error);
        } else {
            self.retained.push_back(error);
        }
    }
}

/// 口の部品。背後実装や試験のスタブが持ち、`UncapturedErrorNotify` の実装の中身にする。
#[derive(Default)]
pub struct ErrorRouter {
    state: Mutex<State>,
}

impl ErrorRouter {
    /// 関数が未登録の状態で作る。
    pub fn new() -> Self {
        Self::default()
    }

    /// 毒化していても中身を取り出す。panic しない。
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 戻り値の経路が無いエラーを 1 件受ける。`DeviceLost` は何もしない。
    ///
    /// 関数が登録済みで実行中でなければ、次の呼び出しで渡す列を古い順に先に渡してから、
    /// lock の外で関数を呼ぶ。関数の実行中は次の呼び出しで渡す列に積み、関数は呼ばない。
    /// 未登録なら持っておく列に積む。
    pub fn deliver_uncaptured(&self, error: ContractError) {
        if matches!(error, ContractError::DeviceLost { .. }) {
            return;
        }
        let (cb, generation, queue) = {
            let mut st = self.lock();
            if st.in_call {
                st.push_bounded(true, error);
                return;
            }
            match st.callback.take() {
                Some(cb) => {
                    st.in_call = true;
                    let mut queue = std::mem::take(&mut st.pending);
                    queue.push_back(error);
                    (cb, st.generation, queue)
                }
                None => {
                    st.push_bounded(false, error);
                    return;
                }
            }
        };
        self.run(cb, generation, queue);
    }

    /// 次の呼び出しで渡す列を古い順に、登録された関数へ lock の外で渡す。
    /// 関数の実行中は何もしない。未登録なら、その列を持っておく列の末尾へ移す。
    pub fn deliver_pending(&self) {
        let (cb, generation, queue) = {
            let mut st = self.lock();
            if st.in_call || st.pending.is_empty() {
                return;
            }
            match st.callback.take() {
                Some(cb) => {
                    st.in_call = true;
                    (cb, st.generation, std::mem::take(&mut st.pending))
                }
                None => {
                    let moved = std::mem::take(&mut st.pending);
                    st.retained.extend(moved);
                    return;
                }
            }
        };
        self.run(cb, generation, queue);
    }

    /// 取り出した関数で列を古い順に呼ぶ。抜けるとき（panic の巻き戻しを含む）は guard が状態を戻す。
    fn run(&self, cb: UncapturedErrorCallback, generation: u64, queue: VecDeque<ContractError>) {
        let mut guard = RestoreGuard {
            router: self,
            generation,
            cb: Some(cb),
            queue,
        };
        while let Some(error) = guard.queue.pop_front() {
            if let Some(cb) = guard.cb.as_mut() {
                cb(error);
            }
        }
    }
}

/// 呼んでいる間だけ取り出した関数と未配達の列を、抜けるときに状態へ戻す。
/// 呼び切れなかった値は次の呼び出しで渡す列の先頭へ順を保って戻し、上限を超える分は件数に数える。
/// 取り出した時点から登録の世代が変わっていたら関数は戻さず、lock を手放してから破棄する。
struct RestoreGuard<'a> {
    router: &'a ErrorRouter,
    generation: u64,
    cb: Option<UncapturedErrorCallback>,
    queue: VecDeque<ContractError>,
}

impl Drop for RestoreGuard<'_> {
    fn drop(&mut self) {
        let leftover_cb = {
            let mut st = self.router.lock();
            st.in_call = false;
            if !self.queue.is_empty() {
                let mut merged = std::mem::take(&mut self.queue);
                merged.append(&mut st.pending);
                let room = UNCAPTURED_RETENTION_LIMIT.saturating_sub(st.retained.len());
                if merged.len() > room {
                    let dropped = (merged.len() - room) as u64;
                    st.overflow = st.overflow.saturating_add(dropped);
                    merged.truncate(room);
                }
                st.pending = merged;
            }
            if st.generation == self.generation && st.callback.is_none() {
                st.callback = self.cb.take();
                None
            } else {
                self.cb.take()
            }
        };
        drop(leftover_cb);
    }
}

impl UncapturedErrorNotify for ErrorRouter {
    fn set_uncaptured_error_callback(&self, callback: UncapturedErrorCallback) {
        let old = {
            let mut st = self.lock();
            st.generation += 1;
            st.callback.replace(callback)
        };
        drop(old);
    }

    fn take_uncaptured_errors(&self) -> (Vec<ContractError>, u64) {
        let mut st = self.lock();
        let values = std::mem::take(&mut st.retained).into_iter().collect();
        let count = std::mem::take(&mut st.overflow);
        (values, count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn err(s: &str) -> ContractError {
        ContractError::ValidationFailed(s.into())
    }

    type Log = Arc<Mutex<Vec<ContractError>>>;

    fn recorder() -> (Log, UncapturedErrorCallback) {
        let log: Log = Arc::default();
        let l = Arc::clone(&log);
        (log, Box::new(move |e| l.lock().unwrap().push(e)))
    }

    #[test]
    fn unregistered_reports_are_retained_oldest_first() {
        let r = ErrorRouter::new();
        r.deliver_uncaptured(err("a"));
        r.deliver_uncaptured(err("b"));
        assert_eq!(r.take_uncaptured_errors(), (vec![err("a"), err("b")], 0));
        assert_eq!(r.take_uncaptured_errors(), (vec![], 0));
    }

    #[test]
    fn retention_is_bounded_and_overflow_is_counted() {
        assert_eq!(UNCAPTURED_RETENTION_LIMIT, 64);
        let r = ErrorRouter::new();
        for i in 0..65 {
            r.deliver_uncaptured(err(&i.to_string()));
        }
        let (values, count) = r.take_uncaptured_errors();
        assert_eq!(values.len(), 64);
        assert_eq!(values[0], err("0"));
        assert_eq!(count, 1);
        assert_eq!(r.take_uncaptured_errors(), (vec![], 0));
    }

    #[test]
    fn registered_callback_receives_and_nothing_is_retained() {
        let r = ErrorRouter::new();
        let (log, cb) = recorder();
        r.set_uncaptured_error_callback(cb);
        r.deliver_uncaptured(err("a"));
        r.deliver_uncaptured(err("b"));
        assert_eq!(*log.lock().unwrap(), vec![err("a"), err("b")]);
        assert_eq!(r.take_uncaptured_errors(), (vec![], 0));
    }

    #[test]
    fn retained_before_registration_is_not_handed_over() {
        let r = ErrorRouter::new();
        r.deliver_uncaptured(err("old"));
        let (log, cb) = recorder();
        r.set_uncaptured_error_callback(cb);
        assert!(log.lock().unwrap().is_empty());
        assert_eq!(r.take_uncaptured_errors(), (vec![err("old")], 0));
    }

    #[test]
    fn later_registration_replaces_earlier() {
        let r = ErrorRouter::new();
        let (first, cb1) = recorder();
        let (second, cb2) = recorder();
        r.set_uncaptured_error_callback(cb1);
        r.set_uncaptured_error_callback(cb2);
        r.deliver_uncaptured(err("a"));
        assert!(first.lock().unwrap().is_empty());
        assert_eq!(*second.lock().unwrap(), vec![err("a")]);
    }

    #[test]
    fn device_lost_goes_nowhere() {
        let r = ErrorRouter::new();
        r.deliver_uncaptured(ContractError::DeviceLost { generation: 1 });
        assert_eq!(r.take_uncaptured_errors(), (vec![], 0));
        let (log, cb) = recorder();
        r.set_uncaptured_error_callback(cb);
        r.deliver_uncaptured(ContractError::DeviceLost { generation: 1 });
        r.deliver_pending();
        assert!(log.lock().unwrap().is_empty());
        assert_eq!(r.take_uncaptured_errors(), (vec![], 0));
    }

    #[test]
    fn report_from_inside_callback_waits_for_next_delivery() {
        let r = Arc::new(ErrorRouter::new());
        let log: Log = Arc::default();
        let (rc, l) = (Arc::clone(&r), Arc::clone(&log));
        r.set_uncaptured_error_callback(Box::new(move |e| {
            let first = l.lock().unwrap().is_empty();
            l.lock().unwrap().push(e);
            if first {
                rc.deliver_uncaptured(err("inner"));
            }
        }));
        r.deliver_uncaptured(err("outer"));
        assert_eq!(*log.lock().unwrap(), vec![err("outer")]);
        assert_eq!(r.take_uncaptured_errors(), (vec![], 0));
        r.deliver_pending();
        assert_eq!(*log.lock().unwrap(), vec![err("outer"), err("inner")]);
        r.deliver_pending();
        assert_eq!(log.lock().unwrap().len(), 2);
    }

    #[test]
    fn registration_survives_a_panicking_callback() {
        let r = ErrorRouter::new();
        let log: Log = Arc::default();
        let l = Arc::clone(&log);
        r.set_uncaptured_error_callback(Box::new(move |e| {
            if e == err("boom") {
                panic!("callback panic");
            }
            l.lock().unwrap().push(e);
        }));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            r.deliver_uncaptured(err("boom"));
        }));
        assert!(result.is_err());
        r.deliver_uncaptured(err("after"));
        assert_eq!(*log.lock().unwrap(), vec![err("after")]);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn router_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ErrorRouter>();
    }
}
