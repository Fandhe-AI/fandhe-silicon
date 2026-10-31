//! 間接実行で GPU が読む引数の固定長の型と、欄ごとのリトルエンディアンのバイト列との相互変換（TASK-1878 のポインタ）。
//!
//! `execution` から再公開される。型は欄がすべて公開で、unsafe を使わずに `to_bytes`・`from_bytes` で変換する。
//! あわせて、引数バッファの開始位置の整列と範囲の検査（`validate_indirect_args_range`）と、`first_instance` を反映するか拒否するかの判定（`first_instance_decision`）を置く。

use crate::error::ContractError;
use crate::memory::validate_buffer_range;

/// 4 バイトを `u32` に読む（リトルエンディアン）。
const fn read_u32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// 4 バイトを `i32` に読む（リトルエンディアン）。
const fn read_i32(b: &[u8], at: usize) -> i32 {
    i32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// 4 バイトずつ並べた配列へ書き込む（リトルエンディアン）。
fn put(out: &mut [u8], at: usize, v: [u8; 4]) {
    out[at..at + 4].copy_from_slice(&v);
}

/// 間接の描画の引数（4 欄・16 バイト）。
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DrawIndirectArgs {
    /// 描く頂点の数。
    pub vertex_count: u32,
    /// 描くインスタンスの数。
    pub instance_count: u32,
    /// 最初の頂点の番号。
    pub first_vertex: u32,
    /// 最初のインスタンスの番号。
    pub first_instance: u32,
}

impl DrawIndirectArgs {
    /// この型のバイト数。
    pub const SIZE: u64 = 16;

    /// 欄の宣言順に各欄をリトルエンディアンで並べた 16 バイトを返す。
    pub fn to_bytes(&self) -> [u8; 16] {
        let mut out = [0u8; 16];
        put(&mut out, 0, self.vertex_count.to_le_bytes());
        put(&mut out, 4, self.instance_count.to_le_bytes());
        put(&mut out, 8, self.first_vertex.to_le_bytes());
        put(&mut out, 12, self.first_instance.to_le_bytes());
        out
    }

    /// `to_bytes` の逆で、同じ並びから値を組み立てる。
    pub fn from_bytes(bytes: &[u8; 16]) -> Self {
        Self {
            vertex_count: read_u32(bytes, 0),
            instance_count: read_u32(bytes, 4),
            first_vertex: read_u32(bytes, 8),
            first_instance: read_u32(bytes, 12),
        }
    }
}

/// 間接のインデックス付き描画の引数（5 欄・20 バイト）。
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DrawIndexedIndirectArgs {
    /// 描くインデックスの数。
    pub index_count: u32,
    /// 描くインスタンスの数。
    pub instance_count: u32,
    /// 最初のインデックスの番号。
    pub first_index: u32,
    /// 各インデックスに足す頂点の番号。
    pub base_vertex: i32,
    /// 最初のインスタンスの番号。
    pub first_instance: u32,
}

impl DrawIndexedIndirectArgs {
    /// この型のバイト数。
    pub const SIZE: u64 = 20;

    /// 欄の宣言順に各欄をリトルエンディアンで並べた 20 バイトを返す。
    pub fn to_bytes(&self) -> [u8; 20] {
        let mut out = [0u8; 20];
        put(&mut out, 0, self.index_count.to_le_bytes());
        put(&mut out, 4, self.instance_count.to_le_bytes());
        put(&mut out, 8, self.first_index.to_le_bytes());
        put(&mut out, 12, self.base_vertex.to_le_bytes());
        put(&mut out, 16, self.first_instance.to_le_bytes());
        out
    }

    /// `to_bytes` の逆で、同じ並びから値を組み立てる。
    pub fn from_bytes(bytes: &[u8; 20]) -> Self {
        Self {
            index_count: read_u32(bytes, 0),
            instance_count: read_u32(bytes, 4),
            first_index: read_u32(bytes, 8),
            base_vertex: read_i32(bytes, 12),
            first_instance: read_u32(bytes, 16),
        }
    }
}

/// 間接の dispatch の引数（3 欄・12 バイト）。
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DispatchIndirectArgs {
    /// x 方向のグループ数。
    pub x: u32,
    /// y 方向のグループ数。
    pub y: u32,
    /// z 方向のグループ数。
    pub z: u32,
}

impl DispatchIndirectArgs {
    /// この型のバイト数。
    pub const SIZE: u64 = 12;

    /// 欄の宣言順に各欄をリトルエンディアンで並べた 12 バイトを返す。
    pub fn to_bytes(&self) -> [u8; 12] {
        let mut out = [0u8; 12];
        put(&mut out, 0, self.x.to_le_bytes());
        put(&mut out, 4, self.y.to_le_bytes());
        put(&mut out, 8, self.z.to_le_bytes());
        out
    }

    /// `to_bytes` の逆で、同じ並びから値を組み立てる。
    pub fn from_bytes(bytes: &[u8; 12]) -> Self {
        Self {
            x: read_u32(bytes, 0),
            y: read_u32(bytes, 4),
            z: read_u32(bytes, 8),
        }
    }
}

/// 間接実行の引数バッファの開始位置の整列と範囲を確かめる純粋関数。
///
/// 判定の順は、開始位置が 4 の倍数か、終端（`offset + args_size`）が `u64` であふれないか、
/// 終端がバッファの長さを超えないか（ちょうど長さなら通る）。`args_size` は 12・16・20 に限らず任意の値を受ける。
///
/// # Errors
///
/// どの拒否も `ContractError::InvalidArgument` で返す。整列の拒否の文は `indirect args offset` で始まり、
/// あふれと超過の文は `validate_buffer_range` と同じ。
pub fn validate_indirect_args_range(
    offset: u64,
    args_size: u64,
    buffer_len: u64,
) -> Result<(), ContractError> {
    if !offset.is_multiple_of(4) {
        return Err(ContractError::InvalidArgument(format!(
            "indirect args offset ({offset}) is not a multiple of 4"
        )));
    }
    validate_buffer_range(offset, args_size, buffer_len)
}

/// `first_instance` の扱いの判定結果（閉じた 2 値）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FirstInstanceDecision {
    /// 引数の値をそのまま使ってよい。
    Apply,
    /// この組は受け付けない。
    Reject,
}

/// `first_instance` を反映するか拒否するかを返す。
///
/// `first_instance` が 0 なら `Apply`。0 以外のときは、`feature_enabled`（device を作ったときに機能を有効にしたか）が
/// 真なら `Apply`、偽なら `Reject`。
pub fn first_instance_decision(
    feature_enabled: bool,
    first_instance: u32,
) -> FirstInstanceDecision {
    if first_instance == 0 || feature_enabled {
        FirstInstanceDecision::Apply
    } else {
        FirstInstanceDecision::Reject
    }
}
