//! 試験用の口が `Exec` に持たせる、操作ごとの印の置き場（feature `fault-injection` の下だけ）。
//!
//! `entry` の `Exec` が欄として持ち、`route_report` が操作ごとに読んで `select` の入力へ渡す。
//! `Copy` で内部の可変性を持たず、crate の外には公開しない。

use crate::route::OpId;

/// 操作ごとの印の組。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Ports {
    forced: [bool; 4],
}

/// 操作ごとの添字。`OpId` に値が足されるとコンパイルで落ちるよう `_` の腕を置かない。
const fn op_slot(op: OpId) -> usize {
    match op {
        OpId::F16Fma => 0,
        OpId::SubgroupSumU32 => 1,
        OpId::SubgroupSumF32 => 2,
        OpId::AtomicAdd => 3,
    }
}

impl Ports {
    /// `op` に `Native` を候補に入れない印を付けた組を返す。
    pub(crate) fn with_forced(self, op: OpId) -> Self {
        let mut forced = self.forced;
        forced[op_slot(op)] = true;
        Self { forced }
    }

    /// `op` に `Native` を候補に入れない印が付いているか。
    pub(crate) fn is_forced(self, op: OpId) -> bool {
        self.forced[op_slot(op)]
    }
}
