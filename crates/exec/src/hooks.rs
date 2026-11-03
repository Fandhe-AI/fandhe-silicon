//! 試験用の口が `Exec` に持たせる、操作ごとの印の置き場（feature `fault-injection` の下だけ）。
//!
//! `entry` の `Exec` が欄として持ち、`route_report` が操作ごとに読んで `select` の入力へ渡す。
//! 操作と経路の組ごとの f32 の非正規化数の判定の差し込みも持つ。
//! `Copy` で内部の可変性を持たず、crate の外には公開しない。

use crate::report::{DenormalInjection, F32DenormalBasis, F32DenormalReport, RouteReport};
use crate::route::{OpId, Route};
use crate::select::DenormalJudgment;

/// 操作ごとの印の組。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Ports {
    forced: [bool; 4],
    injected: [[Option<DenormalInjection>; 3]; 4],
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

/// 経路ごとの添字。`Route` に値が足されるとコンパイルで落ちるよう `_` の腕を置かない。
const fn route_slot(route: Route) -> usize {
    match route {
        Route::Native => 0,
        Route::GpuEmulated => 1,
        Route::Cpu => 2,
    }
}

/// 差し込みの値を、選び方が受ける判定の値へ写す。
pub(crate) fn judgment_of(j: DenormalInjection) -> DenormalJudgment {
    match j {
        DenormalInjection::Preserve => DenormalJudgment::Preserve,
        DenormalInjection::FlushToZero => DenormalJudgment::FlushToZero,
        DenormalInjection::Unknown => DenormalJudgment::Unknown,
    }
}

impl Ports {
    /// `op` に `Native` を候補に入れない印を付けた組を返す。
    pub(crate) fn with_forced(self, op: OpId) -> Self {
        let mut forced = self.forced;
        forced[op_slot(op)] = true;
        Self { forced, ..self }
    }

    /// (`op`, `route`) の判定を `judgment` に差し込んだ組を返す。同じ組は後の値が効く。
    pub(crate) fn with_injected(self, op: OpId, route: Route, judgment: DenormalInjection) -> Self {
        let mut injected = self.injected;
        injected[op_slot(op)][route_slot(route)] = Some(judgment);
        Self { injected, ..self }
    }

    /// (`op`, `route`) に差し込まれた判定。無ければ `None`。
    pub(crate) fn injected(self, op: OpId, route: Route) -> Option<DenormalInjection> {
        self.injected[op_slot(op)][route_slot(route)]
    }

    /// 選ばれた経路に差し込みがあれば、報告の f32 の項の根拠を `Injected` にする。
    pub(crate) fn mark_injected_basis(self, report: RouteReport) -> RouteReport {
        match (report.route, report.f32_denormal) {
            (Some(route), Some(d)) if self.injected(report.op, route).is_some() => RouteReport {
                f32_denormal: Some(F32DenormalReport {
                    basis: F32DenormalBasis::Injected,
                    ..d
                }),
                ..report
            },
            _ => report,
        }
    }

    /// `op` に `Native` を候補に入れない印が付いているか。
    pub(crate) fn is_forced(self, op: OpId) -> bool {
        self.forced[op_slot(op)]
    }
}
