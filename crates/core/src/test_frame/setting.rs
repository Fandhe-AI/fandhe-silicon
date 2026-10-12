//! いまの実行がどの組み合わせで、どの許可を得ているか（手順のスクリプトが設定する環境変数から作る）。
//!
//! 環境変数の値は信頼しない入力として扱う。完全一致の列挙と文字種の検査だけに使い、
//! コマンドライン・パス・ソースの組み立てには使わず、エラーや表示にも入れない。
//! 不正な値は失敗にせず「不正」の状態として持ち、`decision::decide` が走らせない側に倒す。

use super::combination::MachineCombination;
use super::declaration::{MAX_PERMISSIONS, Permission};
use std::ffi::OsString;

/// 組み合わせの名前を渡す環境変数の名前。
pub const COMBINATION_ENV: &str = "FANDHE_SILICON_TEST_MACHINE_COMBINATION";
/// 得ている許可の名前をカンマ区切りで渡す環境変数の名前。
pub const PERMISSIONS_ENV: &str = "FANDHE_SILICON_TEST_PERMISSIONS";

/// 組み合わせの名前の値の長さの上限（バイト）。
const MAX_COMBINATION_VALUE_LEN: usize = 64;
/// 許可の値全体の長さの上限（バイト）。
const MAX_PERMISSIONS_VALUE_LEN: usize = 1024;

/// 組み合わせの設定の状態。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CombinationSetting {
    /// 未設定。
    Unset,
    /// 設定はあるが、空・UTF-8 でない・長すぎる・知らない値のいずれか。
    Invalid,
    /// 有効な組み合わせ。
    Set(MachineCombination),
}

/// 許可の設定の状態。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PermissionSetting {
    /// 設定はあるが、不正な要素・長すぎる・件数の超過・UTF-8 でないのいずれか。
    Invalid,
    /// 得ている許可（未設定と空文字は空の集まり）。
    Granted(Vec<Permission>),
}

/// いまの実行の設定。生成は失敗しない。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunSetting {
    pub(crate) combination: CombinationSetting,
    pub(crate) permissions: PermissionSetting,
}

impl RunSetting {
    /// 名前から値を引く関数を受けて設定を作る。検査と分類はすべてここで行う（テストはここへ値を渡す）。
    pub fn from_lookup<F>(lookup: F) -> RunSetting
    where
        F: Fn(&str) -> Option<OsString>,
    {
        RunSetting {
            combination: parse_combination(lookup(COMBINATION_ENV)),
            permissions: parse_permissions(lookup(PERMISSIONS_ENV)),
        }
    }

    /// プロセスの環境変数から設定を作る。`from_lookup` へ `std::env::var_os` を渡すだけ。
    pub fn from_env() -> RunSetting {
        RunSetting::from_lookup(|name| std::env::var_os(name))
    }
}

fn parse_combination(value: Option<OsString>) -> CombinationSetting {
    let Some(v) = value else {
        return CombinationSetting::Unset;
    };
    let Some(s) = v.to_str() else {
        return CombinationSetting::Invalid;
    };
    if s.len() > MAX_COMBINATION_VALUE_LEN {
        return CombinationSetting::Invalid;
    }
    match MachineCombination::from_name(s) {
        Some(c) => CombinationSetting::Set(c),
        None => CombinationSetting::Invalid,
    }
}

fn parse_permissions(value: Option<OsString>) -> PermissionSetting {
    let Some(v) = value else {
        return PermissionSetting::Granted(Vec::new());
    };
    let Some(s) = v.to_str() else {
        return PermissionSetting::Invalid;
    };
    if s.len() > MAX_PERMISSIONS_VALUE_LEN {
        return PermissionSetting::Invalid;
    }
    if s.is_empty() {
        return PermissionSetting::Granted(Vec::new());
    }
    let mut granted: Vec<Permission> = Vec::new();
    for (count, item) in s.split(',').enumerate() {
        if count >= MAX_PERMISSIONS {
            return PermissionSetting::Invalid;
        }
        match Permission::new(item) {
            Ok(p) => {
                if !granted.contains(&p) {
                    granted.push(p);
                }
            }
            Err(_) => return PermissionSetting::Invalid,
        }
    }
    PermissionSetting::Granted(granted)
}
