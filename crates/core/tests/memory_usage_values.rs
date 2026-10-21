//! 用途の 7 値の公開・ビット値・組み合わせ・`allocate` の受け付けを、別の crate から確かめる（TASK-308 の 3/6。MEM-3 のポインタ）。
//!
//! 試験用の最小の実装 `RecordingOps` はこのファイルの中だけにあり、公開 API ではない。
//! feature なしで、公開のパスだけを使う。

use std::cell::RefCell;

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage, MemoryOps};

/// 名前・定数・期待するビット値の表。`const` の文脈で定数が使えることも同時に確かめる。
const USAGES: [(&str, BufferUsage, u32); 7] = [
    ("STORAGE", BufferUsage::STORAGE, 1 << 0),
    ("UPLOAD", BufferUsage::UPLOAD, 1 << 1),
    ("DOWNLOAD", BufferUsage::DOWNLOAD, 1 << 2),
    ("VERTEX", BufferUsage::VERTEX, 1 << 3),
    ("INDEX", BufferUsage::INDEX, 1 << 4),
    ("UNIFORM", BufferUsage::UNIFORM, 1 << 5),
    ("EXPORTABLE", BufferUsage::EXPORTABLE, 1 << 6),
];

/// `bits()` が期待値と違う名前を返す。陽性対照で、照合が見落とさないことを示すのに使う。
fn mismatched<'a>(table: &[(&'a str, BufferUsage, u32)]) -> Vec<&'a str> {
    table
        .iter()
        .filter(|(_, usage, expected)| usage.bits() != *expected)
        .map(|(name, _, _)| *name)
        .collect()
}

/// 受け取った `usage` を記録するだけの、試験用の最小の `MemoryOps`。
#[derive(Default)]
struct RecordingOps {
    seen: RefCell<Vec<BufferUsage>>,
}

impl MemoryOps for RecordingOps {
    fn allocate(&self, byte_len: u64, usage: BufferUsage) -> Result<BufferHandle, ContractError> {
        let mut seen = self.seen.borrow_mut();
        seen.push(usage);
        let id = u64::try_from(seen.len()).expect("count fits u64");
        Ok(BufferHandle::__backend_new(0, id, 1, byte_len, usage))
    }

    fn upload(&self, _dst: &BufferHandle, _off: u64, _bytes: &[u8]) -> Result<(), ContractError> {
        Ok(())
    }

    #[cfg(feature = "native-blocking-wait")]
    fn download_blocking(
        &self,
        _src: &BufferHandle,
        _off: u64,
        _len: u64,
    ) -> Result<Vec<u8>, ContractError> {
        Ok(Vec::new())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn download_async(
        &self,
        _src: &BufferHandle,
        _off: u64,
        _len: u64,
        callback: Box<dyn FnOnce(Result<Vec<u8>, ContractError>) + Send>,
    ) {
        callback(Ok(Vec::new()));
    }

    #[cfg(target_arch = "wasm32")]
    fn download_async(
        &self,
        _src: &BufferHandle,
        _off: u64,
        _len: u64,
        callback: Box<dyn FnOnce(Result<Vec<u8>, ContractError>)>,
    ) {
        callback(Ok(Vec::new()));
    }

    fn free(&self, _buffer: BufferHandle) {}
}

/// `mask` のビットが立つ定数を `|` で合わせる。
fn combine(mask: u32) -> BufferUsage {
    USAGES
        .iter()
        .filter(|(_, _, bit)| mask & bit != 0)
        .fold(BufferUsage::empty(), |acc, (_, u, _)| acc | *u)
}

/// 7 定数それぞれの `bits()` が `1<<0` から `1<<6` と等しい（失敗時に定数名が分かるよう 1 本ずつ並べる）。
#[test]
fn usage_constants_have_defined_bits() {
    assert_eq!(BufferUsage::STORAGE.bits(), 1 << 0, "STORAGE");
    assert_eq!(BufferUsage::UPLOAD.bits(), 1 << 1, "UPLOAD");
    assert_eq!(BufferUsage::DOWNLOAD.bits(), 1 << 2, "DOWNLOAD");
    assert_eq!(BufferUsage::VERTEX.bits(), 1 << 3, "VERTEX");
    assert_eq!(BufferUsage::INDEX.bits(), 1 << 4, "INDEX");
    assert_eq!(BufferUsage::UNIFORM.bits(), 1 << 5, "UNIFORM");
    assert_eq!(BufferUsage::EXPORTABLE.bits(), 1 << 6, "EXPORTABLE");
    assert!(mismatched(&USAGES).is_empty());
}

/// 各値は 1 ビットだけで、互いに重ならず、和は 0x7F。
#[test]
fn usage_constants_are_single_disjoint_bits() {
    let mut sum = 0u32;
    for (i, (name, a, _)) in USAGES.iter().enumerate() {
        assert_eq!(a.bits().count_ones(), 1, "{name}");
        for (other, b, _) in USAGES.iter().skip(i + 1) {
            assert_eq!(a.bits() & b.bits(), 0, "{name} vs {other}");
        }
        sum |= a.bits();
    }
    assert_eq!(sum, 0x7F);
    assert_eq!(sum & !0x7F, 0);
}

/// 空でない 127 通りで、`bits` と `contains` が合う。
#[test]
fn usage_all_combinations_round_trip() {
    for mask in 1u32..128 {
        let usage = combine(mask);
        assert_eq!(usage.bits(), mask, "mask {mask:#x}");
        for (name, single, bit) in &USAGES {
            assert_eq!(
                usage.contains(*single),
                mask & bit != 0,
                "{name} in {mask:#x}"
            );
        }
    }
}

/// `&dyn MemoryOps` 越しに 127 通りすべてで `allocate` が `Ok` を返し、受け取った `usage` が渡した値と等しい。
#[test]
fn allocate_accepts_every_combination_through_dyn() {
    let ops = RecordingOps::default();
    let dyn_ops: &dyn MemoryOps = &ops;
    for mask in 1u32..128 {
        let handle = dyn_ops.allocate(16, combine(mask)).expect("allocate");
        dyn_ops.free(handle);
    }
    let seen = ops.seen.borrow();
    assert_eq!(seen.len(), 127);
    for (i, usage) in seen.iter().enumerate() {
        let mask = u32::try_from(i).expect("small") + 1;
        assert_eq!(usage.bits(), mask);
    }
}

/// 7 値を 1 つずつ渡しても `Ok` が返り、受け取った値が渡した定数と等しい。
#[test]
fn allocate_accepts_each_single_usage() {
    let ops = RecordingOps::default();
    let dyn_ops: &dyn MemoryOps = &ops;
    for (name, usage, _) in &USAGES {
        let handle = dyn_ops
            .allocate(4, *usage)
            .unwrap_or_else(|_| panic!("{name}"));
        dyn_ops.free(handle);
    }
    let seen = ops.seen.borrow();
    assert_eq!(seen.len(), USAGES.len());
    for ((_, usage, _), got) in USAGES.iter().zip(seen.iter()) {
        assert_eq!(usage, got);
    }
}

/// 陽性対照: 期待をずらした表を照合が検出する。
#[test]
fn mismatch_check_detects_wrong_expectation() {
    assert!(mismatched(&USAGES).is_empty());
    let mut wrong = USAGES;
    wrong[5].2 = 1 << 6;
    assert_eq!(mismatched(&wrong), vec!["UNIFORM"]);
}
