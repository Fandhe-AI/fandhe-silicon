//! 範囲の検査で拒否された `upload` がバッファの内容を変えないことを、試験用の最小の実装で確かめる
//! 統合テスト（TASK-309 の 3/4。MEM-70・MEM-74〜MEM-76 のポインタ）。
//!
//! 背後実装の `upload` を模した非公開の型 `MinimalBuffer` が、コピーの前に公開の
//! `validate_buffer_range` を呼ぶ。この型はテストファイルの中だけにあり、公開 API ではない。
//! feature なしで動き、公開のパスだけを使う。

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::memory::validate_buffer_range;

/// バイト列を持つだけの試験用のバッファ。
struct MinimalBuffer {
    bytes: Vec<u8>,
}

impl MinimalBuffer {
    /// 位置から決まる模様で満たす（`fill` との排他的論理和）。
    fn new(allocation_len: usize, fill: u8) -> Self {
        let bytes = (0..allocation_len)
            .map(|i| u8::try_from(i % 251).unwrap_or(0) ^ fill)
            .collect();
        Self { bytes }
    }

    /// 検査を通ったときだけ `[dst_offset, dst_offset + data.len())` を書き換える。
    fn upload(&mut self, dst_offset: u64, data: &[u8]) -> Result<(), ContractError> {
        let len = u64::try_from(data.len())
            .map_err(|_| ContractError::InvalidArgument("data length does not fit u64".into()))?;
        let alloc = u64::try_from(self.bytes.len())
            .map_err(|_| ContractError::InvalidArgument("allocation does not fit u64".into()))?;
        validate_buffer_range(dst_offset, len, alloc)?;
        let start = usize::try_from(dst_offset)
            .map_err(|_| ContractError::InvalidArgument("offset does not fit usize".into()))?;
        self.bytes[start..start + data.len()].copy_from_slice(data);
        Ok(())
    }

    fn contents(&self) -> &[u8] {
        &self.bytes
    }
}

fn is_overflow(r: &Result<(), ContractError>) -> bool {
    matches!(r, Err(ContractError::InvalidArgument(m)) if m.starts_with("buffer range end overflows u64"))
}

fn is_exceed(r: &Result<(), ContractError>) -> bool {
    matches!(r, Err(ContractError::InvalidArgument(m))
        if m.starts_with("buffer range end (") && m.contains("exceeds allocation length"))
}

/// 拒否されること（枝つき）と、内容が全バイト不変であることを確かめる。
fn assert_rejected_unchanged(alloc: usize, offset: u64, len: usize, overflow: bool) {
    let mut buf = MinimalBuffer::new(alloc, 0x5A);
    let before = buf.contents().to_vec();
    let data = vec![0xFF; len];
    let r = buf.upload(offset, &data);
    let ctx = format!("(allocation {alloc}, offset {offset}, len {len})");
    if overflow {
        assert!(is_overflow(&r), "expected overflow {ctx}: {r:?}");
    } else {
        assert!(is_exceed(&r), "expected exceed {ctx}: {r:?}");
    }
    assert_eq!(buf.contents(), before.as_slice(), "contents changed {ctx}");
}

#[test]
fn upload_reject_keeps_contents_partial_overlap() {
    assert_rejected_unchanged(16, 8, 16, false);
}

#[test]
fn upload_reject_keeps_contents_end_one_past() {
    for alloc in [0usize, 1, 16] {
        assert_rejected_unchanged(alloc, 0, alloc + 1, false);
    }
    assert_rejected_unchanged(16, 1, 16, false);
}

#[test]
fn upload_reject_keeps_contents_offset_past_end_zero_len() {
    assert_rejected_unchanged(16, 17, 0, false);
}

#[test]
fn upload_reject_keeps_contents_offset_max() {
    assert_rejected_unchanged(16, u64::MAX, 1, true);
    assert_rejected_unchanged(16, u64::MAX, 0, false);
}

#[test]
fn upload_reject_keeps_contents_zero_length_buffer() {
    assert_rejected_unchanged(0, 0, 1, false);
}

#[test]
fn upload_reject_keeps_contents_grid_matches_u128_reference() {
    for alloc in [0usize, 1, 16] {
        let a = u64::try_from(alloc).expect("small");
        let mut offsets = vec![0, 1, a, a + 1, u64::MAX];
        if a >= 1 {
            offsets.push(a - 1);
        }
        for offset in offsets {
            for len in [0usize, 1, alloc, alloc + 1] {
                let end = u128::from(offset) + u128::try_from(len).expect("small");
                let ctx = format!("(allocation {alloc}, offset {offset}, len {len})");
                if end > u128::from(u64::MAX) {
                    assert_rejected_unchanged(alloc, offset, len, true);
                } else if end > u128::from(a) {
                    assert_rejected_unchanged(alloc, offset, len, false);
                } else {
                    let mut buf = MinimalBuffer::new(alloc, 0x5A);
                    assert!(buf.upload(offset, &vec![0xFF; len]).is_ok(), "{ctx}");
                }
            }
        }
    }
}

#[test]
fn upload_accept_writes_only_range() {
    // (offset, len): 中ほど・終端ちょうど・先頭・長さ 0・終端位置の長さ 0
    for (offset, len) in [(4usize, 5usize), (11, 5), (0, 3), (7, 0), (16, 0)] {
        let mut buf = MinimalBuffer::new(16, 0x5A);
        let before = buf.contents().to_vec();
        let off = u64::try_from(offset).expect("small");
        let r = buf.upload(off, &vec![0xFF; len]);
        assert!(r.is_ok(), "(offset {offset}, len {len}): {r:?}");
        for (i, (&now, &was)) in buf.contents().iter().zip(before.iter()).enumerate() {
            if (offset..offset + len).contains(&i) {
                assert_eq!(now, 0xFF, "inside range at {i}");
            } else {
                assert_eq!(now, was, "outside range at {i}");
            }
        }
    }
}

#[test]
fn upload_reject_keeps_contents_after_accepted_upload() {
    let mut buf = MinimalBuffer::new(16, 0x5A);
    buf.upload(0, &[0x11; 4]).expect("in-range upload");
    let after_ok = buf.contents().to_vec();
    let r = buf.upload(12, &[0xFF; 8]);
    assert!(is_exceed(&r), "{r:?}");
    assert_eq!(buf.contents(), after_ok.as_slice());
}
