//! 結果のラベル（OK・NG・判定保留・未実施・判定不能）と、その理由の型。
//!
//! `outcome::judge` が決めた結果を表す。種類・理由とも列挙で、文字列の表現は英語の ASCII。
//! 入力の値・自由な文は持たない。走らせなかった結果・判定保留・判定不能を成功にも失敗にも読み替えない。
//! spec TASK-23。

use super::decision::NotRunReason;
use super::threshold::ThresholdId;
use std::fmt;

/// 判定保留の理由のトークン。閾値が未承認であることを表す（どの閾値かは `Label::Pending` の値）。
pub const PENDING_REASON: &str = "threshold-unapproved";

/// 判定不能の理由。値を作るのは試験の側で、`core` から device を取る経路は持たない。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndeterminateReason {
    /// 取れた device が宣言した機材と合わない。
    DeviceMismatch,
    /// 機材に届かない。
    MachineUnreachable,
    /// 合格の証拠がそろっていない。
    EvidenceMissing,
    /// 比べる 2 つの run の環境が違う。
    EnvironmentDiffers,
}

impl IndeterminateReason {
    /// 理由のトークン（`[a-z0-9-]`）。
    pub const fn name(self) -> &'static str {
        match self {
            IndeterminateReason::DeviceMismatch => "device-mismatch",
            IndeterminateReason::MachineUnreachable => "machine-unreachable",
            IndeterminateReason::EvidenceMissing => "evidence-missing",
            IndeterminateReason::EnvironmentDiffers => "environment-differs",
        }
    }
}

impl fmt::Display for IndeterminateReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            IndeterminateReason::DeviceMismatch => "device does not match the declared machine",
            IndeterminateReason::MachineUnreachable => "machine is unreachable",
            IndeterminateReason::EvidenceMissing => "pass evidence is missing",
            IndeterminateReason::EnvironmentDiffers => "the two runs differ in environment",
        })
    }
}

/// ラベルの種類（理由・閾値の ID を持たない値）。集計の列の単位で、`Label::kind` で得る。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LabelKind {
    /// 一致した。
    Ok,
    /// 一致しなかった。
    Ng,
    /// 判定保留。
    Pending,
    /// 走らせなかった。
    NotRun,
    /// 判定不能。
    Indeterminate,
}

impl LabelKind {
    /// 全ての種類。順は ok・ng・pending・not-run・indeterminate で、重複は無い。
    pub const ALL: [LabelKind; 5] = [
        LabelKind::Ok,
        LabelKind::Ng,
        LabelKind::Pending,
        LabelKind::NotRun,
        LabelKind::Indeterminate,
    ];

    /// 種類のトークン（`[a-z0-9-]`。5 種で重複なし）。
    pub const fn name(self) -> &'static str {
        match self {
            LabelKind::Ok => "ok",
            LabelKind::Ng => "ng",
            LabelKind::Pending => "pending",
            LabelKind::NotRun => "not-run",
            LabelKind::Indeterminate => "indeterminate",
        }
    }
}

/// 結果のラベル。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Label {
    /// 一致した。
    Ok,
    /// 一致しなかった。
    Ng,
    /// 判定保留。値は宣言の順で最初に当たった未承認の閾値の ID。
    Pending(ThresholdId),
    /// 走らせなかった。
    NotRun(NotRunReason),
    /// 判定不能。
    Indeterminate(IndeterminateReason),
}

impl Label {
    /// 種類（理由・閾値の ID を除いた値）。
    pub const fn kind(&self) -> LabelKind {
        match self {
            Label::Ok => LabelKind::Ok,
            Label::Ng => LabelKind::Ng,
            Label::Pending(_) => LabelKind::Pending,
            Label::NotRun(_) => LabelKind::NotRun,
            Label::Indeterminate(_) => LabelKind::Indeterminate,
        }
    }

    /// 種類のトークン（`[a-z0-9-]`。5 種で重複なし）。
    pub const fn name(&self) -> &'static str {
        self.kind().name()
    }

    /// 理由のトークン。理由を持つ種類（判定保留・未実施・判定不能）だけ `Some`。判定保留は常に `threshold-unapproved`。
    pub fn reason_name(&self) -> Option<&'static str> {
        match self {
            Label::Pending(_) => Some(PENDING_REASON),
            Label::NotRun(r) => Some(r.name()),
            Label::Indeterminate(r) => Some(r.name()),
            _ => None,
        }
    }

    /// 試験を失敗にするか。`Ng` のときだけ `true`。
    pub const fn fails_test(&self) -> bool {
        matches!(self, Label::Ng)
    }
}

impl fmt::Display for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Label::Pending(id) => write!(f, "pending: threshold {} is not approved", id.as_str()),
            Label::NotRun(r) => write!(f, "not-run: {r}"),
            Label::Indeterminate(r) => write!(f, "indeterminate: {r}"),
            other => f.write_str(other.name()),
        }
    }
}
