//! 用途と操作の対応の検査を、別の crate から全入力の網羅で確かめる（TASK-1796。MEM-109 のポインタ）。
//!
//! 公開のパスだけを使う。13 個の用途の定数を OR した 8192 通りの申告 × 全役割を回し、
//! 期待は定数で書き下した「役割 → 要る用途」の表と集合の包含で求める。

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::memory::{BufferUsage, UsageRole, validate_usage_for_role};

const ALL_USAGES: [BufferUsage; 13] = [
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

/// 役割ごとに要る用途（実装の表とは別に書き下す）。
const REQUIRED: [(UsageRole, BufferUsage); 14] = [
    (UsageRole::MapRead, BufferUsage::MAP_READ),
    (UsageRole::MapWrite, BufferUsage::MAP_WRITE),
    (UsageRole::CopyBufferSource, BufferUsage::DOWNLOAD),
    (UsageRole::CopyBufferToTextureSource, BufferUsage::DOWNLOAD),
    (UsageRole::CopyBufferDestination, BufferUsage::UPLOAD),
    (UsageRole::IndexBuffer, BufferUsage::INDEX),
    (UsageRole::VertexBuffer, BufferUsage::VERTEX),
    (UsageRole::UniformBinding, BufferUsage::UNIFORM),
    (UsageRole::StorageBinding, BufferUsage::STORAGE),
    (UsageRole::IndirectArguments, BufferUsage::INDIRECT),
    (
        UsageRole::QueryResolveDestination,
        BufferUsage::QUERY_RESOLVE,
    ),
    (UsageRole::BlasBuildInput, BufferUsage::BLAS_INPUT),
    (UsageRole::TlasBuildInput, BufferUsage::TLAS_INPUT),
    (UsageRole::Export, BufferUsage::EXPORTABLE),
];

fn combo(mask: u32) -> BufferUsage {
    let mut u = BufferUsage::empty();
    for (i, v) in ALL_USAGES.iter().enumerate() {
        if mask & (1 << i) != 0 {
            u |= *v;
        }
    }
    u
}

/// 実装の表が期待の表と同じ役割を同じ順で持つ。
#[test]
fn usage_check_role_table_matches_expected() {
    assert_eq!(UsageRole::ALL.len(), REQUIRED.len());
    for (i, (role, usage)) in REQUIRED.iter().enumerate() {
        assert_eq!(UsageRole::ALL[i], *role);
        assert_eq!(role.required_usage(), *usage, "{role:?}");
    }
}

/// 8192 通りの申告 × 14 役割で、通るのは要る用途を含むときだけ。
#[test]
fn usage_check_exhaustive_pass_iff_contains() {
    let mut passed = 0usize;
    let mut rejected = 0usize;
    for mask in 0..(1u32 << ALL_USAGES.len()) {
        let declared = combo(mask);
        for (role, required) in REQUIRED {
            let expect_ok = declared.bits() & required.bits() == required.bits();
            let got = validate_usage_for_role(declared, role);
            match (&got, expect_ok) {
                (Ok(()), true) => passed += 1,
                (Err(ContractError::InvalidArgument(_)), false) => rejected += 1,
                _ => panic!("mask {mask:#x} {role:?}: {got:?}, expected ok={expect_ok}"),
            }
        }
    }
    assert_eq!(passed + rejected, 8192 * 14);
    assert_eq!(passed, 4096 * 14);
    assert_eq!(rejected, 4096 * 14);
}

#[test]
fn usage_check_empty_usage_is_rejected_for_every_role() {
    for role in UsageRole::ALL {
        assert!(
            matches!(
                validate_usage_for_role(BufferUsage::empty(), role),
                Err(ContractError::InvalidArgument(_))
            ),
            "{role:?}"
        );
    }
}

#[test]
fn usage_check_over_declaration_passes() {
    let everything = combo((1 << ALL_USAGES.len()) - 1);
    for role in UsageRole::ALL {
        assert!(
            validate_usage_for_role(everything, role).is_ok(),
            "{role:?}"
        );
    }
}

/// 要る用途だけを申告したときは、その役割だけが通る（ほかの役割は、要る用途が同じ組を除いて拒否）。
#[test]
fn usage_check_single_usage_passes_only_matching_roles() {
    for (declared_role, declared) in REQUIRED {
        for (role, required) in REQUIRED {
            let r = validate_usage_for_role(declared, role);
            assert_eq!(
                r.is_ok(),
                required == declared,
                "{declared_role:?} -> {role:?}"
            );
        }
    }
}

/// 拒否の文字列に役割の名前と足りない用途の値の名前が入る。
#[test]
fn usage_check_message_names_role_and_missing_usage() {
    let names = [
        "MAP_READ",
        "MAP_WRITE",
        "DOWNLOAD",
        "DOWNLOAD",
        "UPLOAD",
        "INDEX",
        "VERTEX",
        "UNIFORM",
        "STORAGE",
        "INDIRECT",
        "QUERY_RESOLVE",
        "BLAS_INPUT",
        "TLAS_INPUT",
        "EXPORTABLE",
    ];
    let mut role_names = Vec::new();
    for (i, role) in UsageRole::ALL.iter().enumerate() {
        let Err(ContractError::InvalidArgument(msg)) =
            validate_usage_for_role(BufferUsage::empty(), *role)
        else {
            panic!("{role:?} must be rejected");
        };
        assert!(msg.contains(names[i]), "{msg}");
        role_names.push(msg);
    }
    role_names.sort();
    role_names.dedup();
    assert_eq!(role_names.len(), 14, "messages must differ per role");
}

/// 頂点バッファは `VERTEX` だけを要り、`STORAGE` は要らない。
#[test]
fn usage_check_vertex_buffer_does_not_require_storage() {
    assert!(validate_usage_for_role(BufferUsage::VERTEX, UsageRole::VertexBuffer).is_ok());
    assert!(validate_usage_for_role(BufferUsage::STORAGE, UsageRole::VertexBuffer).is_err());
}

/// 検査を通ったときだけ書く試験用の最小のバッファ。拒否の前後で内容が全バイト一致する。
struct MinimalBuffer {
    usage: BufferUsage,
    bytes: Vec<u8>,
}

impl MinimalBuffer {
    fn write_as(&mut self, role: UsageRole, value: u8) -> Result<(), ContractError> {
        validate_usage_for_role(self.usage, role)?;
        self.bytes.fill(value);
        Ok(())
    }
}

#[test]
fn usage_check_reject_keeps_contents() {
    for mask in 0..(1u32 << ALL_USAGES.len()) {
        let usage = combo(mask);
        for (role, required) in REQUIRED {
            let mut buf = MinimalBuffer {
                usage,
                bytes: vec![0xAB; 16],
            };
            let before = buf.bytes.clone();
            let r = buf.write_as(role, 0x11);
            if usage.contains(required) {
                assert!(r.is_ok());
                assert_eq!(
                    buf.bytes,
                    vec![0x11; 16],
                    "positive control: accepted writes"
                );
            } else {
                assert!(matches!(r, Err(ContractError::InvalidArgument(_))));
                assert_eq!(buf.bytes, before, "mask {mask:#x} {role:?}");
            }
        }
    }
}
