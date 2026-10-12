//! 1 つの試験の宣言（走らせる組み合わせの集まりと、要る許可の集まり）。
//!
//! 試験の関数の先頭で作り、`decision::decide` へ渡す。許可の名前は検査済みの識別子として持ち、
//! 自由な文を受けない。フィールドは非公開で、検査を通らない宣言は作れない。

use super::combination::MachineCombination;
use std::fmt;

/// 許可の名前の最大の長さ（バイト）。
pub const MAX_PERMISSION_LEN: usize = 64;
/// 1 つの宣言が持てる許可の件数の上限。
pub const MAX_PERMISSIONS: usize = 16;
/// 1 つの宣言に渡せる組み合わせの件数の上限（重複を含む）。
pub const MAX_COMBINATIONS: usize = 64;

/// 試験が要る許可の名前。文字は `[a-z0-9-]`、長さは 1 以上 [`MAX_PERMISSION_LEN`] 以下。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Permission(String);

/// [`Permission`] の検査の失敗。入力の値は含めず、種類だけを表す。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionError {
    /// 空の名前。
    Empty,
    /// 長さの上限を超えた名前。
    TooLong,
    /// 許す文字（`[a-z0-9-]`）の外を含む名前。
    InvalidChar,
}

impl Permission {
    /// 名前を検査して作る。
    pub fn new(name: &str) -> Result<Permission, PermissionError> {
        if name.is_empty() {
            return Err(PermissionError::Empty);
        }
        if name.len() > MAX_PERMISSION_LEN {
            return Err(PermissionError::TooLong);
        }
        if !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(PermissionError::InvalidChar);
        }
        Ok(Permission(name.to_owned()))
    }

    /// 名前を返す。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl PermissionError {
    /// 失敗の種類を英語の ASCII で返す。
    pub const fn as_str(self) -> &'static str {
        match self {
            PermissionError::Empty => "permission name is empty",
            PermissionError::TooLong => "permission name is too long",
            PermissionError::InvalidChar => "permission name has an invalid character",
        }
    }
}

impl fmt::Display for PermissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::error::Error for PermissionError {}

/// [`Declaration`] を作るときの失敗。入力の値は含めず、種類だけを表す。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeclarationError {
    /// 走らせる組み合わせが 1 つも無い。
    NoCombination,
    /// 組み合わせの件数が [`MAX_COMBINATIONS`] を超えた。
    TooManyCombinations,
    /// 許可の件数が [`MAX_PERMISSIONS`] を超えた。
    TooManyPermissions,
    /// 許可の名前が検査を通らなかった。
    Permission(PermissionError),
}

impl DeclarationError {
    /// 失敗の種類を英語の ASCII で返す。
    pub const fn as_str(self) -> &'static str {
        match self {
            DeclarationError::NoCombination => "declaration has no machine combination",
            DeclarationError::TooManyCombinations => "declaration has too many combinations",
            DeclarationError::TooManyPermissions => "declaration has too many permissions",
            DeclarationError::Permission(e) => e.as_str(),
        }
    }
}

impl fmt::Display for DeclarationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::error::Error for DeclarationError {}

impl From<PermissionError> for DeclarationError {
    fn from(e: PermissionError) -> Self {
        DeclarationError::Permission(e)
    }
}

/// 1 つの試験の宣言。走らせる組み合わせは 1 つ以上で、重複は取り除かれる。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Declaration {
    combinations: Vec<MachineCombination>,
    permissions: Vec<Permission>,
}

impl Declaration {
    /// 検査済みの許可から宣言を作る。重複は取り除く（結果は重複の有無に依らない）。
    pub fn new(
        combinations: &[MachineCombination],
        permissions: &[Permission],
    ) -> Result<Declaration, DeclarationError> {
        if combinations.is_empty() {
            return Err(DeclarationError::NoCombination);
        }
        if combinations.len() > MAX_COMBINATIONS {
            return Err(DeclarationError::TooManyCombinations);
        }
        if permissions.len() > MAX_PERMISSIONS {
            return Err(DeclarationError::TooManyPermissions);
        }
        let mut cs: Vec<MachineCombination> = Vec::new();
        for c in combinations {
            if !cs.contains(c) {
                cs.push(*c);
            }
        }
        let mut ps: Vec<Permission> = Vec::new();
        for p in permissions {
            if !ps.contains(p) {
                ps.push(p.clone());
            }
        }
        Ok(Declaration {
            combinations: cs,
            permissions: ps,
        })
    }

    /// 許可を名前で受けて宣言を作る。名前は [`Permission::new`] の規則で検査する。[`declaration!`](crate::declaration) の呼び先。
    pub fn from_parts(
        combinations: &[MachineCombination],
        permission_names: &[&str],
    ) -> Result<Declaration, DeclarationError> {
        if permission_names.len() > MAX_PERMISSIONS {
            return Err(DeclarationError::TooManyPermissions);
        }
        let mut ps = Vec::with_capacity(permission_names.len());
        for n in permission_names {
            ps.push(Permission::new(n)?);
        }
        Declaration::new(combinations, &ps)
    }

    /// 走らせる組み合わせ（重複なし）。
    pub fn combinations(&self) -> &[MachineCombination] {
        &self.combinations
    }

    /// 要る許可（重複なし）。
    pub fn permissions(&self) -> &[Permission] {
        &self.permissions
    }
}
