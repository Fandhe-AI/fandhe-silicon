//! run 間の変動の幅の規則を、PoC ごとの閉じた設定として持つ純粋な関数（TASK-1027 の規則の部分）。
//!
//! 呼び手（測定のハーネスと、後の段の判定の処理）が、`gate::GateOutcome::taken_run_ids` で絞った run について
//! `runs` で集めた腕ごとの run の値の列を渡し、「幅を超えたか」（PoC-11 は「差が幅の中か」）を得る。
//! 設定は [`VariationRule::setting`] の定数表の 1 か所だけに置き、外から差し替える公開の口は無い。
//! 単位は `runs::Band` と同じ 1/4 ns の整数で、中央値は丸めないよう 2 倍して持ち、
//! 判定は `u128` の checked 演算で行う（`f64` の割り算は使わない）。
//! 時計・ファイル・環境に触れず、`record::RunRecord` の JSON も変えない。
//!
//! 本モジュールが受け取るのは値の列だけである。GPU の値が取れない run（`runs` の `gpu: None`）には
//! 呼び手が規則を当てず、判定を付けない側に倒す。測り直しと判定不能の扱いは本モジュールの範囲の外。
//!
//! spec との対応: `docs/spec/05-tasks.md` の TASK-1027、`docs/spec/04-behavior/performance.md`
//! （ポインタのみ）。係数の出どころは T-7・T-13・T-15・T-16、対象の PoC は PoC-9・10・11・23 の ID で示す。

use std::fmt;

/// 変動の幅の規則に当たったかを調べる設定の識別子。閉じた集合で、外から足せない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum VariationRule {
    /// PoC-10 の (2)（T-7 の幅）。
    Poc10Row2,
    /// PoC-10 の (3)（T-13 の幅）。
    Poc10Row3,
    /// PoC-23 の CPU 時間（T-15。絶対値の下限つき）。
    Poc23Cpu,
    /// PoC-23 の GPU 時間（T-15）。
    Poc23Gpu,
    /// PoC-23 の V3-b の CPU 時間（T-16）。
    Poc23V3bCpu,
    /// PoC-23 の V3-b の GPU 時間（T-16）。
    Poc23V3bGpu,
    /// PoC-9（run 間の中央値の最大 ÷ 最小）。
    Poc9,
    /// PoC-11（run の p95 を使い、差が幅の中かで見る）。
    Poc11,
}

impl VariationRule {
    /// 全設定。
    pub const ALL: [VariationRule; 8] = [
        Self::Poc10Row2,
        Self::Poc10Row3,
        Self::Poc23Cpu,
        Self::Poc23Gpu,
        Self::Poc23V3bCpu,
        Self::Poc23V3bGpu,
        Self::Poc9,
        Self::Poc11,
    ];

    /// 出力用の綴り（英小文字 ASCII とハイフン）。
    pub const fn name(self) -> &'static str {
        match self {
            Self::Poc10Row2 => "poc10-2",
            Self::Poc10Row3 => "poc10-3",
            Self::Poc23Cpu => "poc23-cpu",
            Self::Poc23Gpu => "poc23-gpu",
            Self::Poc23V3bCpu => "poc23-v3b-cpu",
            Self::Poc23V3bGpu => "poc23-v3b-gpu",
            Self::Poc9 => "poc9",
            Self::Poc11 => "poc11",
        }
    }

    /// 設定の値。値の唯一の置き場（この表以外に係数・run の数を書かない）。
    pub const fn setting(self) -> VariationSetting {
        const RUNS: usize = 6;
        const fn spread(num: u64, den: u64) -> Criterion {
            Criterion::SpreadOverMedian { num, den }
        }
        match self {
            Self::Poc10Row2 => VariationSetting::new(RUNS, RunStatistic::Median, spread(1, 10)),
            Self::Poc10Row3 => VariationSetting::new(RUNS, RunStatistic::Median, spread(1, 20)),
            Self::Poc23Cpu => VariationSetting::new(
                RUNS,
                RunStatistic::Median,
                Criterion::SpreadOverMedianWithFloor {
                    num: 1,
                    den: 20,
                    floor_quarters: 4_000,
                },
            ),
            Self::Poc23Gpu => VariationSetting::new(RUNS, RunStatistic::Median, spread(1, 50)),
            Self::Poc23V3bCpu => VariationSetting::new(RUNS, RunStatistic::Median, spread(1, 10)),
            Self::Poc23V3bGpu => VariationSetting::new(RUNS, RunStatistic::Median, spread(1, 50)),
            Self::Poc9 => VariationSetting::new(
                RUNS,
                RunStatistic::Median,
                Criterion::MaxOverMinAtLeast { num: 13, den: 10 },
            ),
            Self::Poc11 => {
                VariationSetting::new(RUNS, RunStatistic::P95, Criterion::DifferenceWithinSpread)
            }
        }
    }
}

/// 1 run の値として使う統計。呼び手が列を作るときの指示で、本モジュールは列をそのまま受ける。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RunStatistic {
    /// run の中央値。
    Median,
    /// run の p95。
    P95,
}

/// 判定の形。係数は整数比で持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Criterion {
    /// 幅（最大 − 最小）が、係数 × run 間中央値を超える（ちょうど等しいときは超えない）。
    SpreadOverMedian {
        /// 係数の分子。
        num: u64,
        /// 係数の分母。
        den: u64,
    },
    /// 幅が、係数 × run 間中央値と絶対値の下限の大きいほうを超える（ちょうど等しいときは超えない）。
    SpreadOverMedianWithFloor {
        /// 係数の分子。
        num: u64,
        /// 係数の分母。
        den: u64,
        /// 絶対値の下限（1/4 ns 単位）。
        floor_quarters: u64,
    },
    /// 最大 ÷ 最小 が閾値以上（ちょうど等しいときは当たる）。
    MaxOverMinAtLeast {
        /// 閾値の分子。
        num: u64,
        /// 閾値の分母。
        den: u64,
    },
    /// 差の絶対値が幅以下（ちょうど等しいときは当たる）。[`check_difference`] だけが使う。
    DifferenceWithinSpread,
}

/// 設定の値（run の数・1 run の値の統計・判定の形）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VariationSetting {
    /// 腕ごとの run の数。入力の列の長さがこれと違えば `Err`。
    pub runs: usize,
    /// 1 run の値に使う統計。
    pub statistic: RunStatistic,
    /// 判定の形。
    pub criterion: Criterion,
}

impl VariationSetting {
    const fn new(runs: usize, statistic: RunStatistic, criterion: Criterion) -> Self {
        Self {
            runs,
            statistic,
            criterion,
        }
    }
}

/// 規則の適用の失敗。入力の値は埋め込まない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum VariationError {
    /// 腕・run の入力が空。
    Empty,
    /// run の数が設定と違う。
    RunCount,
    /// PoC-9 の最小が 0 で、比が定まらない。
    ZeroMinimum,
    /// 腕の名前が重複している。
    DuplicateArm,
    /// 規則の判定の形と、呼んだ関数が合わない。
    WrongCriterion,
    /// 計算が溢れた。
    Overflow,
}

impl fmt::Display for VariationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            Self::Empty => "empty input",
            Self::RunCount => "run count does not match the setting",
            Self::ZeroMinimum => "minimum is zero",
            Self::DuplicateArm => "duplicate arm name",
            Self::WrongCriterion => "criterion does not match the function",
            Self::Overflow => "arithmetic overflow",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for VariationError {}

/// 1 つの腕の run の値の列の要約（2 倍の中央値・最小・最大・幅）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArmSpread {
    median_x2: u128,
    min: u64,
    max: u64,
}

impl ArmSpread {
    /// run の値（1/4 ns 単位）の列から作る。列の長さが `expected_runs` と違えば `Err`。
    pub fn from_runs(values: &[u64], expected_runs: usize) -> Result<Self, VariationError> {
        if values.is_empty() {
            return Err(VariationError::Empty);
        }
        if values.len() != expected_runs {
            return Err(VariationError::RunCount);
        }
        let mut v = values.to_vec();
        v.sort_unstable();
        let n = v.len();
        let median_x2 = if n % 2 == 1 {
            2 * u128::from(v[n / 2])
        } else {
            u128::from(v[n / 2 - 1]) + u128::from(v[n / 2])
        };
        Ok(Self {
            median_x2,
            min: v[0],
            max: v[n - 1],
        })
    }

    /// 幅（最大 − 最小、1/4 ns 単位）。
    pub const fn spread(&self) -> u64 {
        self.max - self.min
    }
}

/// 幅が `num/den × 中央値` を超えるか（`spread × 2 × den > num × median_x2`）。
fn spread_exceeds(arm: &ArmSpread, num: u64, den: u64) -> Result<bool, VariationError> {
    let lhs = u128::from(arm.spread())
        .checked_mul(2)
        .and_then(|x| x.checked_mul(u128::from(den)))
        .ok_or(VariationError::Overflow)?;
    let rhs = arm
        .median_x2
        .checked_mul(u128::from(num))
        .ok_or(VariationError::Overflow)?;
    Ok(lhs > rhs)
}

/// 幅の規則に当たったか（PoC-11 の規則は [`check_difference`] を使うので `Err`）。
pub fn check_arm(rule: VariationRule, arm: &ArmSpread) -> Result<bool, VariationError> {
    match rule.setting().criterion {
        Criterion::SpreadOverMedian { num, den } => spread_exceeds(arm, num, den),
        Criterion::SpreadOverMedianWithFloor {
            num,
            den,
            floor_quarters,
        } => Ok(arm.spread() > floor_quarters && spread_exceeds(arm, num, den)?),
        Criterion::MaxOverMinAtLeast { num, den } => {
            if arm.min == 0 {
                return Err(VariationError::ZeroMinimum);
            }
            let lhs = u128::from(arm.max)
                .checked_mul(u128::from(den))
                .ok_or(VariationError::Overflow)?;
            let rhs = u128::from(arm.min)
                .checked_mul(u128::from(num))
                .ok_or(VariationError::Overflow)?;
            Ok(lhs >= rhs)
        }
        Criterion::DifferenceWithinSpread => Err(VariationError::WrongCriterion),
    }
}

/// 1 つの腕の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArmVariation {
    /// 腕の名前。
    pub name: String,
    /// 規則に当たったか。
    pub exceeded: bool,
}

/// 1 行（腕の集まり）の結果。腕は名前の昇順。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowVariation {
    /// 腕ごとの結果（名前の昇順）。
    pub arms: Vec<ArmVariation>,
}

impl RowVariation {
    /// どれか 1 つでも当たったか（その行が測り直しの対象になる）。
    pub fn any_exceeded(&self) -> bool {
        self.arms.iter().any(|a| a.exceeded)
    }

    /// 当たった腕の名前（昇順）。
    pub fn exceeded_names(&self) -> Vec<&str> {
        self.arms
            .iter()
            .filter(|a| a.exceeded)
            .map(|a| a.name.as_str())
            .collect()
    }
}

/// 1 行の腕ごとの run の値の列に規則を当てる。渡す順に結果が依らない。
///
/// 腕が 0 件・名前の重複・run の数の不一致・規則と合わない判定の形は `Err`。
pub fn check_row(
    rule: VariationRule,
    arms: &[(&str, &[u64])],
) -> Result<RowVariation, VariationError> {
    if arms.is_empty() {
        return Err(VariationError::Empty);
    }
    let runs = rule.setting().runs;
    let mut out = Vec::with_capacity(arms.len());
    for (name, values) in arms {
        let arm = ArmSpread::from_runs(values, runs)?;
        out.push(ArmVariation {
            name: (*name).to_string(),
            exceeded: check_arm(rule, &arm)?,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    if out.windows(2).any(|w| w[0].name == w[1].name) {
        return Err(VariationError::DuplicateArm);
    }
    Ok(RowVariation { arms: out })
}

/// 差の相手。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Other<'a> {
    /// 定数（1/4 ns 単位）。
    Constant(u64),
    /// 別の腕の run の値の列。
    Arm(&'a [u64]),
}

/// PoC-11 の規則: `target` の run 間中央値と相手の差の絶対値が幅以下なら `true`（幅の中で判定できない）。
///
/// 相手が定数なら `target` の幅、相手が腕なら 2 腕の幅の大きいほうを使う。ちょうど等しいときは `true`。
pub fn check_difference(target: &[u64], other: Other<'_>) -> Result<bool, VariationError> {
    let setting = VariationRule::Poc11.setting();
    let t = ArmSpread::from_runs(target, setting.runs)?;
    let (other_x2, width) = match other {
        Other::Constant(c) => (2 * u128::from(c), t.spread()),
        Other::Arm(values) => {
            let o = ArmSpread::from_runs(values, setting.runs)?;
            (o.median_x2, t.spread().max(o.spread()))
        }
    };
    let diff_x2 = t.median_x2.abs_diff(other_x2);
    let width_x2 = 2 * u128::from(width);
    Ok(diff_x2 <= width_x2)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 6 run の列（中央値が 1000 になる合成値）。幅は min と max で決める。
    fn runs6(min: u64, max: u64) -> [u64; 6] {
        [min, 1000, 1000, 1000, 1000, max]
    }

    fn arm_exceeds(rule: VariationRule, v: &[u64]) -> bool {
        check_arm(rule, &ArmSpread::from_runs(v, 6).unwrap()).unwrap()
    }

    #[test]
    fn names_are_lowercase_ascii_and_unique() {
        for r in VariationRule::ALL {
            assert!(
                r.name()
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            );
        }
        let mut names: Vec<_> = VariationRule::ALL.iter().map(|r| r.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), VariationRule::ALL.len());
    }

    #[test]
    fn spread_rule_is_strict_at_boundary() {
        // 設定の係数を取り出し、中央値 1000 に対する境目の幅を作る。
        for rule in [
            VariationRule::Poc10Row2,
            VariationRule::Poc10Row3,
            VariationRule::Poc23Gpu,
            VariationRule::Poc23V3bCpu,
            VariationRule::Poc23V3bGpu,
        ] {
            let Criterion::SpreadOverMedian { num, den } = rule.setting().criterion else {
                panic!("unexpected criterion");
            };
            assert_eq!(1000 * num % den, 0, "synthetic median must divide evenly");
            let w = 1000 * num / den;
            // 幅 = w: 超えない。w + 1: 超える。w - 1: 超えない。
            assert!(!arm_exceeds(rule, &runs6(1000, 1000 + w)));
            assert!(arm_exceeds(rule, &runs6(1000, 1000 + w + 1)));
            assert!(!arm_exceeds(rule, &runs6(1000, 1000 + w - 1)));
        }
    }

    #[test]
    fn even_count_median_is_not_rounded() {
        // 中央の 2 つが 1019 と 1020 なら中央値は 1019.5。係数 1/20 の閾値は 50.975 で、
        // 幅 51 は超える（中央値を 1020 に丸めると閾値が 51.0 になり超えなくなる）。
        let rule = VariationRule::Poc10Row3;
        let Criterion::SpreadOverMedian { num, den } = rule.setting().criterion else {
            panic!("unexpected criterion");
        };
        assert_eq!((num, den), (1, 20));
        assert!(arm_exceeds(rule, &[1000, 1019, 1019, 1020, 1020, 1051]));
        assert!(!arm_exceeds(rule, &[1001, 1019, 1019, 1020, 1020, 1051]));
    }

    #[test]
    fn floor_applies_to_cpu_rule_only_when_larger() {
        let rule = VariationRule::Poc23Cpu;
        let Criterion::SpreadOverMedianWithFloor { floor_quarters, .. } = rule.setting().criterion
        else {
            panic!("unexpected criterion");
        };
        // 中央値が小さく係数の側が下限より小さい: 下限ちょうどは超えず、+1 で超える。
        let small = |w: u64| [100, 200, 200, 200, 200, 100 + w];
        assert!(!arm_exceeds(rule, &small(floor_quarters)));
        assert!(arm_exceeds(rule, &small(floor_quarters + 1)));
        // 中央値が大きく係数の側が下限より大きい: 係数の側が効く。
        let big = 1_000_000u64;
        let Criterion::SpreadOverMedianWithFloor { num, den, .. } = rule.setting().criterion else {
            panic!("unexpected criterion");
        };
        let w = big * num / den;
        assert!(w > floor_quarters);
        let at = |w: u64| [big - 10, big, big, big, big, big - 10 + w];
        assert!(!arm_exceeds(rule, &at(w)));
        assert!(arm_exceeds(rule, &at(w + 1)));
    }

    #[test]
    fn max_over_min_is_inclusive() {
        let rule = VariationRule::Poc9;
        let Criterion::MaxOverMinAtLeast { num, den } = rule.setting().criterion else {
            panic!("unexpected criterion");
        };
        let min = 1000u64;
        assert_eq!(min * num % den, 0);
        let at = min * num / den;
        assert!(arm_exceeds(rule, &[min, min, min, min, min, at]));
        assert!(!arm_exceeds(rule, &[min, min, min, min, min, at - 1]));
        assert!(arm_exceeds(rule, &[min, min, min, min, min, at + 1]));
    }

    #[test]
    fn zero_minimum_is_error() {
        let a = ArmSpread::from_runs(&[0, 1, 1, 1, 1, 1], 6).unwrap();
        assert_eq!(
            check_arm(VariationRule::Poc9, &a),
            Err(VariationError::ZeroMinimum)
        );
    }

    #[test]
    fn invalid_shapes_are_errors() {
        assert_eq!(ArmSpread::from_runs(&[], 6), Err(VariationError::Empty));
        assert_eq!(
            ArmSpread::from_runs(&[1, 2, 3], 6),
            Err(VariationError::RunCount)
        );
        let ok = [1u64; 6];
        assert_eq!(
            check_row(VariationRule::Poc9, &[]),
            Err(VariationError::Empty)
        );
        assert_eq!(
            check_row(VariationRule::Poc9, &[("a", &ok), ("a", &ok)]),
            Err(VariationError::DuplicateArm)
        );
        assert_eq!(
            check_row(VariationRule::Poc9, &[("a", &ok[..5])]),
            Err(VariationError::RunCount)
        );
        assert_eq!(
            check_row(VariationRule::Poc11, &[("a", &ok)]),
            Err(VariationError::WrongCriterion)
        );
    }

    #[test]
    fn row_result_is_order_independent_and_sorted() {
        let rule = VariationRule::Poc10Row2;
        let wide = runs6(100, 1900);
        let narrow = [1000u64; 6];
        let x = check_row(rule, &[("b", &wide), ("a", &narrow)]).unwrap();
        let y = check_row(rule, &[("a", &narrow), ("b", &wide)]).unwrap();
        assert_eq!(x, y);
        assert_eq!(x.arms[0].name, "a");
        assert!(x.any_exceeded());
        assert_eq!(x.exceeded_names(), vec!["b"]);
        let z = check_row(rule, &[("a", &narrow)]).unwrap();
        assert!(!z.any_exceeded());
    }

    #[test]
    fn difference_with_constant_is_inclusive() {
        // 幅 20（最大 1010 − 最小 990）、中央値 1000。
        let t = [990, 1000, 1000, 1000, 1000, 1010];
        assert_eq!(check_difference(&t, Other::Constant(1020)), Ok(true));
        assert_eq!(check_difference(&t, Other::Constant(980)), Ok(true));
        assert_eq!(check_difference(&t, Other::Constant(1021)), Ok(false));
        assert_eq!(check_difference(&t, Other::Constant(979)), Ok(false));
    }

    #[test]
    fn difference_with_arm_uses_larger_spread() {
        let t = [999, 1000, 1000, 1000, 1000, 1001]; // 幅 2
        let o = [1090, 1100, 1100, 1100, 1100, 1110]; // 幅 20、中央値 1100
        // 差 100 > 幅 20 → 当たらない。
        assert_eq!(check_difference(&t, Other::Arm(&o)), Ok(false));
        let o2 = [1080, 1100, 1100, 1100, 1100, 1100]; // 幅 20
        assert_eq!(check_difference(&t, Other::Arm(&o2)), Ok(false));
        // 差 20 = 大きいほうの幅 20 → 当たる。小さいほうの幅 2 なら当たらない。
        let o3 = [1010, 1020, 1020, 1020, 1020, 1030];
        assert_eq!(check_difference(&t, Other::Arm(&o3)), Ok(true));
        let o4 = [1019, 1020, 1020, 1020, 1020, 1021];
        assert_eq!(check_difference(&t, Other::Arm(&o4)), Ok(false));
    }

    #[test]
    fn difference_checks_run_count() {
        assert_eq!(
            check_difference(&[1, 2], Other::Constant(1)),
            Err(VariationError::RunCount)
        );
    }

    #[test]
    fn large_values_do_not_overflow() {
        let m = u64::MAX;
        let a = ArmSpread::from_runs(&[0, m, m, m, m, m], 6).unwrap();
        assert_eq!(check_arm(VariationRule::Poc10Row2, &a), Ok(true));
        let b = ArmSpread::from_runs(&[m; 6], 6).unwrap();
        assert_eq!(check_arm(VariationRule::Poc10Row2, &b), Ok(false));
        assert_eq!(check_arm(VariationRule::Poc9, &b), Ok(false));
        assert!(check_difference(&[m; 6], Other::Constant(m)).unwrap());
    }
}
