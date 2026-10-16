//! 照合の集まり。件数の上限と ID の重複を入口で拒む。
//!
//! 流し直しの側が照合を渡すときの受け口。無制限の入力と、同じ ID の照合が混ざることを防ぐ。

use super::check::Check;
use std::fmt;

/// 集まりの件数の上限。
pub const MAX_CHECKS: usize = 256;

/// [`CheckSet::new`] の失敗。入力の値（ID）は含めず、種類だけを表す。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckSetError {
    /// 照合が 1 件も無い。
    Empty,
    /// 件数が [`MAX_CHECKS`] を超えた。
    TooMany,
    /// 同じ試験の ID の照合が 2 件以上ある。
    DuplicateId,
}

impl CheckSetError {
    /// 失敗の種類を英語の ASCII で返す。
    pub const fn as_str(self) -> &'static str {
        match self {
            CheckSetError::Empty => "check set is empty",
            CheckSetError::TooMany => "check set has too many checks",
            CheckSetError::DuplicateId => "check set has a duplicate test id",
        }
    }
}

impl fmt::Display for CheckSetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::error::Error for CheckSetError {}

/// 照合の並び。宣言の順を保ち、試験の ID は重複しない。
#[derive(Clone, Copy)]
pub struct CheckSet<'a> {
    checks: &'a [&'a dyn Check],
}

impl<'a> CheckSet<'a> {
    /// 検査して集まりを作る。検査の順は 空 → 件数 → 重複。
    pub fn new(checks: &'a [&'a dyn Check]) -> Result<CheckSet<'a>, CheckSetError> {
        if checks.is_empty() {
            return Err(CheckSetError::Empty);
        }
        if checks.len() > MAX_CHECKS {
            return Err(CheckSetError::TooMany);
        }
        for (i, c) in checks.iter().enumerate() {
            let id = c.subject().id();
            if checks[..i].iter().any(|p| p.subject().id() == id) {
                return Err(CheckSetError::DuplicateId);
            }
        }
        Ok(CheckSet { checks })
    }

    /// 宣言の順の照合。
    pub fn checks(&self) -> &'a [&'a dyn Check] {
        self.checks
    }

    /// 照合の件数（1 以上）。
    pub fn len(&self) -> usize {
        self.checks.len()
    }

    /// 空かどうか。`new` は空を拒むので常に `false`。
    pub fn is_empty(&self) -> bool {
        self.checks.is_empty()
    }
}
