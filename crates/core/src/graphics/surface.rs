//! 表示先にかかわる値の型と、その値を照らす純粋関数の置き場（TASK-654）。
//!
//! `graphics` モジュールから再公開される（公開のパスは `fandhe_silicon_core::graphics::…`）。
//! 背後実装の API は呼ばず、状態を持たない。描画先の大きさの検査（`validate_offscreen_target_size`）の部品を使う。

use super::validate_offscreen_target_size;
use crate::error::ContractError;

/// 表示先の設定の幅・高さが 0 のとき、または `max_texture_size` を超えるときに拒む。上限ちょうどは通す（項目 `SURF-7`）。
/// 比べ方は描画先と同じ規則で、拒否の文言の先頭だけ `surface configuration: ` に付け替える。
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
}
