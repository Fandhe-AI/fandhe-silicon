//! 比較基準の別 workspace（`baseline-wgpu/`）にある、公開 API に wgpu の型を出す lib crate。
//!
//! 製品の workspace の公開 API の検査（`scripts/public-api-check.sh`）が wgpu 系の型を検出できることを、
//! `--manifest-path` で本 workspace に当てて確かめるための入力であり、製品の crate からは呼ばれない。
//! 検査が働くには公開アイテムの型に wgpu の crate が現れる必要があるため、意図して wgpu の型を公開している。
//! 公開の struct の field、trait のメソッドの引数と戻り値、その trait の impl の 3 種の位置に wgpu の型を置く。
//! 形の対応づけは TASK-255 のポインタのみ。

/// 公開の field に wgpu の型を持つ struct。
///
/// 検査の入力として、struct の field の位置に wgpu の型を出す役割だけを持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdapterPreference {
    /// 使う backend の集合（wgpu の型）。
    pub backends: wgpu::Backends,
    /// アダプタ選択の電力の方針（wgpu の型）。
    pub power: wgpu::PowerPreference,
}

/// wgpu の型をメソッドの戻り値と引数・戻り値に持つ trait。
///
/// 検査の入力として、trait のメソッドの位置に wgpu の型を出す役割だけを持つ。
pub trait PreferenceSource {
    /// 既定の backend の集合を返す（戻り値に wgpu の型）。
    fn default_backends(&self) -> wgpu::Backends;

    /// 渡した電力の方針を、この実装での扱いに直して返す（引数と戻り値に wgpu の型）。
    fn resolve_power(&self, requested: wgpu::PowerPreference) -> wgpu::PowerPreference;
}

impl PreferenceSource for AdapterPreference {
    /// 保持している backend の集合をそのまま返す。
    fn default_backends(&self) -> wgpu::Backends {
        self.backends
    }

    /// 方針が未指定（`None`）のときだけ保持している方針を返し、それ以外は渡された方針を返す。
    fn resolve_power(&self, requested: wgpu::PowerPreference) -> wgpu::PowerPreference {
        match requested {
            wgpu::PowerPreference::None => self.power,
            other => other,
        }
    }
}
