//! テクスチャの記述子の検査と操作の時点の検査を、別の crate から公開のパスだけで項目ごとに確かめる（TASK-1942 のポインタ）。
//!
//! feature なしで流れる。拒否は `ContractError::InvalidArgument` の種類だけを照らし、文言は照らさない。

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::texture::{
    TextureDescriptor, TextureDimension, TextureExtent, TextureUsage, TextureUsageRole,
    max_mip_level_count, validate_texture_descriptor, validate_texture_usage_for_role,
    validate_texture_view_format,
};
use fandhe_silicon_core::texture_format::{AstcBlock, AstcChannel, TextureFormat};

use TextureDimension::{D1, D2, D3};

/// 4 つの上限は互いに違う値にして、引数の取り違えで落ちる形にする。
const L1: u32 = 4096;
const L2: u32 = 8192;
const L3: u32 = 1024;
const LAYERS: u32 = 256;

const ALL_USAGES: [TextureUsage; 7] = [
    TextureUsage::COPY_SRC,
    TextureUsage::COPY_DST,
    TextureUsage::TEXTURE_BINDING,
    TextureUsage::STORAGE_BINDING,
    TextureUsage::RENDER_ATTACHMENT,
    TextureUsage::TRANSIENT_ATTACHMENT,
    TextureUsage::STORAGE_ATOMIC,
];

fn all_usage() -> TextureUsage {
    ALL_USAGES
        .iter()
        .fold(TextureUsage::empty(), |acc, u| acc | *u)
}

/// 7 つの用途の和から `removed` のビットを除いたもの。
fn all_but(removed: TextureUsage) -> TextureUsage {
    ALL_USAGES
        .iter()
        .filter(|u| **u != removed)
        .fold(TextureUsage::empty(), |acc, u| acc | *u)
}

fn ext(width: u32, height: u32, depth_or_array_layers: u32) -> TextureExtent {
    TextureExtent {
        width,
        height,
        depth_or_array_layers,
    }
}

fn desc(
    dim: TextureDimension,
    size: TextureExtent,
    format: TextureFormat,
    usage: TextureUsage,
) -> TextureDescriptor<'static> {
    TextureDescriptor::new(dim, size, format, usage)
}

/// 色の形式・`TEXTURE_BINDING` の通る記述子。
fn plain(dim: TextureDimension, w: u32, h: u32, d: u32) -> TextureDescriptor<'static> {
    desc(
        dim,
        ext(w, h, d),
        TextureFormat::Rgba8Unorm,
        TextureUsage::TEXTURE_BINDING,
    )
}

fn check(d: &TextureDescriptor<'_>) -> Result<(), ContractError> {
    validate_texture_descriptor(d, L1, L2, L3, LAYERS)
}

fn is_invalid<T>(r: &Result<T, ContractError>) -> bool {
    matches!(r, Err(ContractError::InvalidArgument(_)))
}

fn assert_ok(d: &TextureDescriptor<'_>) {
    assert_eq!(check(d), Ok(()), "{d:?}");
}

fn assert_rejected(d: &TextureDescriptor<'_>) {
    assert!(is_invalid(&check(d)), "{d:?} -> {:?}", check(d));
}

/// 多重化できる描画先の記述子（サンプル数だけを変えて使う）。
fn msaa(samples: u32) -> TextureDescriptor<'static> {
    let mut d = desc(
        D2,
        ext(64, 64, 1),
        TextureFormat::Rgba8Unorm,
        TextureUsage::RENDER_ATTACHMENT,
    );
    d.sample_count = samples;
    d
}

fn transient() -> TextureDescriptor<'static> {
    desc(
        D2,
        ext(64, 64, 1),
        TextureFormat::Rgba8Unorm,
        TextureUsage::TRANSIENT_ATTACHMENT | TextureUsage::RENDER_ATTACHMENT,
    )
}

fn astc_6x6(channel: AstcChannel) -> TextureFormat {
    TextureFormat::Astc {
        block: AstcBlock::B6x6,
        channel,
    }
}

/// TEX-59: 用途が空の記述子は 3 つの次元とも拒まれる。
#[test]
fn empty_usage_is_rejected() {
    for dim in [D1, D2, D3] {
        let mut d = plain(dim, 4, 1, 1);
        assert_ok(&d);
        d.usage = TextureUsage::empty();
        assert_rejected(&d);
    }
}

/// TEX-60: 1 次元の幅の上限と、1 次元で拒む組。
#[test]
fn d1_limits() {
    assert_ok(&plain(D1, L1, 1, 1));
    assert_rejected(&plain(D1, L1 + 1, 1, 1));
    assert_rejected(&plain(D1, 8, 2, 1));
    assert_rejected(&plain(D1, 8, 1, 2));
    let mut d = plain(D1, 8, 1, 1);
    d.sample_count = 2;
    assert_rejected(&d);
    let mut d = plain(D1, 8, 1, 1);
    d.mip_level_count = 2;
    assert_rejected(&d);
    let mut d = plain(D1, 8, 1, 1);
    d.format = TextureFormat::Depth32Float;
    assert_rejected(&d);
    let mut d = plain(D1, 8, 1, 1);
    d.format = TextureFormat::Bc1RgbaUnorm;
    assert_rejected(&d);
    let mut d = plain(D1, 8, 1, 1);
    d.usage = TextureUsage::RENDER_ATTACHMENT;
    assert_rejected(&d);
    let mut d = plain(D1, 8, 1, 1);
    d.usage = TextureUsage::TRANSIENT_ATTACHMENT;
    assert_rejected(&d);
}

/// TEX-61: 2 次元の幅・高さ・層の上限。
#[test]
fn d2_limits() {
    assert_ok(&plain(D2, L2, L2, 1));
    assert_ok(&plain(D2, 1, 1, LAYERS));
    assert_rejected(&plain(D2, L2 + 1, 1, 1));
    assert_rejected(&plain(D2, 1, L2 + 1, 1));
    assert_rejected(&plain(D2, 1, 1, LAYERS + 1));
    assert_rejected(&plain(D2, 0, 1, 1));
    assert_rejected(&plain(D2, 1, 0, 1));
    assert_rejected(&plain(D2, 1, 1, 0));
}

/// TEX-62: 3 次元の 3 辺の上限と、3 次元で拒む組・受ける組。
#[test]
fn d3_limits() {
    assert_ok(&plain(D3, L3, L3, L3));
    assert_rejected(&plain(D3, L3 + 1, 1, 1));
    assert_rejected(&plain(D3, 1, L3 + 1, 1));
    assert_rejected(&plain(D3, 1, 1, L3 + 1));
    let mut d = plain(D3, 4, 4, 4);
    d.sample_count = 2;
    assert_rejected(&d);
    let mut d = plain(D3, 4, 4, 4);
    d.format = TextureFormat::Depth32Float;
    assert_rejected(&d);
    let mut d = plain(D3, 4, 4, 4);
    d.usage = TextureUsage::TRANSIENT_ATTACHMENT;
    assert_rejected(&d);
    let size = ext(8, 8, 2);
    let u = TextureUsage::TEXTURE_BINDING;
    assert_rejected(&desc(D3, size, TextureFormat::Etc2Rgb8Unorm, u));
    assert_ok(&desc(D3, size, TextureFormat::Bc1RgbaUnorm, u));
    assert_ok(&desc(D3, ext(12, 12, 2), astc_6x6(AstcChannel::Unorm), u));
    let mut d = plain(D3, 4, 4, 4);
    d.usage = TextureUsage::RENDER_ATTACHMENT;
    assert_ok(&d);
}

/// TEX-63: mip の段数の上限と、その式の値。
#[test]
fn mip_level_count_limits() {
    let with_mips = |mut d: TextureDescriptor<'static>, m: u32| {
        d.mip_level_count = m;
        d
    };
    let d2 = plain(D2, 1280, 720, 1);
    assert_ok(&with_mips(d2.clone(), 11));
    assert_rejected(&with_mips(d2.clone(), 12));
    assert_rejected(&with_mips(d2, 0));
    let d3 = plain(D3, 4, 4, 32);
    assert_ok(&with_mips(d3.clone(), 6));
    assert_rejected(&with_mips(d3.clone(), 7));
    assert_rejected(&with_mips(d3, 0));

    let wide = plain(D2, 65536, 1, 1);
    let wide_check =
        |d: &TextureDescriptor<'_>| validate_texture_descriptor(d, L1, 65536, L3, LAYERS);
    assert_eq!(wide_check(&with_mips(wide.clone(), 16)), Ok(()));
    assert!(is_invalid(&wide_check(&with_mips(wide, 17))));

    assert_eq!(max_mip_level_count(D2, ext(1280, 720, 1)), 11);
    assert_eq!(max_mip_level_count(D2, ext(65536, 1, 1)), 16);
    assert_eq!(max_mip_level_count(D3, ext(4, 4, 32)), 6);
    assert_eq!(max_mip_level_count(D1, ext(4096, 1, 1)), 1);
}

/// TEX-64: サンプル数の値と、1 より大きいときの組。
#[test]
fn sample_count_values() {
    for s in [1, 2, 4, 8, 16, 32] {
        assert_ok(&msaa(s));
    }
    for s in [0, 3, 64] {
        assert_rejected(&msaa(s));
    }
    let mut d = msaa(4);
    d.mip_level_count = 2;
    assert_rejected(&d);
    let mut d = msaa(4);
    d.usage |= TextureUsage::STORAGE_BINDING;
    assert_rejected(&d);
    let mut d = msaa(4);
    d.usage = TextureUsage::TEXTURE_BINDING;
    assert_rejected(&d);
}

/// TEX-56: 一時的な描画先の制約。
#[test]
fn transient_attachment_constraints() {
    assert_ok(&transient());
    let mut d = transient();
    d.sample_count = 4;
    assert_ok(&d);

    let mut d = transient();
    d.usage |= TextureUsage::COPY_SRC;
    assert_rejected(&d);
    let mut d = transient();
    d.usage = TextureUsage::TRANSIENT_ATTACHMENT;
    assert_rejected(&d);
    let mut d = transient();
    d.mip_level_count = 2;
    assert_rejected(&d);
    let mut d = transient();
    d.size = ext(64, 64, 2);
    assert_rejected(&d);
    let mut d = transient();
    d.view_formats = &[TextureFormat::Rgba8UnormSrgb];
    assert_rejected(&d);
}

fn with_views<'a>(
    format: TextureFormat,
    size: TextureExtent,
    views: &'a [TextureFormat],
) -> TextureDescriptor<'a> {
    let mut d = desc(D2, size, format, TextureUsage::TEXTURE_BINDING);
    d.view_formats = views;
    d
}

/// TEX-65: `view_formats` の互換と、操作の時点の view の形式。
#[test]
fn view_formats_compatibility() {
    let one = ext(4, 4, 1);
    let rgba = TextureFormat::Rgba8Unorm;
    assert_ok(&with_views(rgba, one, &[TextureFormat::Rgba8UnormSrgb]));
    assert_ok(&with_views(rgba, one, &[TextureFormat::Rgba8Unorm]));
    assert_rejected(&with_views(rgba, one, &[TextureFormat::Bgra8Unorm]));
    assert_rejected(&with_views(
        rgba,
        one,
        &[TextureFormat::Rgba8UnormSrgb, TextureFormat::Bgra8Unorm],
    ));
    assert_ok(&with_views(
        TextureFormat::Bgra8UnormSrgb,
        one,
        &[TextureFormat::Bgra8Unorm],
    ));
    let astc_size = ext(12, 12, 1);
    let unorm = astc_6x6(AstcChannel::Unorm);
    assert_ok(&with_views(
        unorm,
        astc_size,
        &[astc_6x6(AstcChannel::UnormSrgb)],
    ));
    assert_rejected(&with_views(unorm, astc_size, &[astc_6x6(AstcChannel::Hdr)]));

    let srgb = TextureFormat::Rgba8UnormSrgb;
    assert_eq!(validate_texture_view_format(rgba, &[srgb], rgba), Ok(()));
    assert_eq!(validate_texture_view_format(rgba, &[srgb], srgb), Ok(()));
    assert!(is_invalid(&validate_texture_view_format(
        rgba,
        &[srgb],
        TextureFormat::Bgra8Unorm
    )));
    assert_eq!(validate_texture_view_format(rgba, &[], rgba), Ok(()));
    assert!(is_invalid(&validate_texture_view_format(rgba, &[], srgb)));
}

/// TEX-59: 圧縮形式の大きさはブロックの倍数。
#[test]
fn compressed_block_multiple() {
    let u = TextureUsage::TEXTURE_BINDING;
    let bc = TextureFormat::Bc1RgbaUnorm;
    assert_ok(&desc(D2, ext(8, 8, 1), bc, u));
    assert_rejected(&desc(D2, ext(6, 8, 1), bc, u));
    assert_rejected(&desc(D2, ext(8, 6, 1), bc, u));
    let astc = astc_6x6(AstcChannel::Unorm);
    assert_ok(&desc(D2, ext(12, 12, 1), astc, u));
    assert_rejected(&desc(D2, ext(8, 8, 1), astc, u));
}

/// TEX-50〜TEX-54・TEX-58: 役割ごとの要る用途と、atomic の 2 回呼び。
#[test]
fn usage_roles() {
    assert_eq!(TextureUsageRole::ALL.len(), 6);
    for &role in TextureUsageRole::ALL {
        let need = role.required_usage();
        assert_eq!(validate_texture_usage_for_role(need, role), Ok(()));
        assert_eq!(validate_texture_usage_for_role(all_usage(), role), Ok(()));
        assert!(is_invalid(&validate_texture_usage_for_role(
            all_but(need),
            role
        )));
        assert!(is_invalid(&validate_texture_usage_for_role(
            TextureUsage::empty(),
            role
        )));
    }
    let atomic = TextureUsage::STORAGE_ATOMIC;
    assert!(is_invalid(&validate_texture_usage_for_role(
        atomic,
        TextureUsageRole::StorageBinding
    )));
    let both = TextureUsage::STORAGE_BINDING | TextureUsage::STORAGE_ATOMIC;
    assert_eq!(
        validate_texture_usage_for_role(both, TextureUsageRole::StorageBinding),
        Ok(())
    );
    assert_eq!(
        validate_texture_usage_for_role(both, TextureUsageRole::StorageAtomic),
        Ok(())
    );
    assert!(is_invalid(&validate_texture_usage_for_role(
        TextureUsage::STORAGE_BINDING,
        TextureUsageRole::StorageAtomic
    )));
}

/// TEX-54: 描画先の用途は 2 次元と 3 次元で受ける。
#[test]
fn render_attachment_dimension() {
    let u = TextureUsage::RENDER_ATTACHMENT;
    let f = TextureFormat::Rgba8Unorm;
    assert_rejected(&desc(D1, ext(8, 1, 1), f, u));
    assert_ok(&desc(D2, ext(8, 8, 1), f, u));
    assert_ok(&desc(D3, ext(8, 8, 2), f, u));
}

/// 拒否の記述子の一覧。
fn rejected_descriptors() -> Vec<TextureDescriptor<'static>> {
    let mut v = vec![
        plain(D2, L2 + 1, 1, 1),
        plain(D1, 8, 2, 1),
        plain(D3, 1, 1, L3 + 1),
        msaa(3),
        desc(
            D1,
            ext(8, 1, 1),
            TextureFormat::Rgba8Unorm,
            TextureUsage::RENDER_ATTACHMENT,
        ),
        desc(
            D2,
            ext(6, 8, 1),
            TextureFormat::Bc1RgbaUnorm,
            TextureUsage::TEXTURE_BINDING,
        ),
    ];
    let mut empty = plain(D2, 4, 4, 1);
    empty.usage = TextureUsage::empty();
    v.push(empty);
    let mut mips = plain(D2, 1280, 720, 1);
    mips.mip_level_count = 12;
    v.push(mips);
    let mut t = transient();
    t.mip_level_count = 2;
    v.push(t);
    let mut views = plain(D2, 4, 4, 1);
    views.view_formats = &[TextureFormat::Bgra8Unorm];
    v.push(views);
    v
}

/// 同じ引数で 2 回呼ぶと同じ結果になる。
#[test]
fn rejection_is_repeatable() {
    for d in rejected_descriptors() {
        let first = check(&d);
        assert!(first.is_err(), "{d:?}");
        assert_eq!(first, check(&d));
    }
    for d in [plain(D2, 4, 4, 1), msaa(4), transient()] {
        assert_eq!(check(&d), Ok(()));
        assert_eq!(check(&d), Ok(()));
    }
    for &role in TextureUsageRole::ALL {
        let a = validate_texture_usage_for_role(TextureUsage::empty(), role);
        assert!(a.is_err());
        assert_eq!(
            a,
            validate_texture_usage_for_role(TextureUsage::empty(), role)
        );
        let ok = role.required_usage();
        assert_eq!(validate_texture_usage_for_role(ok, role), Ok(()));
        assert_eq!(validate_texture_usage_for_role(ok, role), Ok(()));
    }
    let (rgba, srgb) = (TextureFormat::Rgba8Unorm, TextureFormat::Rgba8UnormSrgb);
    let a = validate_texture_view_format(rgba, &[], srgb);
    assert!(a.is_err());
    assert_eq!(a, validate_texture_view_format(rgba, &[], srgb));
}

/// 拒否はどれも `InvalidArgument` だけを返す。
#[test]
fn rejection_is_invalid_argument_only() {
    for d in rejected_descriptors() {
        assert!(is_invalid(&check(&d)), "{d:?} -> {:?}", check(&d));
    }
    for &role in TextureUsageRole::ALL {
        assert!(is_invalid(&validate_texture_usage_for_role(
            TextureUsage::empty(),
            role
        )));
    }
    assert!(is_invalid(&validate_texture_view_format(
        TextureFormat::Rgba8Unorm,
        &[],
        TextureFormat::Rgba8UnormSrgb
    )));
}
