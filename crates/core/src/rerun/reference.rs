//! Node での照合の行（`wasm/node/cases.json`）を、流し直しの照合 1 件へ写す部品（spec TASK-29。範囲は BUILD-41 の再現の枠）。
//!
//! スクリプト `scripts/wasm-node-check.sh` が出す行の状態の語（`match`・`mismatch`・`not-run` と理由）を
//! 文字列のトークンとして受け、`test_frame` の 5 種のラベルへ写す純粋な変換だけを持つ。
//! ファイル・環境変数・標準出力・子プロセス（Node）には触れず、入力の値はエラーにも `Display` にも入れない。
//! 組み合わせは決めない（宣言は呼び出し側が渡す）。
//! スクリプト側の行の状態 → ラベルの写しの表は `scripts/wasm-node-check.sh` の `row-labels` にあり、
//! この型との食い違いは `tests/rerun_node_record.rs` が検出する。

use super::check::Check;
use crate::test_frame::{
    Declaration, MachineCombination, ReferenceGap, RunResult, Subject, Verdict,
};

/// 照合の行の参照の出どころ。綴りは `wasm/node/cases.json` の `reference_source` と同じ（完全一致）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReferenceSource {
    /// 行に書いた固定値。
    Fixed,
    /// ネイティブの NEON で作る値。
    NativeNeon,
}

impl ReferenceSource {
    /// 全ての出どころ。重複は無い。
    pub const ALL: [ReferenceSource; 2] = [ReferenceSource::Fixed, ReferenceSource::NativeNeon];

    /// 出どころのトークン（`[a-z0-9-]`）。
    pub const fn name(self) -> &'static str {
        match self {
            ReferenceSource::Fixed => "fixed",
            ReferenceSource::NativeNeon => "native-neon",
        }
    }

    /// トークンから出どころを得る。完全一致だけを受け、空白の除去も大文字小文字の読み替えもしない。
    pub fn from_name(s: &str) -> Option<ReferenceSource> {
        ReferenceSource::ALL.into_iter().find(|r| r.name() == s)
    }
}

/// 参照の欠けの判定。出どころを先に見て、次に参照の有無を見る（スクリプトの規則と同じ順）。
///
/// `producible` に無い出どころは、参照の値があっても比べない（`Unavailable`）。
/// 出どころが作れて参照が無ければ `Missing`、どちらも満たせば `None`。
pub fn reference_gap(
    source: ReferenceSource,
    has_reference: bool,
    producible: &[ReferenceSource],
) -> Option<ReferenceGap> {
    if !producible.contains(&source) {
        Some(ReferenceGap::Unavailable)
    } else if !has_reference {
        Some(ReferenceGap::Missing)
    } else {
        None
    }
}

/// 照合の行 1 件の状態。スクリプトの出力の語に対応する。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowState {
    /// 一致した。
    Match,
    /// 一致しなかった。
    Mismatch,
    /// 参照が無い・作れないので比べなかった。
    NotRun(ReferenceGap),
}

impl RowState {
    /// スクリプトの状態のトークンと理由のトークンから作る。完全一致だけを受ける。
    ///
    /// `match`・`mismatch` は理由なし、`not-run` は `reference-missing`・`reference-unavailable` のどちらかが要る。
    /// それ以外（理由の無い `not-run`・未知の語・`match` に付いた理由）は `None`。
    pub fn from_tokens(state: &str, reason: Option<&str>) -> Option<RowState> {
        match (state, reason) {
            ("match", None) => Some(RowState::Match),
            ("mismatch", None) => Some(RowState::Mismatch),
            ("not-run", Some("reference-missing")) => Some(RowState::NotRun(ReferenceGap::Missing)),
            ("not-run", Some("reference-unavailable")) => {
                Some(RowState::NotRun(ReferenceGap::Unavailable))
            }
            _ => None,
        }
    }

    /// 本体の戻り値へ写す。`Match`→一致、`Mismatch`→不一致、`NotRun`→参照の欠け。
    pub const fn run_result(self) -> RunResult {
        match self {
            RowState::Match => RunResult::Verdict(Verdict::Match),
            RowState::Mismatch => RunResult::Verdict(Verdict::Mismatch),
            RowState::NotRun(g) => RunResult::NoReference(g),
        }
    }
}

/// 照合の行 1 件を [`Check`] にしたもの。状態は作る時点で決まっていて、`run` は写した結果を返すだけ。
#[derive(Clone, Debug)]
pub struct RowCheck {
    subject: Subject,
    declaration: Declaration,
    state: RowState,
}

impl RowCheck {
    /// 対象・宣言・行の状態から作る。宣言（走らせる組み合わせ）は呼び出し側が決める。
    pub fn new(subject: Subject, declaration: Declaration, state: RowState) -> RowCheck {
        RowCheck {
            subject,
            declaration,
            state,
        }
    }

    /// 行の状態。
    pub const fn state(&self) -> RowState {
        self.state
    }
}

impl Check for RowCheck {
    fn subject(&self) -> &Subject {
        &self.subject
    }

    fn declaration(&self) -> &Declaration {
        &self.declaration
    }

    fn run(&self, _on: MachineCombination) -> RunResult {
        self.state.run_result()
    }
}
