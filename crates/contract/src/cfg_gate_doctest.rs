//! feature ゲートの有無でコンパイルの成否が切り替わることを rustdoc の doctest で確かめる置き場（`doc(hidden)`）。
//!
//! 外部 crate（trybuild 等）を足さずに「feature を有効にしない構成ではコンパイルエラー、
//! 有効にした構成では通る」を 1 つの snippet の対で確かめる。同じ snippet の fence を
//! `cfg_attr` で切り替えるため、無効側の失敗は「ゲートが無いと落ちる」ことを、
//! 有効側の成功は「snippet の import が正しい」ことを担保する（stable の rustdoc は
//! エラーコードを照合しないので、この対で失敗の理由の取り違えを防ぐ）。
//!
//! 流れる場所: 無効側は `scripts/build-profiles.sh` の組（既定の組など）の `cargo test`、
//! 有効側は `make verify`（`--all-features`）の doctest と、`scripts/build-profiles.sh` の feature 単独の組。
//! 有効側の確かめは `tests/feature_gate_enabled.rs` が受け持つ。
//! spec との対応: TASK-233 の 1/4〜3/4・5/7、BUILD-11〜13（ポインタ表記）。
//! 置く対: 実在の feature ゲート `backend-metal`、`native-blocking-wait` の core の 2 アイテム
//! （`MemoryOps::download_blocking`・`CommandQueue::wait_blocking`。別々の snippet）、
//! `fault-injection`（core の `fault` モジュールと `Device::fault_hooks`。TASK-271）。
//! feature に依らず通る `download_async`・`on_complete` の doctest も置く。
//! `ext-interop` は門の目印の対のままで、枠の確かめにとどまる。

/// `backend-metal` を有効にしない構成では `backend_metal` が解決できずコンパイルエラーになる。
#[cfg_attr(not(feature = "backend-metal"), doc = "```compile_fail")]
#[cfg_attr(feature = "backend-metal", doc = "```")]
#[doc = "use fandhe_silicon_contract::backend_metal;"]
#[doc = "```"]
#[allow(dead_code)] // doctest を載せるためだけのアイテムで、実行時には使わない。
pub struct BackendMetalGate;

/// `native-blocking-wait` を有効にしない構成では `MemoryOps::download_blocking` が解決できずコンパイルエラーになる
/// （contract の feature が core の同名 feature へ転送されることの確かめ）。
#[cfg_attr(not(feature = "native-blocking-wait"), doc = "```compile_fail")]
#[cfg_attr(feature = "native-blocking-wait", doc = "```")]
#[doc = "use fandhe_silicon_core::memory::{BufferHandle, MemoryOps};"]
#[doc = "fn probe(ops: &dyn MemoryOps, buffer: &BufferHandle) {"]
#[doc = "    let _ = ops.download_blocking(buffer, 0, 4);"]
#[doc = "}"]
#[doc = "```"]
#[allow(dead_code)] // doctest を載せるためだけのアイテムで、実行時には使わない。
pub struct DownloadBlockingGate;

/// `native-blocking-wait` を有効にしない構成では `CommandQueue::wait_blocking` が解決できずコンパイルエラーになる。
/// `download_blocking` の対とは別の snippet にし、片方だけ門の外に出たときも無効側が気づけるようにする。
#[cfg_attr(not(feature = "native-blocking-wait"), doc = "```compile_fail")]
#[cfg_attr(feature = "native-blocking-wait", doc = "```")]
#[doc = "use fandhe_silicon_core::execution::{CommandQueue, SubmitToken};"]
#[doc = "fn probe(queue: &dyn CommandQueue, token: SubmitToken) {"]
#[doc = "    let _ = queue.wait_blocking(token, None);"]
#[doc = "}"]
#[doc = "```"]
#[allow(dead_code)] // doctest を載せるためだけのアイテムで、実行時には使わない。
pub struct WaitBlockingGate;

/// feature に依らずすべての構成で通る非ブロッキングの口。`download_async` と `on_complete` は門の外にある。
/// （無効の構成に残る読み戻しと完了の通知が、この 2 つであることの確かめ。）
///
/// ```
/// use fandhe_silicon_core::error::{CompletionState, ContractError};
/// use fandhe_silicon_core::execution::{CommandQueue, SubmitToken};
/// use fandhe_silicon_core::memory::{BufferHandle, MemoryOps};
/// fn probe(ops: &dyn MemoryOps, buffer: &BufferHandle, queue: &dyn CommandQueue, token: SubmitToken) {
///     ops.download_async(buffer, 0, 4, Box::new(|_: Result<Vec<u8>, ContractError>| {}));
///     queue.on_complete(token, Box::new(|_: CompletionState| {}));
/// }
/// ```
#[allow(dead_code)] // doctest を載せるためだけのアイテムで、実行時には使わない。
pub struct NonBlockingReadbackGate;

/// `ext-interop` を有効にした構成にだけ存在する門の目印。無効側の doctest が解決に失敗することの根拠になる。
#[cfg(feature = "ext-interop")]
pub struct ExtInteropEnabled;

/// `ext-interop` を有効にしない構成では目印が解決できずコンパイルエラーになる。
#[cfg_attr(not(feature = "ext-interop"), doc = "```compile_fail")]
#[cfg_attr(feature = "ext-interop", doc = "```")]
#[doc = "use fandhe_silicon_contract::cfg_gate_doctest::ExtInteropEnabled;"]
#[doc = "```"]
#[allow(dead_code)] // doctest を載せるためだけのアイテムで、実行時には使わない。
pub struct ExtInteropGate;

/// `fault-injection` を有効にしない構成では core の `fault` モジュールが解決できずコンパイルエラーになる
/// （contract の feature が core の同名 feature へ転送されることの確かめ）。
#[cfg_attr(not(feature = "fault-injection"), doc = "```compile_fail")]
#[cfg_attr(feature = "fault-injection", doc = "```")]
#[doc = "use fandhe_silicon_core::fault::FaultHooks;"]
#[doc = "```"]
#[allow(dead_code)] // doctest を載せるためだけのアイテムで、実行時には使わない。
pub struct FaultInjectionGate;

/// `fault-injection` を有効にしない構成では `Device::fault_hooks` が解決できずコンパイルエラーになる
/// （core の `Device` の既定メソッドが同じ feature の裏にあることの確かめ）。
#[cfg_attr(not(feature = "fault-injection"), doc = "```compile_fail")]
#[cfg_attr(feature = "fault-injection", doc = "```")]
#[doc = "use fandhe_silicon_core::device::Device;"]
#[doc = "fn probe(device: &dyn Device) {"]
#[doc = "    let _ = device.fault_hooks();"]
#[doc = "}"]
#[doc = "```"]
#[allow(dead_code)] // doctest を載せるためだけのアイテムで、実行時には使わない。
pub struct DeviceFaultHooksGate;
