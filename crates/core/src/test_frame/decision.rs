//! 宣言と実行の設定から、試験を走らせるかどうかを決める純粋な関数。
//!
//! 試験の関数の先頭から呼ばれる。入力以外の状態を読まず、panic しない。
//! 走らせない場合は理由を列挙で返し、成功にも失敗にも読み替えない。

use super::combination::MachineCombination;
use super::declaration::Declaration;
use super::setting::{CombinationSetting, PermissionSetting, RunSetting};
use std::fmt;

/// 走らせない理由。自由な文は持たず、入力の値も含めない。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotRunReason {
    /// 組み合わせの設定が無い。
    NotConfigured,
    /// 組み合わせの設定が不正。
    InvalidCombinationSetting,
    /// 許可の設定が不正。
    InvalidPermissionSetting,
    /// 設定の組み合わせが宣言に無い。
    CombinationNotDeclared,
    /// 宣言が要る許可のうち、得ていないものがある。
    PermissionMissing,
}

impl NotRunReason {
    /// 理由を英語の ASCII で返す。
    pub const fn as_str(self) -> &'static str {
        match self {
            NotRunReason::NotConfigured => "machine combination is not configured",
            NotRunReason::InvalidCombinationSetting => "machine combination setting is invalid",
            NotRunReason::InvalidPermissionSetting => "permission setting is invalid",
            NotRunReason::CombinationNotDeclared => "combination is not declared by the test",
            NotRunReason::PermissionMissing => "a required permission is not granted",
        }
    }
}

impl NotRunReason {
    /// 理由のトークン（`[a-z0-9-]`）。結果のラベルが理由として出す。
    pub const fn name(self) -> &'static str {
        match self {
            NotRunReason::NotConfigured => "not-configured",
            NotRunReason::InvalidCombinationSetting => "invalid-combination-setting",
            NotRunReason::InvalidPermissionSetting => "invalid-permission-setting",
            NotRunReason::CombinationNotDeclared => "combination-not-declared",
            NotRunReason::PermissionMissing => "permission-missing",
        }
    }
}

impl fmt::Display for NotRunReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 実行の可否。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// 走らせる。値は設定の組み合わせ。
    Run(MachineCombination),
    /// 走らせない。
    NotRun(NotRunReason),
}

impl fmt::Display for Decision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Decision::Run(c) => write!(f, "run on {}", c.name()),
            Decision::NotRun(r) => write!(f, "not run: {r}"),
        }
    }
}

/// 宣言と設定から可否を決める。上から先に当たった条件が勝つ。
/// 不正な設定は宣言に関わらず走らせない側に倒す（fail-closed）。
pub fn decide(declaration: &Declaration, setting: &RunSetting) -> Decision {
    let combination = match &setting.combination {
        CombinationSetting::Unset => return Decision::NotRun(NotRunReason::NotConfigured),
        CombinationSetting::Invalid => {
            return Decision::NotRun(NotRunReason::InvalidCombinationSetting);
        }
        CombinationSetting::Set(c) => *c,
    };
    let granted = match &setting.permissions {
        PermissionSetting::Invalid => {
            return Decision::NotRun(NotRunReason::InvalidPermissionSetting);
        }
        PermissionSetting::Granted(g) => g,
    };
    if !declaration.combinations().contains(&combination) {
        return Decision::NotRun(NotRunReason::CombinationNotDeclared);
    }
    if declaration
        .permissions()
        .iter()
        .any(|p| !granted.contains(p))
    {
        return Decision::NotRun(NotRunReason::PermissionMissing);
    }
    Decision::Run(combination)
}
