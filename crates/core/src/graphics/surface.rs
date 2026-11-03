//! 表示先にかかわる値の型と、その値を照らす純粋関数の置き場（TASK-654）。
//!
//! `graphics` モジュールから再公開される（公開のパスは `fandhe_silicon_core::graphics::…`）。
//! 背後実装の API は呼ばず、状態を持たない。描画先の大きさの検査（`validate_offscreen_target_size`）の部品を使う。

use super::validate_offscreen_target_size;
use crate::error::ContractError;

/// 表示先の設定の幅・高さが 0 のとき、または `max_texture_size` を超えるときに拒む。上限ちょうどは通す（項目 `SURF-7`）。
/// 比べ方は描画先と同じ規則で、拒否の文言の先頭に `surface configuration: ` を足す（元の文言は後ろに残る）。
///
/// ```
/// use fandhe_silicon_core::error::ContractError;
/// use fandhe_silicon_core::graphics::validate_surface_size;
/// assert_eq!(validate_surface_size(1280, 720, 16384), Ok(()));
/// assert_eq!(validate_surface_size(16384, 16384, 16384), Ok(()));
/// assert!(matches!(
///     validate_surface_size(0, 720, 16384),
///     Err(ContractError::InvalidArgument(_))
/// ));
/// assert!(matches!(
///     validate_surface_size(16385, 1, 16384),
///     Err(ContractError::InvalidArgument(_))
/// ));
/// ```
///
/// # Errors
///
/// 幅または高さが 0 のとき、または幅か高さが `max_texture_size` を超えるとき
/// `ContractError::InvalidArgument`。文言の先頭は `surface configuration:`。
pub fn validate_surface_size(
    width: u32,
    height: u32,
    max_texture_size: u32,
) -> Result<(), ContractError> {
    validate_offscreen_target_size(width, height, max_texture_size).map_err(with_surface_prefix)
}

/// 拒否の文言の先頭に `surface configuration: ` を付ける。`InvalidArgument` 以外はそのまま返す。
fn with_surface_prefix(e: ContractError) -> ContractError {
    match e {
        ContractError::InvalidArgument(m) => {
            ContractError::InvalidArgument(format!("surface configuration: {m}"))
        }
        other => other,
    }
}

/// 表示先から取得した結果（項目 `SURF-9`）。`Success`・`Suboptimal` だけが取得した値を持つ。
///
/// 外の crate からは、ワイルドカードの腕の無い `match` は書けない。
///
/// ```
/// use fandhe_silicon_core::graphics::{CurrentSurfaceTexture, SurfaceStatus};
/// assert_eq!(CurrentSurfaceTexture::Success(1u8).status(), SurfaceStatus::Good);
/// assert_eq!(CurrentSurfaceTexture::<u8>::Outdated.status(), SurfaceStatus::Outdated);
/// ```
///
/// ```compile_fail,E0004
/// use fandhe_silicon_core::graphics::CurrentSurfaceTexture;
/// fn f(c: CurrentSurfaceTexture<u8>) -> u8 {
///     match c {
///         CurrentSurfaceTexture::Success(_) => 0,
///         CurrentSurfaceTexture::Suboptimal(_) => 1,
///         CurrentSurfaceTexture::Timeout => 2,
///         CurrentSurfaceTexture::Occluded => 3,
///         CurrentSurfaceTexture::Outdated => 4,
///         CurrentSurfaceTexture::Lost => 5,
///         CurrentSurfaceTexture::Validation => 6,
///     }
/// }
/// ```
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CurrentSurfaceTexture<T> {
    /// 取得した値を持つ。
    Success(T),
    /// 取得した値を持つ。
    Suboptimal(T),
    /// 取得した値を持たない。
    Timeout,
    /// 取得した値を持たない。
    Occluded,
    /// 表示先が古くなったことを表す。取得した値を持たない。
    Outdated,
    /// 取得した値を持たない。
    Lost,
    /// 取得した値を持たない。
    Validation,
}

/// 取得の結果から取得した値を除いた、状態だけの値。外の crate からは、ワイルドカードの腕の無い `match` は書けない。
///
/// ```compile_fail,E0004
/// use fandhe_silicon_core::graphics::SurfaceStatus;
/// fn f(s: SurfaceStatus) -> u8 {
///     match s {
///         SurfaceStatus::Good => 0,
///         SurfaceStatus::Suboptimal => 1,
///         SurfaceStatus::Timeout => 2,
///         SurfaceStatus::Occluded => 3,
///         SurfaceStatus::Outdated => 4,
///         SurfaceStatus::Lost => 5,
///         SurfaceStatus::Validation => 6,
///     }
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SurfaceStatus {
    /// `CurrentSurfaceTexture::Success` に対応する。
    Good,
    /// `CurrentSurfaceTexture::Suboptimal` に対応する。
    Suboptimal,
    /// `CurrentSurfaceTexture::Timeout` に対応する。
    Timeout,
    /// `CurrentSurfaceTexture::Occluded` に対応する。
    Occluded,
    /// `CurrentSurfaceTexture::Outdated` に対応する。
    Outdated,
    /// `CurrentSurfaceTexture::Lost` に対応する。
    Lost,
    /// `CurrentSurfaceTexture::Validation` に対応する。
    Validation,
}

impl SurfaceStatus {
    /// 全 7 値（宣言の順）。
    pub const ALL: &'static [Self] = &[
        Self::Good,
        Self::Suboptimal,
        Self::Timeout,
        Self::Occluded,
        Self::Outdated,
        Self::Lost,
        Self::Validation,
    ];
}

impl<T> CurrentSurfaceTexture<T> {
    /// 取得した値を除いた状態を返す。`Success` は `Good`、ほかは同じ名前の値。
    pub fn status(&self) -> SurfaceStatus {
        match self {
            Self::Success(_) => SurfaceStatus::Good,
            Self::Suboptimal(_) => SurfaceStatus::Suboptimal,
            Self::Timeout => SurfaceStatus::Timeout,
            Self::Occluded => SurfaceStatus::Occluded,
            Self::Outdated => SurfaceStatus::Outdated,
            Self::Lost => SurfaceStatus::Lost,
            Self::Validation => SurfaceStatus::Validation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const L: u32 = 16384;

    fn is_invalid(r: Result<(), ContractError>) -> bool {
        matches!(r, Err(ContractError::InvalidArgument(_)))
    }

    #[test]
    fn surface_size_zero_is_rejected() {
        for (w, h) in [(0, 1), (1, 0), (0, 0)] {
            assert!(is_invalid(validate_surface_size(w, h, L)), "{w}x{h}");
        }
    }

    #[test]
    fn surface_size_at_limit_passes() {
        for (w, h) in [(L, L), (L, 1), (1, L)] {
            assert_eq!(validate_surface_size(w, h, L), Ok(()), "{w}x{h}");
        }
    }

    #[test]
    fn surface_size_over_limit_is_rejected() {
        assert!(is_invalid(validate_surface_size(L + 1, 1, L)));
        assert!(is_invalid(validate_surface_size(1, L + 1, L)));
    }

    #[test]
    fn surface_size_limit_extremes() {
        assert!(is_invalid(validate_surface_size(1, 1, 0)));
        assert_eq!(validate_surface_size(u32::MAX, u32::MAX, u32::MAX), Ok(()));
    }

    #[test]
    fn surface_size_message_has_prefix() {
        for (w, h) in [(0, 1), (L + 1, 1)] {
            match validate_surface_size(w, h, L) {
                Err(ContractError::InvalidArgument(m)) => {
                    assert!(m.starts_with("surface configuration: "), "{m}");
                }
                other => panic!("unexpected: {other:?}"),
            }
        }
    }

    fn all_results() -> [CurrentSurfaceTexture<u8>; 7] {
        use CurrentSurfaceTexture as C;
        [
            C::Success(1),
            C::Suboptimal(1),
            C::Timeout,
            C::Occluded,
            C::Outdated,
            C::Lost,
            C::Validation,
        ]
    }

    #[test]
    fn surface_status_mapping() {
        use SurfaceStatus as S;
        let expected = [
            S::Good,
            S::Suboptimal,
            S::Timeout,
            S::Occluded,
            S::Outdated,
            S::Lost,
            S::Validation,
        ];
        for (r, e) in all_results().iter().zip(expected) {
            assert_eq!(r.status(), e, "{r:?}");
        }
    }

    #[test]
    fn surface_status_all_distinct() {
        let set: std::collections::HashSet<_> = SurfaceStatus::ALL.iter().copied().collect();
        assert_eq!(SurfaceStatus::ALL.len(), 7);
        assert_eq!(set.len(), 7);
    }

    #[test]
    fn surface_status_all_matches_results() {
        let all: std::collections::HashSet<_> = SurfaceStatus::ALL.iter().copied().collect();
        let got: Vec<_> = all_results().iter().map(|r| r.status()).collect();
        let got_set: std::collections::HashSet<_> = got.iter().copied().collect();
        assert_eq!(got.len(), 7);
        assert_eq!(got_set, all);
    }
}
