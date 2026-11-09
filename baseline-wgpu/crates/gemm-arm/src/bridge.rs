//! 下位の API（wgpu の実験的な機能の同意の印）の unsafe な口を呼ぶ橋（spec TASK-1010 のポインタ）。
//!
//! この crate で `unsafe` を持つのはこの module だけ。`device` の経路が device の要求に渡す値をここから受け取る。

/// 実験的な機能を使う同意の印を作る。
pub(crate) fn experimental_features() -> wgpu::ExperimentalFeatures {
    // SAFETY: 実験的な機能として要求するのは cooperative matrix だけで、使う口は
    // cooperative matrix の WGSL の作成と dispatch だけ。device の要求は検査つきの
    // シェーダーの作成だけを使い、検査を外す口は使わない。
    unsafe { wgpu::ExperimentalFeatures::enabled() }
}
