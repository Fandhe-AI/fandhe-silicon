//! Top-k の比べに使う、名前の付いた追加の入力と手で計算した期待の並び（spec TASK-976 の 4/6）。
//!
//! 検証用のテストから呼ばれ、入力を別の crate・別のテストからも同じ形で使えるようにする。
//! 入力は -0.0 を +0.0 より小さいスロットに置いたもの、k が有限の件数より大きいもの、
//! 有限でない値を含むものの 3 種。期待は [`super::topk::reference_topk_f32`] の規則から手で組み立てた並びで、
//! 参照の実装と照らすのはテスト側の役目。
//! 埋める位置に入れる値はここでは定めない（使う側が渡す）。
//! feature `test-support` の裏にだけ公開される（置き場の説明は `docs/parity-tool.md` の「所在」）。
//! spec との対応: PARITY-4・PARITY-6（ポインタのみ）。

use super::topk::TopKEntry;

/// 名前の付いた Top-k の入力 1 件（スコア列・k・期待の並び）。
///
/// `PartialEq` を実装しない（`f32` を含み、`==` は符号付きの零を区別しないため）。
#[derive(Debug, Clone, Copy)]
pub struct TopKInput {
    name: &'static str,
    scores: &'static [f32],
    k: usize,
    expected: &'static [(u32, f32)],
}

impl TopKInput {
    const fn new(
        name: &'static str,
        scores: &'static [f32],
        k: usize,
        expected: &'static [(u32, f32)],
    ) -> Self {
        Self {
            name,
            scores,
            k,
            expected,
        }
    }

    /// 失敗の表示と記録に使う識別名。
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// 入力のスコア列。
    pub fn scores(&self) -> &'static [f32] {
        self.scores
    }

    /// 選ぶ件数。
    pub fn k(&self) -> usize {
        self.k
    }

    /// 手で計算した期待の並び（有限のスコアだけ。長さは `min(k, 有限の件数)`）。
    pub fn expected(&self) -> Vec<TopKEntry> {
        self.expected
            .iter()
            .map(|&(slot, score)| TopKEntry::new(slot, score))
            .collect()
    }

    /// 有限のスコアの件数。`k` がこれより大きいと、その差が埋める位置の数になる。
    pub fn finite_count(&self) -> usize {
        self.scores.iter().filter(|s| s.is_finite()).count()
    }
}

const NAN_SIGNALING: f32 = f32::from_bits(0x7f80_0001);
const NAN_NEGATIVE: f32 = f32::from_bits(0xffc0_0000);

/// -0.0 を +0.0 より小さいスロット番号に置いた入力。期待では +0.0 が -0.0 より前に来る。
pub const SIGNED_ZERO_INPUTS: &[TopKInput] = &[
    TopKInput::new(
        "signed_zero_adjacent",
        &[-0.0, 0.0],
        2,
        &[(1, 0.0), (0, -0.0)],
    ),
    TopKInput::new(
        "signed_zero_separated",
        &[-0.0, 1.0, 0.0, -1.0],
        4,
        &[(1, 1.0), (2, 0.0), (0, -0.0), (3, -1.0)],
    ),
    TopKInput::new(
        "signed_zero_cut_at_k",
        &[-0.0, 2.0, 0.0],
        2,
        &[(1, 2.0), (2, 0.0)],
    ),
    TopKInput::new(
        "signed_zero_repeated",
        &[-0.0, -0.0, 0.0, 0.0],
        4,
        &[(2, 0.0), (3, 0.0), (0, -0.0), (1, -0.0)],
    ),
];

/// `k` が有限のスコアの件数より大きく、末尾に埋める位置ができる入力。
pub const PAD_POSITION_INPUTS: &[TopKInput] = &[
    TopKInput::new("pad_after_one_finite", &[1.0, f32::NAN], 3, &[(0, 1.0)]),
    TopKInput::new(
        "pad_all_positions",
        &[f32::NAN, f32::INFINITY, f32::NEG_INFINITY],
        2,
        &[],
    ),
    TopKInput::new(
        "pad_after_signed_zeros",
        &[2.0, -0.0, 0.0],
        5,
        &[(0, 2.0), (2, 0.0), (1, -0.0)],
    ),
];

/// 有限でない値（NaN の各ビット列・+Inf・-Inf）を含む入力。期待にはそれらが入らない。
pub const NON_FINITE_INPUTS: &[TopKInput] = &[
    TopKInput::new(
        "non_finite_mixed_with_zeros",
        &[f32::NAN, 1.0, f32::INFINITY, -0.0, f32::NEG_INFINITY, 0.0],
        6,
        &[(1, 1.0), (5, 0.0), (3, -0.0)],
    ),
    TopKInput::new(
        "non_finite_nan_payloads",
        &[NAN_SIGNALING, 3.0, NAN_NEGATIVE, 3.0, f32::INFINITY],
        2,
        &[(1, 3.0), (3, 3.0)],
    ),
    TopKInput::new(
        "non_finite_inf_first",
        &[f32::INFINITY, 0.5, f32::NEG_INFINITY, 0.25],
        2,
        &[(1, 0.5), (3, 0.25)],
    ),
];
