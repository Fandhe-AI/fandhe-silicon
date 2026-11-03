//! サンプラーの値の型・記述子・handle を、別の crate から公開のパスだけで確かめる（TASK-373 のポインタ）。
//!
//! feature なしで流れる。

use std::collections::HashSet;

use fandhe_silicon_core::sampler::{
    AddressMode, FilterMode, MipmapFilterMode, SamplerBorderColor, SamplerDescriptor, SamplerHandle,
};

fn address_name(m: AddressMode) -> &'static str {
    match m {
        AddressMode::ClampToEdge => "clamp-to-edge",
        AddressMode::Repeat => "repeat",
        AddressMode::MirrorRepeat => "mirror-repeat",
        AddressMode::ClampToBorder => "clamp-to-border",
        _ => "other",
    }
}

fn filter_name(m: FilterMode) -> &'static str {
    match m {
        FilterMode::Nearest => "nearest",
        FilterMode::Linear => "linear",
        _ => "other",
    }
}

fn mipmap_name(m: MipmapFilterMode) -> &'static str {
    match m {
        MipmapFilterMode::Nearest => "nearest",
        MipmapFilterMode::Linear => "linear",
        _ => "other",
    }
}

fn border_name(m: SamplerBorderColor) -> &'static str {
    match m {
        SamplerBorderColor::TransparentBlack => "transparent-black",
        SamplerBorderColor::OpaqueBlack => "opaque-black",
        SamplerBorderColor::OpaqueWhite => "opaque-white",
        SamplerBorderColor::Zero => "zero",
        _ => "other",
    }
}

const ALL_ADDRESS: [AddressMode; 4] = [
    AddressMode::ClampToEdge,
    AddressMode::Repeat,
    AddressMode::MirrorRepeat,
    AddressMode::ClampToBorder,
];
const ALL_FILTER: [FilterMode; 2] = [FilterMode::Nearest, FilterMode::Linear];
const ALL_MIPMAP: [MipmapFilterMode; 2] = [MipmapFilterMode::Nearest, MipmapFilterMode::Linear];
const ALL_BORDER: [SamplerBorderColor; 4] = [
    SamplerBorderColor::TransparentBlack,
    SamplerBorderColor::OpaqueBlack,
    SamplerBorderColor::OpaqueWhite,
    SamplerBorderColor::Zero,
];

#[test]
fn descriptor_default_values() {
    // TEX-39
    let d = SamplerDescriptor::default();
    assert_eq!(d.label, None);
    assert_eq!(d.address_mode_u, AddressMode::ClampToEdge);
    assert_eq!(d.address_mode_v, AddressMode::ClampToEdge);
    assert_eq!(d.address_mode_w, AddressMode::ClampToEdge);
    assert_eq!(d.mag_filter, FilterMode::Nearest);
    assert_eq!(d.min_filter, FilterMode::Nearest);
    assert_eq!(d.mipmap_filter, MipmapFilterMode::Nearest);
    assert_eq!(d.lod_min_clamp, 0.0);
    assert_eq!(d.lod_max_clamp, 32.0);
    assert_eq!(d.anisotropy_clamp, 1);
    assert_eq!(d.border_color, None);
}

#[test]
fn descriptor_fields_hold_written_values() {
    // TEX-39
    let mut d = SamplerDescriptor::default();
    d.label = Some("sampler");
    d.address_mode_u = AddressMode::Repeat;
    d.address_mode_v = AddressMode::MirrorRepeat;
    d.address_mode_w = AddressMode::ClampToBorder;
    d.mag_filter = FilterMode::Linear;
    d.min_filter = FilterMode::Linear;
    d.mipmap_filter = MipmapFilterMode::Linear;
    d.lod_min_clamp = 1.5;
    d.lod_max_clamp = 8.0;
    d.anisotropy_clamp = 16;
    d.border_color = Some(SamplerBorderColor::OpaqueWhite);
    assert_eq!(d.label, Some("sampler"));
    assert_eq!(d.address_mode_u, AddressMode::Repeat);
    assert_eq!(d.address_mode_v, AddressMode::MirrorRepeat);
    assert_eq!(d.address_mode_w, AddressMode::ClampToBorder);
    assert_eq!(d.mag_filter, FilterMode::Linear);
    assert_eq!(d.min_filter, FilterMode::Linear);
    assert_eq!(d.mipmap_filter, MipmapFilterMode::Linear);
    assert_eq!(d.lod_min_clamp, 1.5);
    assert_eq!(d.lod_max_clamp, 8.0);
    assert_eq!(d.anisotropy_clamp, 16);
    assert_eq!(d.border_color, Some(SamplerBorderColor::OpaqueWhite));
}

#[test]
fn descriptor_holds_values_unchecked() {
    let mut d = SamplerDescriptor::default();
    d.lod_min_clamp = 5.0;
    d.lod_max_clamp = 1.0;
    assert_eq!(d.lod_min_clamp, 5.0);
    assert_eq!(d.lod_max_clamp, 1.0);

    d.anisotropy_clamp = 0;
    assert_eq!(d.anisotropy_clamp, 0);
    d.anisotropy_clamp = u16::MAX;
    assert_eq!(d.anisotropy_clamp, u16::MAX);

    d.lod_min_clamp = f32::NAN;
    assert!(d.lod_min_clamp.is_nan());

    let mut e = SamplerDescriptor::default();
    e.address_mode_u = AddressMode::ClampToBorder;
    e.border_color = None;
    assert_eq!(e.address_mode_u, AddressMode::ClampToBorder);
    assert_eq!(e.border_color, None);
}

#[test]
fn descriptor_clone_and_eq() {
    let base = SamplerDescriptor::default();
    assert_eq!(base.clone(), base);

    let mut a = SamplerDescriptor::default();
    a.label = Some("x");
    assert_eq!(a.clone(), a);
    assert_ne!(a, base);

    let mut b = SamplerDescriptor::default();
    b.address_mode_w = AddressMode::Repeat;
    assert_ne!(b, base);

    let mut c = SamplerDescriptor::default();
    c.mipmap_filter = MipmapFilterMode::Linear;
    assert_ne!(c, base);

    let mut d = SamplerDescriptor::default();
    d.anisotropy_clamp = 2;
    assert_ne!(d, base);

    let mut e = SamplerDescriptor::default();
    e.border_color = Some(SamplerBorderColor::Zero);
    assert_ne!(e, base);
}

#[test]
fn value_defaults() {
    assert_eq!(AddressMode::default(), AddressMode::ClampToEdge);
    assert_eq!(FilterMode::default(), FilterMode::Nearest);
    assert_eq!(MipmapFilterMode::default(), MipmapFilterMode::Nearest);
}

#[test]
fn value_enums_are_distinct() {
    assert_eq!(ALL_ADDRESS.iter().collect::<HashSet<_>>().len(), 4);
    assert_eq!(ALL_FILTER.iter().collect::<HashSet<_>>().len(), 2);
    assert_eq!(ALL_MIPMAP.iter().collect::<HashSet<_>>().len(), 2);
    assert_eq!(ALL_BORDER.iter().collect::<HashSet<_>>().len(), 4);
}

#[test]
fn value_enums_match_with_wildcard() {
    let a: Vec<_> = ALL_ADDRESS.iter().map(|m| address_name(*m)).collect();
    assert_eq!(
        a,
        [
            "clamp-to-edge",
            "repeat",
            "mirror-repeat",
            "clamp-to-border"
        ]
    );
    let f: Vec<_> = ALL_FILTER.iter().map(|m| filter_name(*m)).collect();
    assert_eq!(f, ["nearest", "linear"]);
    let m: Vec<_> = ALL_MIPMAP.iter().map(|m| mipmap_name(*m)).collect();
    assert_eq!(m, ["nearest", "linear"]);
    let b: Vec<_> = ALL_BORDER.iter().map(|m| border_name(*m)).collect();
    assert_eq!(
        b,
        ["transparent-black", "opaque-black", "opaque-white", "zero"]
    );
}

#[test]
fn handle_round_trip() {
    // TEX-39
    for (g, i, dev) in [
        (0, 0, 0),
        (1, 2, 3),
        (u64::MAX, 0, 1),
        (0, u64::MAX, 1),
        (u64::MAX, u64::MAX, u64::MAX),
    ] {
        let h = SamplerHandle::__backend_new(g, i, dev);
        assert_eq!(h.__backend_generation(), g);
        assert_eq!(h.__backend_id(), i);
        assert_eq!(h.__backend_device_id(), dev);
    }
}

type Change = fn(&mut SamplerDescriptor<'static>);

#[test]
fn descriptor_each_field_changes_equality() {
    // TEX-39
    let base = SamplerDescriptor::default();
    let changes: [(&str, Change); 11] = [
        ("label", |d| d.label = Some("x")),
        ("address_mode_u", |d| d.address_mode_u = AddressMode::Repeat),
        ("address_mode_v", |d| d.address_mode_v = AddressMode::Repeat),
        ("address_mode_w", |d| d.address_mode_w = AddressMode::Repeat),
        ("mag_filter", |d| d.mag_filter = FilterMode::Linear),
        ("min_filter", |d| d.min_filter = FilterMode::Linear),
        ("mipmap_filter", |d| {
            d.mipmap_filter = MipmapFilterMode::Linear
        }),
        ("lod_min_clamp", |d| d.lod_min_clamp = 1.0),
        ("lod_max_clamp", |d| d.lod_max_clamp = 16.0),
        ("anisotropy_clamp", |d| d.anisotropy_clamp = 2),
        ("border_color", |d| {
            d.border_color = Some(SamplerBorderColor::Zero)
        }),
    ];
    for (name, change) in changes {
        let mut d = SamplerDescriptor::default();
        change(&mut d);
        assert_ne!(d, base, "{name}");
    }
}

#[test]
fn descriptor_and_handle_debug_are_not_empty() {
    // TEX-39
    assert!(!format!("{:?}", SamplerDescriptor::default()).is_empty());
    assert!(!format!("{:?}", SamplerHandle::__backend_new(1, 2, 3)).is_empty());
}
