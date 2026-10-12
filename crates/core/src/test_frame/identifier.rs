//! 試験が宣言する識別子（試験の ID と、対象のビヘイビア項目の ID）の受け口。
//!
//! 試験の側（`outcome::Subject::from_parts`）から呼ばれる。識別子は信頼しない入力として扱い、
//! 文字の種類と長さを限って検査する。外れは `Err` で返し、エラーは種類だけで入力の値を含めない。
//! 検査を通らない識別子は作れない（フィールドは非公開）。spec TASK-23 の 5/8。

use std::fmt;

/// 試験の ID の最大の長さ（バイト）。
pub const MAX_TEST_ID_LEN: usize = 128;
/// ビヘイビア項目の ID の最大の長さ（バイト）。
pub const MAX_BEHAVIOR_ID_LEN: usize = 32;

/// 識別子の検査の失敗。入力の値は含めず、種類だけを表す。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentifierError {
    /// 空の識別子。
    Empty,
    /// 長さの上限を超えた識別子。
    TooLong,
    /// 許す文字の外を含む識別子。
    InvalidChar,
}

impl IdentifierError {
    /// 失敗の種類を英語の ASCII で返す。
    pub const fn as_str(self) -> &'static str {
        match self {
            IdentifierError::Empty => "identifier is empty",
            IdentifierError::TooLong => "identifier is too long",
            IdentifierError::InvalidChar => "identifier has an invalid character",
        }
    }
}

impl fmt::Display for IdentifierError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::error::Error for IdentifierError {}

/// 空 → 長さ → 文字の順で検査する共通の処理。
fn check(name: &str, max: usize, allowed: fn(u8) -> bool) -> Result<(), IdentifierError> {
    if name.is_empty() {
        return Err(IdentifierError::Empty);
    }
    if name.len() > max {
        return Err(IdentifierError::TooLong);
    }
    if !name.bytes().all(allowed) {
        return Err(IdentifierError::InvalidChar);
    }
    Ok(())
}

/// 試験の ID。文字は `[a-z0-9_-]`、長さは 1 以上 [`MAX_TEST_ID_LEN`] 以下。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TestId(String);

impl TestId {
    /// 名前を検査して作る。
    pub fn new(name: &str) -> Result<TestId, IdentifierError> {
        check(name, MAX_TEST_ID_LEN, |b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-'
        })?;
        Ok(TestId(name.to_owned()))
    }

    /// 名前を返す。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 対象のビヘイビア項目の ID（`04-behavior/` の項目 ID のポインタの形）。
/// 文字は `[A-Z0-9-]`、長さは 1 以上 [`MAX_BEHAVIOR_ID_LEN`] 以下。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BehaviorId(String);

impl BehaviorId {
    /// 名前を検査して作る。
    pub fn new(name: &str) -> Result<BehaviorId, IdentifierError> {
        check(name, MAX_BEHAVIOR_ID_LEN, |b| {
            b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'-'
        })?;
        Ok(BehaviorId(name.to_owned()))
    }

    /// 名前を返す。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
