//! `binding` モジュールの型を外の crate から確かめる公開テスト（TASK-419。BIND-5・BIND-11・BIND-12・BIND-13 のポインタ）。
//!
//! 作成時の検査は無い型なので、渡した値がそのまま読めること・写しを持つこと・派生を確かめる。
//! ビットの値・`Debug` の文字列・`Send`/`Sync` は期待にしない。

use core::num::NonZeroU64;

use fandhe_silicon_core::binding::{
    BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor, BindingDeclaration,
    BindingResource, BindingType, BufferBinding, BufferBindingType, PipelineLayout,
    PipelineLayoutDescriptor, ShaderStageSet,
};
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};

const ALL: [ShaderStageSet; 9] = [
    ShaderStageSet::VERTEX,
    ShaderStageSet::FRAGMENT,
    ShaderStageSet::COMPUTE,
    ShaderStageSet::TASK,
    ShaderStageSet::MESH,
    ShaderStageSet::RAY_GENERATION,
    ShaderStageSet::ANY_HIT,
    ShaderStageSet::CLOSEST_HIT,
    ShaderStageSet::MISS,
];

fn decl(binding: u32) -> BindingDeclaration {
    BindingDeclaration::new(
        binding,
        ShaderStageSet::COMPUTE,
        BindingType::buffer(BufferBindingType::Storage { read_only: false }),
    )
}

#[test]
fn declaration_keeps_fields_and_is_editable() {
    let mut d = BindingDeclaration::new(
        3,
        ShaderStageSet::VERTEX_FRAGMENT,
        BindingType::buffer(BufferBindingType::Uniform),
    );
    assert_eq!(d.binding, 3);
    assert_eq!(d.visibility, ShaderStageSet::VERTEX_FRAGMENT);
    assert_eq!(d.ty, BindingType::buffer(BufferBindingType::Uniform));
    let c = d.clone();
    assert_eq!(c, d);
    d.binding = 4;
    d.visibility = ShaderStageSet::COMPUTE;
    d.ty = BindingType::buffer(BufferBindingType::Storage { read_only: true });
    assert_eq!(d.binding, 4);
    assert_ne!(c, d);
}

#[test]
fn buffer_binding_types_are_distinct_and_matchable() {
    let u = BindingType::buffer(BufferBindingType::Uniform);
    let ro = BindingType::buffer(BufferBindingType::Storage { read_only: true });
    let rw = BindingType::buffer(BufferBindingType::Storage { read_only: false });
    assert_ne!(u, ro);
    assert_ne!(u, rw);
    assert_ne!(ro, rw);
    match &ro {
        BindingType::Buffer { ty, .. } => {
            assert_eq!(*ty, BufferBindingType::Storage { read_only: true });
        }
        _ => panic!("unexpected variant"),
    }
}

#[test]
fn shader_stage_set_values() {
    for (i, a) in ALL.iter().enumerate() {
        assert!(!a.is_empty());
        assert!(a.contains(ShaderStageSet::NONE));
        for (j, b) in ALL.iter().enumerate() {
            assert_eq!(a.contains(*b), i == j);
            let u = *a | *b;
            assert!(u.contains(*a) && u.contains(*b));
        }
    }
    assert!(ShaderStageSet::NONE.is_empty());
    assert!(ShaderStageSet::NONE.contains(ShaderStageSet::NONE));
    assert_eq!(
        ShaderStageSet::VERTEX | ShaderStageSet::FRAGMENT,
        ShaderStageSet::VERTEX_FRAGMENT
    );
    assert!(ShaderStageSet::VERTEX_FRAGMENT.contains(ShaderStageSet::VERTEX));
    assert!(ShaderStageSet::VERTEX_FRAGMENT.contains(ShaderStageSet::FRAGMENT));
    assert!(!ShaderStageSet::VERTEX_FRAGMENT.contains(ShaderStageSet::COMPUTE));
    let mut x = ShaderStageSet::VERTEX;
    x |= ShaderStageSet::FRAGMENT;
    assert_eq!(x, ShaderStageSet::VERTEX | ShaderStageSet::FRAGMENT);
}

#[test]
fn bind_group_layout_copies_declarations() {
    let layout = {
        let decls = vec![decl(0), decl(1)];
        let desc = BindGroupLayoutDescriptor::new(&decls);
        assert_eq!(desc.entries.len(), 2);
        BindGroupLayout::__backend_new(7, &desc)
    };
    assert_eq!(layout.entries(), &[decl(0), decl(1)]);
    assert_eq!(layout.__backend_device_id(), 7);
    assert_eq!(layout.clone(), layout);
    let other =
        BindGroupLayout::__backend_new(8, &BindGroupLayoutDescriptor::new(&[decl(0), decl(1)]));
    assert_ne!(other, layout);
}

#[test]
fn pipeline_layout_copies_layouts_and_keeps_gaps() {
    let pl: PipelineLayout = {
        let a = BindGroupLayout::__backend_new(7, &BindGroupLayoutDescriptor::new(&[decl(0)]));
        let b = BindGroupLayout::__backend_new(7, &BindGroupLayoutDescriptor::new(&[decl(1)]));
        let slots = [Some(&a), None, Some(&b)];
        let desc = PipelineLayoutDescriptor::new(&slots, 16);
        assert_eq!(desc.immediate_size, 16);
        PipelineLayout::__backend_new(7, &desc)
    };
    let got = pl.bind_group_layouts();
    assert_eq!(got.len(), 3);
    assert!(got[1].is_none());
    assert_eq!(got[0].as_ref().map(|l| l.entries()), Some(&[decl(0)][..]));
    assert_eq!(got[2].as_ref().map(|l| l.entries()), Some(&[decl(1)][..]));
    assert_eq!(pl.immediate_size(), 16);
    assert_eq!(pl.__backend_device_id(), 7);
    assert_eq!(pl.clone(), pl);
}

#[test]
fn bind_group_borrows_layout_and_copies_entries() {
    let layout = BindGroupLayout::__backend_new(7, &BindGroupLayoutDescriptor::new(&[decl(0)]));
    let h = BufferHandle::__backend_new(0, 1, 7, 64, BufferUsage::STORAGE);
    let group = {
        let entries = [BindGroupEntry::new(
            0,
            BindingResource::Buffer(BufferBinding::new(&h, 8, NonZeroU64::new(4))),
        )];
        BindGroup::__backend_new(7, &layout, &entries)
    };
    assert!(core::ptr::eq(group.layout(), &layout));
    assert_eq!(group.__backend_device_id(), 7);
    assert_eq!(group.entries().len(), 1);
    assert_eq!(group.entries()[0].binding, 0);
    match &group.entries()[0].resource {
        BindingResource::Buffer(b) => {
            assert!(core::ptr::eq(b.buffer, &h));
            assert_eq!(b.offset, 8);
            assert_eq!(b.size, NonZeroU64::new(4));
        }
        _ => panic!("unexpected variant"),
    }
}

#[test]
fn backend_new_copies_values_without_checking() {
    // 番号の重なり・4 の倍数でない `immediate_size`・資源の範囲を照らさず、そのまま写す
    let layout =
        BindGroupLayout::__backend_new(7, &BindGroupLayoutDescriptor::new(&[decl(2), decl(2)]));
    assert_eq!(layout.entries(), &[decl(2), decl(2)]);
    let pl = PipelineLayout::__backend_new(7, &PipelineLayoutDescriptor::new(&[Some(&layout)], 3));
    assert_eq!(pl.immediate_size(), 3);
    let h = BufferHandle::__backend_new(0, 1, 7, 64, BufferUsage::STORAGE);
    let entries = [
        BindGroupEntry::new(
            9,
            BindingResource::Buffer(BufferBinding::new(&h, 1 << 40, NonZeroU64::new(u64::MAX))),
        ),
        BindGroupEntry::new(9, BindingResource::Buffer(BufferBinding::new(&h, 0, None))),
    ];
    let group = BindGroup::__backend_new(8, &layout, &entries);
    assert_eq!(group.__backend_device_id(), 8);
    assert_eq!(group.entries().len(), 2);
    match &group.entries()[0].resource {
        BindingResource::Buffer(b) => {
            assert_eq!(b.offset, 1 << 40);
            assert_eq!(b.size, NonZeroU64::new(u64::MAX));
        }
        _ => panic!("unexpected variant"),
    }
}
