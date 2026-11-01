//! テクスチャの形式の問合せの鍵の一覧と行の表の網羅・名前の規則を、外の crate から確かめる公開テスト（TASK-1313）。
//!
//! 鍵は 118 件、行は 77 行（`"Astc"` の行だけ 42 件の鍵を持ち、ほかは 1 件）。
//! 形式の値側の期待は `TextureFormat::all()` と `Debug` の綴りから取る。

use fandhe_silicon_core::texture_format::{AstcBlock, AstcChannel, TextureFormat};
use fandhe_silicon_core::texture_format_key::{TEXTURE_FORMAT_QUERY_KEYS, TEXTURE_FORMAT_ROWS};
use std::collections::HashSet;

const ROW_NAMES: [&str; 77] = [
    "R8Unorm",
    "R8Snorm",
    "R8Uint",
    "R8Sint",
    "R16Uint",
    "R16Sint",
    "R16Unorm",
    "R16Snorm",
    "R16Float",
    "Rg8Unorm",
    "Rg8Snorm",
    "Rg8Uint",
    "Rg8Sint",
    "R32Uint",
    "R32Sint",
    "R32Float",
    "Rg16Uint",
    "Rg16Sint",
    "Rg16Unorm",
    "Rg16Snorm",
    "Rg16Float",
    "Rgba8Unorm",
    "Rgba8UnormSrgb",
    "Rgba8Snorm",
    "Rgba8Uint",
    "Rgba8Sint",
    "Bgra8Unorm",
    "Bgra8UnormSrgb",
    "Rgb9e5Ufloat",
    "Rgb10a2Uint",
    "Rgb10a2Unorm",
    "Rg11b10Ufloat",
    "R64Uint",
    "Rg32Uint",
    "Rg32Sint",
    "Rg32Float",
    "Rgba16Uint",
    "Rgba16Sint",
    "Rgba16Unorm",
    "Rgba16Snorm",
    "Rgba16Float",
    "Rgba32Uint",
    "Rgba32Sint",
    "Rgba32Float",
    "Stencil8",
    "Depth16Unorm",
    "Depth24Plus",
    "Depth24PlusStencil8",
    "Depth32Float",
    "Depth32FloatStencil8",
    "NV12",
    "P010",
    "Bc1RgbaUnorm",
    "Bc1RgbaUnormSrgb",
    "Bc2RgbaUnorm",
    "Bc2RgbaUnormSrgb",
    "Bc3RgbaUnorm",
    "Bc3RgbaUnormSrgb",
    "Bc4RUnorm",
    "Bc4RSnorm",
    "Bc5RgUnorm",
    "Bc5RgSnorm",
    "Bc6hRgbUfloat",
    "Bc6hRgbFloat",
    "Bc7RgbaUnorm",
    "Bc7RgbaUnormSrgb",
    "Etc2Rgb8Unorm",
    "Etc2Rgb8UnormSrgb",
    "Etc2Rgb8A1Unorm",
    "Etc2Rgb8A1UnormSrgb",
    "Etc2Rgba8Unorm",
    "Etc2Rgba8UnormSrgb",
    "EacR11Unorm",
    "EacR11Snorm",
    "EacRg11Unorm",
    "EacRg11Snorm",
    "Astc",
];

/// 形式の値から期待する鍵の文字列を作る。
fn expected_key(f: &TextureFormat) -> String {
    if let TextureFormat::Astc { block, channel } = f {
        format!("texformat:Astc.{block:?}.{channel:?}")
    } else {
        format!("texformat:{f:?}")
    }
}

#[test]
fn query_keys_are_118_and_unique() {
    assert_eq!(TEXTURE_FORMAT_QUERY_KEYS.len(), 118);
    let set: HashSet<&str> = TEXTURE_FORMAT_QUERY_KEYS.iter().map(|k| k.0).collect();
    assert_eq!(set.len(), 118);
}

#[test]
fn query_keys_follow_all_formats() {
    let all = TextureFormat::all();
    assert_eq!(TEXTURE_FORMAT_QUERY_KEYS.len(), all.len());
    for (i, f) in all.iter().enumerate() {
        assert_eq!(TEXTURE_FORMAT_QUERY_KEYS[i].0, expected_key(f), "{f:?}");
    }
}

#[test]
fn rows_are_77_with_unique_names() {
    assert_eq!(TEXTURE_FORMAT_ROWS.len(), 77);
    let rows: HashSet<&str> = TEXTURE_FORMAT_ROWS.iter().map(|r| r.row).collect();
    assert_eq!(rows.len(), 77);
    let expected: HashSet<&str> = ROW_NAMES.into_iter().collect();
    assert_eq!(rows, expected);
}

#[test]
fn rows_flatten_to_query_keys() {
    let all = TextureFormat::all();
    let flat: Vec<_> = TEXTURE_FORMAT_ROWS
        .iter()
        .flat_map(|r| r.keys.iter())
        .collect();
    assert_eq!(flat.len(), 118);
    for (i, k) in flat.iter().enumerate() {
        assert_eq!(k.key, TEXTURE_FORMAT_QUERY_KEYS[i]);
        assert_eq!(k.format, all[i]);
    }
}

#[test]
fn non_astc_rows_have_one_key_named_after_row() {
    let mut count = 0;
    for r in TEXTURE_FORMAT_ROWS.iter().filter(|r| r.row != "Astc") {
        count += 1;
        assert_eq!(r.keys.len(), 1, "{}", r.row);
        assert_eq!(r.keys[0].key.0, format!("texformat:{}", r.row));
        assert_eq!(format!("{:?}", r.keys[0].format), r.row);
    }
    assert_eq!(count, 76);
}

#[test]
fn astc_row_has_block_by_channel_keys() {
    let row = TEXTURE_FORMAT_ROWS
        .iter()
        .find(|r| r.row == "Astc")
        .expect("Astc row");
    assert_eq!(row.keys.len(), 42);
    let mut i = 0;
    for block in AstcBlock::all() {
        for channel in AstcChannel::all() {
            assert_eq!(
                row.keys[i].key.0,
                format!("texformat:Astc.{block:?}.{channel:?}")
            );
            assert_eq!(
                row.keys[i].format,
                TextureFormat::Astc {
                    block: *block,
                    channel: *channel
                }
            );
            i += 1;
        }
    }
    assert_eq!(i, 42);
}

#[test]
fn astc_row_key_is_not_listed() {
    assert!(
        TEXTURE_FORMAT_QUERY_KEYS
            .iter()
            .all(|k| k.0 != "texformat:Astc")
    );
}

#[test]
fn keys_use_limited_characters() {
    for k in TEXTURE_FORMAT_QUERY_KEYS {
        let rest = k.0.strip_prefix("texformat:").expect("prefix");
        assert!(!rest.is_empty());
        assert!(
            rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '.'),
            "{}",
            k.0
        );
    }
}
