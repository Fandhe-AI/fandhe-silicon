//! `BufferHandle` の大きさ・用途の問合せ（`size`・`usage`）の最小の往復の試験。
//!
//! 背後実装が `__backend_new` に渡した値をそのまま返すことを確かめる（TASK-310 の 2/6。
//! MEM-44・MEM-45 のポインタ）。7 値の全組み合わせの網羅は別の試験が持つ。

use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};

#[test]
fn size_returns_byte_len_given_to_backend_new() {
    for len in [0u64, 1, 16, u64::MAX] {
        let h = BufferHandle::__backend_new(0, 1, 1, len, BufferUsage::STORAGE);
        assert_eq!(h.size(), len);
    }
}

#[test]
fn usage_returns_usage_given_to_backend_new() {
    let singles = [
        BufferUsage::STORAGE,
        BufferUsage::UPLOAD,
        BufferUsage::DOWNLOAD,
        BufferUsage::VERTEX,
        BufferUsage::INDEX,
        BufferUsage::UNIFORM,
        BufferUsage::EXPORTABLE,
        BufferUsage::STORAGE | BufferUsage::UPLOAD,
        BufferUsage::empty(),
    ];
    for u in singles {
        let h = BufferHandle::__backend_new(0, 1, 1, 8, u);
        assert_eq!(h.usage(), u);
    }
}

#[test]
fn query_does_not_change_backend_fields() {
    let h = BufferHandle::__backend_new(3, 5, 7, 32, BufferUsage::UNIFORM);
    let _ = (h.size(), h.usage());
    assert_eq!(h.__backend_generation(), 3);
    assert_eq!(h.__backend_id(), 5);
    assert_eq!(h.__backend_device_id(), 7);
}

#[test]
fn query_through_shared_reference() {
    let h = BufferHandle::__backend_new(0, 1, 1, 4, BufferUsage::DOWNLOAD);
    let r = &h;
    for _ in 0..3 {
        assert_eq!(r.size(), 4);
        assert_eq!(r.usage(), BufferUsage::DOWNLOAD);
    }
    assert_eq!(h.size(), 4);
}
