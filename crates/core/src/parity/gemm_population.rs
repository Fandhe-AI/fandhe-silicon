//! GEMM の門の run の母数の表と数え方（spec TASK-1005 の 3/6）。
//!
//! 腕の中身は各腕のタスク（TASK-774・TASK-1008〜TASK-1011）が持ち、本モジュールは腕の識別名と
//! 流す dtype と「母数に入るか」だけを持つ。門の run の枠と各腕の試験から、セル（腕 × 行 × dtype）を
//! キーにして呼ばれる。行と dtype は [`super::gemm_inputs`] の定義をそのまま使う。
//! 採否の規則と照合の集計は本モジュールの範囲の外（件数を数えるだけ）。

use super::gemm_inputs::{GemmGateRow, GemmInputDtype};

/// 母数に入る件数（母数に入る腕 × 行 × dtype の数。表から数えた値と試験で一致を確かめる）。
pub const GEMM_GATE_POPULATION: usize = 60;

const F32_ONLY: [GemmInputDtype; 1] = [GemmInputDtype::F32];
const F32_AND_F16ACC: [GemmInputDtype; 2] = [GemmInputDtype::F32, GemmInputDtype::F16Acc];

/// 門の run で流す腕。`Mps` は参考の腕で、母数に入らない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GemmGateArm {
    /// G1。
    G1,
    /// G2。
    G2,
    /// G3。
    G3,
    /// G3'。
    G3Prime,
    /// G5。
    G5,
    /// G6。
    G6,
    /// G7。
    G7,
    /// G9。
    G9,
    /// MPS（参考の腕。母数に入らない）。
    Mps,
}

impl GemmGateArm {
    /// 9 つの腕すべて（この順で固定）。
    pub const ALL: [GemmGateArm; 9] = [
        Self::G1,
        Self::G2,
        Self::G3,
        Self::G3Prime,
        Self::G5,
        Self::G6,
        Self::G7,
        Self::G9,
        Self::Mps,
    ];

    /// 腕の識別名。`[a-z0-9-]` だけで、試験の識別子の一部に使える。
    pub fn name(self) -> &'static str {
        match self {
            Self::G1 => "g1",
            Self::G2 => "g2",
            Self::G3 => "g3",
            Self::G3Prime => "g3-prime",
            Self::G5 => "g5",
            Self::G6 => "g6",
            Self::G7 => "g7",
            Self::G9 => "g9",
            Self::Mps => "mps",
        }
    }

    /// 門の run で流す dtype（`F32` → `F16Acc` の順）。
    pub fn dtypes(self) -> &'static [GemmInputDtype] {
        match self {
            Self::G1 | Self::G2 | Self::G5 | Self::G7 => &F32_ONLY,
            Self::G3 | Self::G3Prime | Self::G6 | Self::G9 | Self::Mps => &F32_AND_F16ACC,
        }
    }

    /// この腕が `dtype` を門の run で流すか。
    pub fn runs(self, dtype: GemmInputDtype) -> bool {
        self.dtypes().contains(&dtype)
    }

    /// 母数に入るか。`Mps` だけ入らない。
    pub fn in_population(self) -> bool {
        !matches!(self, Self::Mps)
    }
}

/// 門の run の 1 セル（腕 × 行 × dtype）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GemmGateCell {
    arm: GemmGateArm,
    row: GemmGateRow,
    dtype: GemmInputDtype,
}

impl GemmGateCell {
    /// 腕。
    pub fn arm(self) -> GemmGateArm {
        self.arm
    }

    /// 行。
    pub fn row(self) -> GemmGateRow {
        self.row
    }

    /// 入力の dtype。
    pub fn dtype(self) -> GemmInputDtype {
        self.dtype
    }

    /// セルの識別名 `"<腕>-<行>-<dtype>"`（例 `"g6-sq512-f32"`）。
    pub fn id(self) -> String {
        format!(
            "{}-{}-{}",
            self.arm.name(),
            self.row.name(),
            self.dtype.name()
        )
    }

    /// 母数に入るか（腕の値をそのまま返す）。
    pub fn in_population(self) -> bool {
        self.arm.in_population()
    }

    /// 識別名（[`Self::id`] の形）と完全一致したときだけセルを返す。大小文字や前後の空白は許さない。
    ///
    /// 門の run の枠が、プロセスに渡された識別名を 1 セルへ解決するために呼ぶ。
    pub fn from_id(id: &str) -> Option<GemmGateCell> {
        gate_cells().into_iter().find(|c| c.id() == id)
    }
}

/// 門の run で流す全セル（`Mps` を含む）。順は 腕 → 行 → dtype で決定的。
pub fn gate_cells() -> Vec<GemmGateCell> {
    let mut cells = Vec::new();
    for arm in GemmGateArm::ALL {
        for row in GemmGateRow::ALL {
            for &dtype in arm.dtypes() {
                cells.push(GemmGateCell { arm, row, dtype });
            }
        }
    }
    cells
}

/// 母数に入るセル（[`gate_cells`] から `Mps` を除いたもの）。
pub fn population_cells() -> Vec<GemmGateCell> {
    gate_cells()
        .into_iter()
        .filter(|c| c.in_population())
        .collect()
}
