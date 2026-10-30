//! `binding` モジュールの `#[non_exhaustive]` な列挙 3 型を、別の crate から固定する公開テスト（TASK-419）。
//!
//! 仕組みは `error_non_exhaustive.rs` と同じ。`#![deny(unreachable_patterns)]` のもとで
//! 全変種を並べた後に `_` の腕を置く。属性が外れると `_` が到達不能になって落ちる。

#![deny(unreachable_patterns)]

use fandhe_silicon_core::binding::{BindingResource, BindingType, BufferBindingType};

#[test]
fn binding_type_stays_non_exhaustive() {
    fn label(v: &BindingType) -> &'static str {
        match v {
            BindingType::Buffer { .. } => "Buffer",
            _ => "other",
        }
    }
    assert_eq!(
        label(&BindingType::buffer(BufferBindingType::Uniform)),
        "Buffer"
    );
}

#[test]
fn buffer_binding_type_stays_non_exhaustive() {
    fn label(v: &BufferBindingType) -> &'static str {
        match v {
            BufferBindingType::Uniform => "Uniform",
            BufferBindingType::Storage { .. } => "Storage",
            _ => "other",
        }
    }
    assert_eq!(label(&BufferBindingType::Uniform), "Uniform");
}

#[test]
fn binding_resource_stays_non_exhaustive() {
    fn label(v: &BindingResource<'_>) -> &'static str {
        match v {
            BindingResource::Buffer(_) => "Buffer",
            _ => "other",
        }
    }
    let h = fandhe_silicon_core::memory::BufferHandle::__backend_new(
        0,
        1,
        7,
        64,
        fandhe_silicon_core::memory::BufferUsage::STORAGE,
    );
    let r = BindingResource::Buffer(fandhe_silicon_core::binding::BufferBinding::new(
        &h, 0, None,
    ));
    assert_eq!(label(&r), "Buffer");
}
