//! 閾値の ID と、承認の有無の表。
//!
//! 結果の決め方（`outcome::judge`）が、宣言された閾値に未承認があるかを調べるときに呼ぶ。
//! 承認の有無は下の定数 [`APPROVED_THRESHOLDS`] の 1 か所だけが持ち、表に無い ID は未承認として扱う
//! （fail-closed。判定保留に倒れる）。承認の判定を試験の側から差し替える口は公開していない。
//! 表の根拠の所在は `docs/spec/05-tasks.md` の TASK-53（ポインタ）。

use std::fmt;

/// 承認済みの閾値の ID の表（唯一の置き場）。載っていない ID は未承認で、判定保留に倒れる。
///
/// 閾値の数字は持たない（ID の綴りだけ）。
pub const APPROVED_THRESHOLDS: [&str; 12] = [
    "t-1", "t-2", "t-3", "t-5", "t-7", "t-8", "t-9", "t-10", "t-11", "t-15", "t-16", "t-17",
];

/// 閾値の ID の検査の失敗。入力の値は含めず、種類だけを表す。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThresholdIdError {
    /// 正規の綴り（`t-` と、先頭が 0 でない 1〜3 桁の数字）でない。
    Invalid,
}

impl ThresholdIdError {
    /// 失敗の種類を英語の ASCII で返す。
    pub const fn as_str(self) -> &'static str {
        match self {
            ThresholdIdError::Invalid => "threshold id is not in the canonical form",
        }
    }
}

impl fmt::Display for ThresholdIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::error::Error for ThresholdIdError {}

/// 検査済みの閾値の ID。正規の綴りは `t-` に、先頭が 0 でない 1〜3 桁の数字を続けたものだけ。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ThresholdId(String);

impl ThresholdId {
    /// 綴りを検査して作る。前後の空白・先頭の 0・大文字は受けない。
    pub fn new(name: &str) -> Result<ThresholdId, ThresholdIdError> {
        let digits = name
            .strip_prefix("t-")
            .ok_or(ThresholdIdError::Invalid)?
            .as_bytes();
        let ok = (1..=3).contains(&digits.len())
            && digits[0] != b'0'
            && digits.iter().all(u8::is_ascii_digit);
        if !ok {
            return Err(ThresholdIdError::Invalid);
        }
        Ok(ThresholdId(name.to_owned()))
    }

    /// 綴りを返す。
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 承認の有無の表に載っているか。表に無い ID は `false`（未承認）。
    pub fn is_approved(&self) -> bool {
        APPROVED_THRESHOLDS.contains(&self.0.as_str())
    }
}
