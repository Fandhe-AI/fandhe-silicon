//! 同じ名前の 4 値が `TextureFormat` にあり、`Debug` の綴りが名前と合うことを別の crate から確かめる公開テスト（TASK-1312）。
//!
//! 値が作れること（コンパイルが通ること）、`Debug` の綴りが名前どおりであること、
//! `TextureFormat::all()` に含まれることを 1 値ずつ見る。

use fandhe_silicon_core::texture_format::TextureFormat;

/// 値と、その `Debug` の綴りとして期待する名前の組を返す。
fn four_values() -> [(TextureFormat, &'static str); 4] {
    [
        (TextureFormat::Rgba8Unorm, "Rgba8Unorm"),
        (TextureFormat::Bgra8Unorm, "Bgra8Unorm"),
        (TextureFormat::Rgba32Float, "Rgba32Float"),
        (TextureFormat::Depth32Float, "Depth32Float"),
    ]
}

#[test]
fn four_values_debug_spelling_matches_name() {
    for (value, name) in four_values() {
        assert_eq!(format!("{value:?}"), name, "{name} の Debug の綴り");
    }
}

#[test]
fn four_values_are_listed_in_all() {
    for (value, name) in four_values() {
        assert!(
            TextureFormat::all().contains(&value),
            "{name} が all() に無い"
        );
    }
}
