//! 上の層の用途の 12 値と契約の `BufferUsage` の対応表（TASK-1796 の 1 つ目の部品。MEM-96 のポインタ）。
//!
//! `memory` モジュールの一部。上の層（`fandhe-silicon-upper`）が利用者の用途の指定を `allocate` の
//! `usage` へ写すときに引く表で、背後実装の API は呼ばず、確保もしない。
//! 写しは名前（`BufferUsageKind` の variant）で行い、ビットの値では行わない。
//! 背後実装ごとの提供の有無は能力の問合せの持ち分で、この表には載せない。

use super::buffer::BufferUsage;

/// 上の層の用途の 12 値。`BufferUsage` への写しは [`BufferUsageKind::contract_usage`]。
///
/// variant の順は表の行の順で、[`BufferUsageKind::ALL`] の並びと一致する。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BufferUsageKind {
    /// 読みの対応付け。
    MapRead,
    /// 書きの対応付け。
    MapWrite,
    /// コピーの元。
    CopySrc,
    /// コピーの先。
    CopyDst,
    /// インデックスデータ。
    Index,
    /// 頂点データ。
    Vertex,
    /// 定数（uniform）データ。
    Uniform,
    /// 計算の入出力の記憶領域。
    Storage,
    /// 間接実行の引数。
    Indirect,
    /// 問合せの解決先。
    QueryResolve,
    /// 下位の加速構造の構築の入力。
    BlasInput,
    /// 上位の加速構造の構築の入力。
    TlasInput,
}

impl BufferUsageKind {
    /// 12 値のすべて（表の行の順）。
    pub const ALL: [Self; 12] = [
        Self::MapRead,
        Self::MapWrite,
        Self::CopySrc,
        Self::CopyDst,
        Self::Index,
        Self::Vertex,
        Self::Uniform,
        Self::Storage,
        Self::Indirect,
        Self::QueryResolve,
        Self::BlasInput,
        Self::TlasInput,
    ];

    /// この値に当たる契約の用途を返す。
    ///
    /// 常にちょうど 1 つのビットを持つ値を返し、空の用途や複数のビットの値は返さない。
    /// `BufferUsage::EXPORTABLE` はどの値からも返らない（契約だけの値）。
    pub const fn contract_usage(self) -> BufferUsage {
        match self {
            Self::MapRead => BufferUsage::MAP_READ,
            Self::MapWrite => BufferUsage::MAP_WRITE,
            Self::CopySrc => BufferUsage::DOWNLOAD,
            Self::CopyDst => BufferUsage::UPLOAD,
            Self::Index => BufferUsage::INDEX,
            Self::Vertex => BufferUsage::VERTEX,
            Self::Uniform => BufferUsage::UNIFORM,
            Self::Storage => BufferUsage::STORAGE,
            Self::Indirect => BufferUsage::INDIRECT,
            Self::QueryResolve => BufferUsage::QUERY_RESOLVE,
            Self::BlasInput => BufferUsage::BLAS_INPUT,
            Self::TlasInput => BufferUsage::TLAS_INPUT,
        }
    }

    /// この値がホストからの読み書きの経路（`upload`・`download`・対応付け）を持つとき `true`。
    ///
    /// 背後実装が確保をホストから見える領域に置く判断の入力になる。
    pub const fn has_host_path(self) -> bool {
        matches!(
            self,
            Self::MapRead | Self::MapWrite | Self::CopySrc | Self::CopyDst
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_mapping_all_rows_have_single_distinct_bit() {
        let mut seen = 0u32;
        for kind in BufferUsageKind::ALL {
            let bits = kind.contract_usage().bits();
            assert_eq!(bits.count_ones(), 1, "{kind:?}");
            assert_eq!(seen & bits, 0, "{kind:?} reuses a bit");
            seen |= bits;
        }
        assert_eq!(seen & BufferUsage::EXPORTABLE.bits(), 0);
    }
}
