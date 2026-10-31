//! `texture_format` モジュールの値の一覧の件数を、別の crate から数える公開テスト（TASK-1312）。
//!
//! 本格的な値の照合は別のテストが受け持つ。ここでは `all()` の長さと ASTC の展開の件数だけを確かめる。

use fandhe_silicon_core::texture_format::{AstcBlock, AstcChannel, TextureFormat};

#[test]
fn texture_format_all_has_118_concrete_formats() {
    assert_eq!(TextureFormat::all().len(), 118);
}

#[test]
fn texture_format_all_has_42_astc_formats() {
    let astc = TextureFormat::all()
        .iter()
        .filter(|f| matches!(f, TextureFormat::Astc { .. }))
        .count();
    assert_eq!(astc, 42);
}

#[test]
fn astc_block_all_has_14_values() {
    assert_eq!(AstcBlock::all().len(), 14);
}

#[test]
fn astc_channel_all_has_3_values() {
    assert_eq!(AstcChannel::all().len(), 3);
}
