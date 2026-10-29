//! 用途と操作の対応の検査の純粋関数（TASK-1796 の 2 つ目の部品。MEM-109 のポインタ）。
//!
//! `memory` モジュールの一部。背後実装が、役割の操作を自分の記録に基づいて実行する前に
//! 通す検査で、確保した時点で背後実装が記録に残した用途（`BufferUsage`）を受け取って判定する。
//! `BufferHandle::usage` の値は判定の正にしない（呼び手が `__backend_new` で作れるため）。
//! 背後実装の API は呼ばず、確保もせず、バッファの内容にも触れない（エラーの文字列を除く）。
//! 検査の順（device のロスト・未提供・別 device の確認との前後）は背後実装の呼び出し側が決める。
//! 読み戻しの元・`upload` の宛先・テクスチャからバッファへのコピーの先は、
//! 対応する [`UsageRole`] を置いていないので、この検査の対象に含まれない。

use super::buffer::BufferUsage;
use crate::error::ContractError;

/// 用途を要る操作の役割。要る用途は [`UsageRole::required_usage`]。
///
/// 検査の対象外の操作に対応する variant は置かない（「要る用途なし」で通る variant を作らない）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum UsageRole {
    /// 読みの対応付け。
    MapRead,
    /// 書きの対応付け。
    MapWrite,
    /// バッファ間のコピーの元。
    CopyBufferSource,
    /// バッファからテクスチャへのコピーの元。
    CopyBufferToTextureSource,
    /// バッファ間のコピーの先。
    CopyBufferDestination,
    /// インデックス付きの描画のインデックスバッファ。
    IndexBuffer,
    /// 描画の頂点バッファ。
    VertexBuffer,
    /// uniform の束縛。
    UniformBinding,
    /// storage の束縛。
    StorageBinding,
    /// 間接実行の引数バッファ。
    IndirectArguments,
    /// 問合せの解決先。
    QueryResolveDestination,
    /// 下位の加速構造の構築の入力。
    BlasBuildInput,
    /// 上位の加速構造の構築の入力。
    TlasBuildInput,
    /// export の対象。
    Export,
}

impl UsageRole {
    /// 検査の対象の役割のすべて。
    pub const ALL: [Self; 14] = [
        Self::MapRead,
        Self::MapWrite,
        Self::CopyBufferSource,
        Self::CopyBufferToTextureSource,
        Self::CopyBufferDestination,
        Self::IndexBuffer,
        Self::VertexBuffer,
        Self::UniformBinding,
        Self::StorageBinding,
        Self::IndirectArguments,
        Self::QueryResolveDestination,
        Self::BlasBuildInput,
        Self::TlasBuildInput,
        Self::Export,
    ];

    /// この役割に要る用途。常にちょうど 1 つのビットを持つ。
    pub const fn required_usage(self) -> BufferUsage {
        match self {
            Self::MapRead => BufferUsage::MAP_READ,
            Self::MapWrite => BufferUsage::MAP_WRITE,
            Self::CopyBufferSource | Self::CopyBufferToTextureSource => BufferUsage::DOWNLOAD,
            Self::CopyBufferDestination => BufferUsage::UPLOAD,
            Self::IndexBuffer => BufferUsage::INDEX,
            Self::VertexBuffer => BufferUsage::VERTEX,
            Self::UniformBinding => BufferUsage::UNIFORM,
            Self::StorageBinding => BufferUsage::STORAGE,
            Self::IndirectArguments => BufferUsage::INDIRECT,
            Self::QueryResolveDestination => BufferUsage::QUERY_RESOLVE,
            Self::BlasBuildInput => BufferUsage::BLAS_INPUT,
            Self::TlasBuildInput => BufferUsage::TLAS_INPUT,
            Self::Export => BufferUsage::EXPORTABLE,
        }
    }

    /// エラーの文字列に載せる役割の名前（英語）。
    const fn role_name(self) -> &'static str {
        match self {
            Self::MapRead => "map-read",
            Self::MapWrite => "map-write",
            Self::CopyBufferSource => "copy-buffer-source",
            Self::CopyBufferToTextureSource => "copy-buffer-to-texture-source",
            Self::CopyBufferDestination => "copy-buffer-destination",
            Self::IndexBuffer => "index-buffer",
            Self::VertexBuffer => "vertex-buffer",
            Self::UniformBinding => "uniform-binding",
            Self::StorageBinding => "storage-binding",
            Self::IndirectArguments => "indirect-arguments",
            Self::QueryResolveDestination => "query-resolve-destination",
            Self::BlasBuildInput => "blas-build-input",
            Self::TlasBuildInput => "tlas-build-input",
            Self::Export => "export",
        }
    }

    /// エラーの文字列に載せる、要る用途の値の名前（英語）。
    const fn usage_name(self) -> &'static str {
        match self {
            Self::MapRead => "MAP_READ",
            Self::MapWrite => "MAP_WRITE",
            Self::CopyBufferSource | Self::CopyBufferToTextureSource => "DOWNLOAD",
            Self::CopyBufferDestination => "UPLOAD",
            Self::IndexBuffer => "INDEX",
            Self::VertexBuffer => "VERTEX",
            Self::UniformBinding => "UNIFORM",
            Self::StorageBinding => "STORAGE",
            Self::IndirectArguments => "INDIRECT",
            Self::QueryResolveDestination => "QUERY_RESOLVE",
            Self::BlasBuildInput => "BLAS_INPUT",
            Self::TlasBuildInput => "TLAS_INPUT",
            Self::Export => "EXPORTABLE",
        }
    }
}

/// 確保時に申告した用途 `declared` が `role` に要る用途を含むかを判定する。
///
/// 過申告（要る用途以外のビットも立っている）は通す。空の用途はどの役割でも拒否する。
/// 1 つの操作が複数の役割でバッファを使うときは、呼び手が役割ごとに呼ぶ。
/// `declared` は背後実装が確保の時に記録した値を渡すこと（ハンドルの `usage()` を正にしない）。
///
/// # Errors
///
/// 要る用途が `declared` に無いとき `ContractError::InvalidArgument`。文字列に役割の名前と
/// 足りない用途の値の名前を載せる。
pub fn validate_usage_for_role(
    declared: BufferUsage,
    role: UsageRole,
) -> Result<(), ContractError> {
    if declared.contains(role.required_usage()) {
        return Ok(());
    }
    Err(ContractError::InvalidArgument(format!(
        "buffer usage lacks {} required for {}",
        role.usage_name(),
        role.role_name()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_check_declared_passes_and_missing_is_rejected() {
        assert!(validate_usage_for_role(BufferUsage::INDEX, UsageRole::IndexBuffer).is_ok());
        let err = validate_usage_for_role(BufferUsage::VERTEX, UsageRole::IndexBuffer);
        assert!(
            matches!(err, Err(ContractError::InvalidArgument(ref m)) if m.contains("INDEX") && m.contains("index-buffer"))
        );
    }

    #[test]
    fn usage_check_roles_require_single_bit() {
        for role in UsageRole::ALL {
            assert_eq!(role.required_usage().bits().count_ones(), 1, "{role:?}");
        }
    }
}
