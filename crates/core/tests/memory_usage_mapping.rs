//! 上の層の用途の 12 値と契約の `BufferUsage` の対応表を、別の crate から確かめる（TASK-1796。MEM-96 のポインタ）。
//!
//! 公開のパスだけを使う。期待は定数で書き下し、実装の対応表を写さない。

use fandhe_silicon_core::memory::{BufferUsage, BufferUsageKind};

/// 期待する対応（種別、契約の値、ビット、ホストの経路）。ビットは値の定義とは別に書く。
const EXPECTED: [(BufferUsageKind, BufferUsage, u32, bool); 12] = [
    (
        BufferUsageKind::MapRead,
        BufferUsage::MAP_READ,
        1 << 7,
        true,
    ),
    (
        BufferUsageKind::MapWrite,
        BufferUsage::MAP_WRITE,
        1 << 8,
        true,
    ),
    (
        BufferUsageKind::CopySrc,
        BufferUsage::DOWNLOAD,
        1 << 2,
        true,
    ),
    (BufferUsageKind::CopyDst, BufferUsage::UPLOAD, 1 << 1, true),
    (BufferUsageKind::Index, BufferUsage::INDEX, 1 << 4, false),
    (BufferUsageKind::Vertex, BufferUsage::VERTEX, 1 << 3, false),
    (
        BufferUsageKind::Uniform,
        BufferUsage::UNIFORM,
        1 << 5,
        false,
    ),
    (
        BufferUsageKind::Storage,
        BufferUsage::STORAGE,
        1 << 0,
        false,
    ),
    (
        BufferUsageKind::Indirect,
        BufferUsage::INDIRECT,
        1 << 9,
        false,
    ),
    (
        BufferUsageKind::QueryResolve,
        BufferUsage::QUERY_RESOLVE,
        1 << 10,
        false,
    ),
    (
        BufferUsageKind::BlasInput,
        BufferUsage::BLAS_INPUT,
        1 << 11,
        false,
    ),
    (
        BufferUsageKind::TlasInput,
        BufferUsage::TLAS_INPUT,
        1 << 12,
        false,
    ),
];

#[test]
fn usage_mapping_twelve_rows_match_expected() {
    assert_eq!(BufferUsageKind::ALL.len(), 12);
    for (i, (kind, usage, bits, host)) in EXPECTED.iter().enumerate() {
        assert_eq!(BufferUsageKind::ALL[i], *kind, "row {i} order");
        assert_eq!(kind.contract_usage(), *usage, "{kind:?}");
        assert_eq!(kind.contract_usage().bits(), *bits, "{kind:?} bits");
        assert_eq!(kind.has_host_path(), *host, "{kind:?} host path");
    }
}

#[test]
fn usage_mapping_existing_seven_bits_are_unchanged() {
    let existing = [
        (BufferUsage::STORAGE, 1 << 0),
        (BufferUsage::UPLOAD, 1 << 1),
        (BufferUsage::DOWNLOAD, 1 << 2),
        (BufferUsage::VERTEX, 1 << 3),
        (BufferUsage::INDEX, 1 << 4),
        (BufferUsage::UNIFORM, 1 << 5),
        (BufferUsage::EXPORTABLE, 1 << 6),
    ];
    for (usage, bits) in existing {
        assert_eq!(usage.bits(), bits);
    }
}

#[test]
fn usage_mapping_thirteen_contract_values_are_disjoint() {
    let all = [
        BufferUsage::STORAGE,
        BufferUsage::UPLOAD,
        BufferUsage::DOWNLOAD,
        BufferUsage::VERTEX,
        BufferUsage::INDEX,
        BufferUsage::UNIFORM,
        BufferUsage::EXPORTABLE,
        BufferUsage::MAP_READ,
        BufferUsage::MAP_WRITE,
        BufferUsage::INDIRECT,
        BufferUsage::QUERY_RESOLVE,
        BufferUsage::BLAS_INPUT,
        BufferUsage::TLAS_INPUT,
    ];
    let mut sum = 0u32;
    for u in all {
        assert_eq!(u.bits().count_ones(), 1);
        assert_eq!(sum & u.bits(), 0);
        sum |= u.bits();
    }
    assert_eq!(sum, 0x1FFF);
}

#[test]
fn usage_mapping_exportable_is_not_an_output_of_the_table() {
    for kind in BufferUsageKind::ALL.iter().copied() {
        assert_ne!(kind.contract_usage(), BufferUsage::EXPORTABLE, "{kind:?}");
        assert!(!kind.contract_usage().is_empty(), "{kind:?}");
    }
}

#[test]
fn usage_mapping_is_injective() {
    for a in BufferUsageKind::ALL.iter().copied() {
        for b in BufferUsageKind::ALL.iter().copied() {
            assert_eq!(a == b, a.contract_usage() == b.contract_usage());
        }
    }
}
