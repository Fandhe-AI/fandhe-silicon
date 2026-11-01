//! `PixelFormat`・`DepthFormat` から `TextureFormat` への変換を外の crate から確かめる公開テスト（TASK-1312）。
//! feature `graphics` の裏でだけ走る。値の数は `#[non_exhaustive]` で外から数えられないので、core の中の試験が固定する。

use fandhe_silicon_core::graphics::{DepthFormat, PixelFormat};
use fandhe_silicon_core::texture_format::TextureFormat;

fn take<T: Into<TextureFormat>>(t: T) -> TextureFormat {
    t.into()
}

#[test]
fn pixel_format_converts_to_same_name() {
    let cases = [
        (PixelFormat::Rgba8Unorm, TextureFormat::Rgba8Unorm),
        (PixelFormat::Bgra8Unorm, TextureFormat::Bgra8Unorm),
        (PixelFormat::Rgba32Float, TextureFormat::Rgba32Float),
    ];
    for (p, expected) in cases {
        assert_eq!(TextureFormat::from(p), expected);
        let t: TextureFormat = p.into();
        assert_eq!(t, expected);
    }
}

#[test]
fn depth_format_converts_to_same_name() {
    let d = DepthFormat::Depth32Float;
    assert_eq!(TextureFormat::from(d), TextureFormat::Depth32Float);
    let t: TextureFormat = d.into();
    assert_eq!(t, TextureFormat::Depth32Float);
}

#[test]
fn converted_values_are_distinct_and_listed() {
    let converted = [
        TextureFormat::from(PixelFormat::Rgba8Unorm),
        TextureFormat::from(PixelFormat::Bgra8Unorm),
        TextureFormat::from(PixelFormat::Rgba32Float),
        TextureFormat::from(DepthFormat::Depth32Float),
    ];
    for (i, a) in converted.iter().enumerate() {
        assert!(TextureFormat::all().contains(a));
        for b in &converted[i + 1..] {
            assert_ne!(a, b);
        }
    }
}

#[test]
fn conversion_is_usable_as_into_bound() {
    assert_eq!(take(PixelFormat::Bgra8Unorm), TextureFormat::Bgra8Unorm);
    assert_eq!(take(PixelFormat::Rgba32Float), TextureFormat::Rgba32Float);
    assert_eq!(take(DepthFormat::Depth32Float), TextureFormat::Depth32Float);
}
