//! 比較基準の別 workspace（`baseline-wgpu/`）にある、公開 API に wgpu の型を出す lib crate。
//!
//! 製品の workspace の公開 API の検査（`scripts/public-api-check.sh`）が wgpu 系の型を検出できることを、
//! `--manifest-path` で本 workspace に当てて確かめるための入力であり、製品の crate からは呼ばれない。
//! 検査が働くには公開アイテムの型に wgpu の crate が現れる必要があるため、意図して wgpu の型を公開している。

/// 公開 API の戻り値に wgpu の型（`wgpu::Backends`）を出す関数。
///
/// 検査の入力として wgpu の型参照を 1 件作る役割だけを持ち、全 backend を表す値を返す。
pub fn all_backends() -> wgpu::Backends {
    wgpu::Backends::all()
}
