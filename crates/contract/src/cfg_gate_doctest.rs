//! feature ゲートの有無でコンパイルの成否が切り替わることを rustdoc の doctest で確かめる置き場（`doc(hidden)`）。
//!
//! 外部 crate（trybuild 等）を足さずに「feature を有効にしない構成ではコンパイルエラー、
//! 有効にした構成では通る」を 1 つの snippet の対で確かめる。同じ snippet の fence を
//! `cfg_attr` で切り替えるため、無効側の失敗は「ゲートが無いと落ちる」ことを、
//! 有効側の成功は「snippet の import が正しい」ことを担保する（stable の rustdoc は
//! エラーコードを照合しないので、この対で失敗の理由の取り違えを防ぐ）。
//!
//! 流れる場所: 無効側は `scripts/build-profiles.sh` の組（既定の組など）の `cargo test`、
//! 有効側は `make verify`（`--all-features`）の doctest。
//! spec との対応: TASK-233 の 1/4、BUILD-11〜13（ポインタ表記）。`native-blocking-wait`・`ext-interop`
//! を使うコードの snippet は、対象のアイテムが入った時点で同じ形でここに足す。
//! 現状は仕組みの確認として、実在の feature ゲート `backend-metal` で対を置く。

/// `backend-metal` を有効にしない構成では `backend_metal` が解決できずコンパイルエラーになる。
#[cfg_attr(not(feature = "backend-metal"), doc = "```compile_fail")]
#[cfg_attr(feature = "backend-metal", doc = "```")]
#[doc = "use fandhe_silicon_contract::backend_metal;"]
#[doc = "```"]
#[allow(dead_code)] // doctest を載せるためだけのアイテムで、実行時には使わない。
pub struct BackendMetalGate;
