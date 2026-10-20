//! 背後実装を持たない試験用のスタブ。模擬のロスト（TASK-271 の (1)）と門の状態の記録だけを持つ。
//!
//! `FaultHooks` を実装する側の見本であり、`inject` から呼ばれる。実際の device や投入は持たないため、
//! 門は状態の記録だけで、本物の投入を未完了にする働きは背後実装が持つ。
//! ロストは模擬であって、実際の故障ではない。名前は仮。

use std::sync::{Mutex, MutexGuard};

use super::FaultHooks;
use crate::error::{ContractError, DeviceLostReason};

/// ロストのコールバック。最初のロストで 1 回だけ呼ばれる。
pub type LostCallback = Box<dyn FnOnce(DeviceLostReason) + Send>;

#[derive(Default)]
struct State {
    lost_reason: Option<DeviceLostReason>,
    callback: Option<LostCallback>,
    callback_count: u64,
    gate_armed: bool,
}

/// 模擬のロストと門の状態を持つスタブ。`Send + Sync`。
#[derive(Default)]
pub struct FaultStub {
    state: Mutex<State>,
}

impl FaultStub {
    /// ロストしていない、門が仕込まれていない、コールバックが無い状態で作る。
    pub fn new() -> Self {
        Self::default()
    }

    /// 毒化していても中身を取り出す。panic しない。
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// ロストのコールバックを登録する。登録済みなら上書きする。
    /// すでにロスト済みのときは呼ばれず、保持するだけになる。
    pub fn set_lost_callback(&self, cb: LostCallback) {
        self.lock().callback = Some(cb);
    }

    /// 模擬のロストが起きたか。
    pub fn is_lost(&self) -> bool {
        self.lock().lost_reason.is_some()
    }

    /// 最初のロストの理由。ロストしていなければ `None`。
    pub fn lost_reason(&self) -> Option<DeviceLostReason> {
        self.lock().lost_reason.clone()
    }

    /// ロストのコールバックを呼んだ回数。未登録のまま起きたロストは数えない。
    pub fn lost_callback_count(&self) -> u64 {
        self.lock().callback_count
    }

    /// 門が仕込まれているか。
    pub fn gate_armed(&self) -> bool {
        self.lock().gate_armed
    }
}

impl FaultHooks for FaultStub {
    /// 1 回目は理由を記録し、コールバックを lock の外で 1 回呼ぶ。2 回目以降は何もせず `Ok`。
    fn simulate_device_lost(&self, reason: DeviceLostReason) -> Result<(), ContractError> {
        let cb = {
            let mut st = self.lock();
            if st.lost_reason.is_some() {
                return Ok(());
            }
            st.lost_reason = Some(reason.clone());
            let cb = st.callback.take();
            if cb.is_some() {
                st.callback_count += 1;
            }
            cb
        };
        // コールバックの中から同じスタブを呼んでもデッドロックしないよう、lock を手放してから呼ぶ。
        if let Some(cb) = cb {
            cb(reason);
        }
        Ok(())
    }

    fn arm_gate(&self) -> Result<(), ContractError> {
        self.lock().gate_armed = true;
        Ok(())
    }

    fn open_gate(&self) -> Result<(), ContractError> {
        self.lock().gate_armed = false;
        Ok(())
    }
}
