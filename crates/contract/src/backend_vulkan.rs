//! Vulkan の背後実装（下-1）。`backend-vulkan` feature で有効になる。
//!
//! この背後実装の `unsafe` / FFI はこのモジュールに閉じ込め、外へは safe な API だけを出す。
//! ローダは実行時に動的に読み込む `ash` に依存する（wasm32 の依存グラフには入らない）。
//! wasm32 ではコンパイル対象から外す（`lib.rs` の `cfg`）。
//! 子モジュール `spirv_operands` は検証用の道具で、feature `test-support` の裏にだけ公開される
//! （TASK-98。説明は `docs/vulkan-operand-extraction.md`）。
//!
//! 公開の入口 `VulkanDeviceProvider` は Vulkan の `DeviceProvider` で、device を作る前に要求の
//! 組み合わせを照らし、通った要求には instance・物理 device・queue family・logical device・
//! command pool を作って非公開の device を返す（TASK-159 のポインタ。説明は `docs/vulkan-device-request.md`）。
//!
//! spec との対応: TASK-5（ポインタ表記）。

#[cfg(feature = "test-support")]
pub mod spirv_operands;

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use ash::vk;
use fandhe_silicon_core::capability::{
    BackendKind, Capabilities, CapabilityLevel, CapabilityQuery, FeatureId, LimitsTable,
};
use fandhe_silicon_core::capability_table::UNLISTED_FEATURE_QUERY;
use fandhe_silicon_core::device::{
    __backend_next_device_id, Device, DeviceDiagnostics, DeviceOwnership, DeviceProvider,
    DeviceRequest, Profile,
};
use fandhe_silicon_core::error::{CompletionState, ContractError};
use fandhe_silicon_core::execution::{
    __backend_next_encoder_id, Binding, CommandEncoder, CommandQueue, ConstantBinding,
    ImportedSemaphoreHandle, PipelineHandle, SubmitToken, ThreadgroupMemory,
};
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage, MemoryOps};

/// この provider が受け持つ背後実装。
const BACKEND: BackendKind = BackendKind::Vulkan;

/// `DeviceDiagnostics::backend_name` に載せる名前。
const BACKEND_NAME: &str = "vulkan";

/// device の世代の採番。背後実装の中で 1 から単調増加に取る。
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

/// カウンタの現在値を返して 1 進める。使い切ったら `None`。`open_device` から呼ばれる。
fn take_next(counter: &AtomicU64) -> Option<u64> {
    let mut current = counter.load(Ordering::Relaxed);
    loop {
        let next = current.checked_add(1)?;
        match counter.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return Some(current),
            Err(seen) => current = seen,
        }
    }
}

/// device を作る前に要求を照らす。背後実装 → プロファイル → 所有形態の順で、最初に当たったものを返す。
///
/// `VulkanDeviceProvider::request` から呼ばれ、native の資源には触れない。
fn check_request(req: &DeviceRequest) -> Result<(), ContractError> {
    if req.backend != BACKEND {
        return Err(ContractError::InvalidCombination(format!(
            "Vulkan provider cannot serve backend {:?}",
            req.backend
        )));
    }
    match req.profile {
        Profile::ComputeOnly => {}
        Profile::Graphics => {
            #[cfg(not(feature = "graphics"))]
            return Err(ContractError::InvalidCombination(
                "profile Graphics requires the `graphics` feature of the contract crate".into(),
            ));
        }
        Profile::CpuOnly => {
            return Err(ContractError::InvalidCombination(format!(
                "backend {BACKEND:?} does not serve profile {:?}",
                req.profile
            )));
        }
        _ => {
            return Err(ContractError::InvalidCombination(
                "unknown device profile".into(),
            ));
        }
    }
    match req.ownership {
        DeviceOwnership::ProductIndependent => Ok(()),
        DeviceOwnership::Shared => Err(ContractError::Unsupported {
            op: "request(DeviceOwnership::Shared)",
            backend: BACKEND,
        }),
        DeviceOwnership::Imported { .. } => Err(ContractError::Unsupported {
            op: "request(DeviceOwnership::Imported)",
            backend: BACKEND,
        }),
        _ => Err(ContractError::InvalidCombination(
            "unknown device ownership".into(),
        )),
    }
}

/// Vulkan の `DeviceProvider` の入口。
///
/// Vulkan でない背後実装・`CpuOnly`・feature `graphics` の無いビルドの `Graphics` は
/// `InvalidCombination`、`Shared`・`Imported` は `Unsupported` で拒む。検査を通った要求には
/// device を作って返し、作る途中の失敗は `DeviceUnavailable("<呼び出し>: <エラー>")` で返す。
#[derive(Debug, Clone, Copy, Default)]
pub struct VulkanDeviceProvider;

impl DeviceProvider for VulkanDeviceProvider {
    fn request(req: DeviceRequest) -> Result<Box<dyn Device>, ContractError> {
        check_request(&req)?;
        Ok(Box::new(open_device(&req)?))
    }
}

/// 差し込みの失敗の文字列に載せる印。
#[cfg(feature = "fault-injection")]
const INJECTED: &str = "injected";

/// 差し込みの失敗の `DeviceUnavailable` の文字列の末尾に印を足す。ほかの失敗はそのまま返す。
#[cfg(feature = "fault-injection")]
fn mark_injected(e: ContractError) -> ContractError {
    match e {
        ContractError::DeviceUnavailable(s) => {
            ContractError::DeviceUnavailable(format!("{s} ({INJECTED})"))
        }
        other => other,
    }
}

/// 選ぶ関数の結果から失敗側を取り出して印を付ける。成功なら印だけの `DeviceUnavailable` を返す。
#[cfg(feature = "fault-injection")]
fn injected_from<T>(r: Result<T, ContractError>) -> ContractError {
    match r {
        Err(e) => mark_injected(e),
        Ok(_) => ContractError::DeviceUnavailable(INJECTED.into()),
    }
}

// 初期化の失敗を差し込む provider を作る。各 `request` は検査の後に差し込みの失敗を返し、
// 実の `request` の経路（`VulkanDeviceProvider`）には触れない。
macro_rules! injected_provider {
    ($($(#[$doc:meta])* $name:ident => $failure:expr;)*) => {$(
        $(#[$doc])*
        #[cfg(feature = "fault-injection")]
        #[derive(Debug, Clone, Copy, Default)]
        pub struct $name;

        #[cfg(feature = "fault-injection")]
        impl DeviceProvider for $name {
            fn request(req: DeviceRequest) -> Result<Box<dyn Device>, ContractError> {
                check_request(&req)?;
                Err($failure)
            }
        }
    )*};
}

injected_provider! {
    /// ローダが読めない失敗を差し込む入口。実際の故障ではなく、feature `fault-injection` の裏にある。
    VulkanLoaderMissingProvider => ContractError::DeviceUnavailable(
        "Entry::load: Vulkan loader is not loadable (injected)".into(),
    );
    /// instance の作成の失敗を差し込む入口。実際の故障ではなく、feature `fault-injection` の裏にある。
    VulkanCreateInstanceFailsProvider => mark_injected(vk_failure(
        "create_instance",
        vk::Result::ERROR_INITIALIZATION_FAILED,
    ));
    /// 物理 device の列挙の失敗を差し込む入口。実際の故障ではなく、feature `fault-injection` の裏にある。
    VulkanEnumeratePhysicalDevicesFailsProvider => mark_injected(vk_failure(
        "enumerate_physical_devices",
        vk::Result::ERROR_INITIALIZATION_FAILED,
    ));
    /// 物理 device が 1 つも無い失敗を差し込む入口。実際の故障ではなく、feature `fault-injection` の裏にある。
    VulkanNoPhysicalDeviceProvider => injected_from(first_physical_device::<u8>(&[]));
    /// 要求を満たす queue family が無い失敗を差し込む入口。実際の故障ではなく、feature `fault-injection` の裏にある。
    VulkanNoQueueFamilyProvider => injected_from(select_queue_family(&[], Profile::ComputeOnly));
    /// logical device の作成の失敗を差し込む入口。実際の故障ではなく、feature `fault-injection` の裏にある。
    VulkanCreateDeviceFailsProvider => mark_injected(vk_failure(
        "create_device",
        vk::Result::ERROR_INITIALIZATION_FAILED,
    ));
    /// command pool の作成の失敗を差し込む入口。実際の故障ではなく、feature `fault-injection` の裏にある。
    VulkanCreateCommandPoolFailsProvider => mark_injected(vk_failure(
        "create_command_pool",
        vk::Result::ERROR_OUT_OF_DEVICE_MEMORY,
    ));
}

/// Vulkan の呼び出しの失敗を `DeviceUnavailable("<呼び出し>: <エラー>")` にする。
fn vk_failure(call: &str, result: vk::Result) -> ContractError {
    ContractError::DeviceUnavailable(format!("{call}: {result:?}"))
}

/// ローダを読み込む。`Entry::load()` を呼ぶ唯一の場所で、探す場所の指定も環境変数の読み取りもしない。
fn load_entry() -> Result<ash::Entry, ContractError> {
    // SAFETY: `Entry::load` は Vulkan のローダの共有ライブラリを読み込んで関数表を引く。呼び出しが前提とする
    // 引数は無い。返る `Entry` は読み込んだライブラリを保持するので、持ち主の型の最後の欄に置いて
    // instance・device より後まで生かす。
    unsafe { ash::Entry::load() }
        .map_err(|e| ContractError::DeviceUnavailable(format!("Entry::load: {e:?}")))
}

/// 診断の `adapter_name` を作る。`<名前> (vulkan api=<major>.<minor>.<patch> driver_version=0x<8 桁の 16 進>)`。
fn adapter_name_of(name: &str, api_version: u32, driver_version: u32) -> String {
    format!(
        "{name} (vulkan api={}.{}.{} driver_version=0x{driver_version:08x})",
        vk::api_version_major(api_version),
        vk::api_version_minor(api_version),
        vk::api_version_patch(api_version),
    )
}

/// 列挙した物理 device の先頭を返す。空なら `DeviceUnavailable`。
fn first_physical_device<T: Copy>(devices: &[T]) -> Result<T, ContractError> {
    devices.first().copied().ok_or_else(|| {
        ContractError::DeviceUnavailable("enumerate_physical_devices: no physical device".into())
    })
}

/// プロファイルを満たす最初の queue family の添字を返す。無ければ `DeviceUnavailable`。
///
/// `queue_count` が 0 の家族は飛ばす。`ComputeOnly` は `COMPUTE`、`Graphics` は `COMPUTE | GRAPHICS` を要る。
fn select_queue_family(
    families: &[vk::QueueFamilyProperties],
    profile: Profile,
) -> Result<u32, ContractError> {
    let required = match profile {
        Profile::ComputeOnly => Some(vk::QueueFlags::COMPUTE),
        Profile::Graphics => Some(vk::QueueFlags::COMPUTE | vk::QueueFlags::GRAPHICS),
        _ => None,
    };
    required
        .and_then(|required| {
            families
                .iter()
                .position(|f| f.queue_count > 0 && f.queue_flags.contains(required))
        })
        .and_then(|i| u32::try_from(i).ok())
        .ok_or_else(|| {
            ContractError::DeviceUnavailable(
                "get_physical_device_queue_family_properties: no queue family supports the requested profile"
                    .into(),
            )
        })
}

/// device の作成で有効にする機能。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EnabledFeatures {
    draw_indirect_first_instance: bool,
}

/// 物理 device が対応する機能のうち、作成で有効にするものを選ぶ。
fn enabled_features(supported: &vk::PhysicalDeviceFeatures) -> EnabledFeatures {
    EnabledFeatures {
        draw_indirect_first_instance: supported.draw_indirect_first_instance == vk::TRUE,
    }
}

/// `create_device` に渡す機能の組。有効にする機能の欄だけを立て、残りは既定の値のまま。
fn to_vk_features(enabled: EnabledFeatures) -> vk::PhysicalDeviceFeatures {
    vk::PhysicalDeviceFeatures {
        draw_indirect_first_instance: vk::Bool32::from(enabled.draw_indirect_first_instance),
        ..Default::default()
    }
}

/// 子から順に解放する。command pool → device → instance の順で、無いものは飛ばす。
///
/// `Native` の `Drop` から呼ばれる。試験では閉包で順を観測する。
fn release_in_order<P, D, I>(
    pool: Option<P>,
    device: Option<D>,
    instance: I,
    destroy_pool: impl FnOnce(P),
    destroy_device: impl FnOnce(D),
    destroy_instance: impl FnOnce(I),
) {
    if let Some(pool) = pool {
        destroy_pool(pool);
    }
    if let Some(device) = device {
        destroy_device(device);
    }
    destroy_instance(instance);
}

/// native のハンドルの持ち主。`Drop` が子から順に解放する。
///
/// instance を作った時点で作り、device・command pool は作れた順に埋める。`request` の途中で失敗しても
/// この型が落ちるので、作り終えた分は同じ順で解放される。`_entry` は最後の欄で、最後に落ちる。
struct Native {
    command_pool: Option<vk::CommandPool>,
    device: Option<ash::Device>,
    instance: ash::Instance,
    _entry: ash::Entry,
}

impl Drop for Native {
    fn drop(&mut self) {
        let pool_owner = self.device.clone();
        release_in_order(
            self.command_pool.take(),
            self.device.take(),
            self.instance.clone(),
            |pool| {
                if let Some(device) = &pool_owner {
                    // SAFETY: `pool` はこの型が `device` から作った command pool で、`take` で外した
                    // 1 回だけ破棄する。`device` は次の段で破棄するので、この時点で生きている。
                    // 投入を受け付けないので、pool から確保した command buffer は無い。
                    unsafe { device.destroy_command_pool(pool, None) };
                }
            },
            |device| {
                // SAFETY: `device` はこの型が `instance` から作ったもので、`take` で外した 1 回だけ破棄する。
                // 子の command pool は直前に破棄済みで、`instance` は次の段で破棄するので生きている。
                unsafe { device.destroy_device(None) };
            },
            |instance| {
                // SAFETY: `instance` はこの型が `_entry` から作ったもので、ここで 1 回だけ破棄する。
                // 子の device・command pool は破棄済みで、`_entry` は最後の欄として後で落ちる。
                unsafe { instance.destroy_instance(None) };
            },
        );
    }
}

/// Vulkan の device。非公開で、`DeviceProvider::request` が `Box<dyn Device>` にして返す。
struct VulkanDevice {
    id: u64,
    generation: u64,
    ownership: DeviceOwnership,
    adapter_name: String,
    limits: LimitsTable,
    native: Native,
}

impl fmt::Debug for VulkanDevice {
    /// native のハンドルは出さない。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VulkanDevice")
            .field("id", &self.id)
            .field("generation", &self.generation)
            .field("adapter_name", &self.adapter_name)
            .field("has_command_pool", &self.native.command_pool.is_some())
            .finish_non_exhaustive()
    }
}

/// 要求どおりの device を作る。`VulkanDeviceProvider::request` から検査の後に呼ばれる。
///
/// 呼び出しの順は、ローダ → instance → 物理 device の列挙 → 性質・queue family・機能の読み取り →
/// device → command pool。どこで失敗しても、作り終えた分は `Native` の `Drop` が解放する。
fn open_device(req: &DeviceRequest) -> Result<VulkanDevice, ContractError> {
    let exhausted = || ContractError::DeviceUnavailable("device counter space is exhausted".into());
    let entry = load_entry()?;

    let app_info = vk::ApplicationInfo::default().api_version(vk::API_VERSION_1_0);
    let instance_info = vk::InstanceCreateInfo::default().application_info(&app_info);
    // SAFETY: `instance_info` と、それが指す `app_info` は呼び出しの間だけ生きる局所の値で、
    // 層・拡張の配列は空（null と 0 件）。`entry` は生きている。
    let instance = unsafe { entry.create_instance(&instance_info, None) }
        .map_err(|e| vk_failure("create_instance", e))?;
    let mut native = Native {
        command_pool: None,
        device: None,
        instance,
        _entry: entry,
    };

    // SAFETY: `native.instance` はこの関数が作った生きている instance。
    let physical_devices = unsafe { native.instance.enumerate_physical_devices() }
        .map_err(|e| vk_failure("enumerate_physical_devices", e))?;
    let physical = first_physical_device(&physical_devices)?;

    // SAFETY: `physical` は上の列挙で得た、この instance の物理 device。
    let properties = unsafe { native.instance.get_physical_device_properties(physical) };
    let name = properties.device_name_as_c_str().map_err(|_| {
        ContractError::DeviceUnavailable(
            "get_physical_device_properties: device name is not NUL-terminated".into(),
        )
    })?;
    let adapter_name = adapter_name_of(
        &name.to_string_lossy(),
        properties.api_version,
        properties.driver_version,
    );
    let limits = LimitsTable {
        max_buffer_bytes: 0,
        max_dispatch_dims: properties.limits.max_compute_work_group_count,
        max_texture_size: 0,
    };

    // SAFETY: `physical` はこの instance の物理 device。
    let families = unsafe {
        native
            .instance
            .get_physical_device_queue_family_properties(physical)
    };
    let family = select_queue_family(&families, req.profile)?;

    // SAFETY: `physical` はこの instance の物理 device。
    let supported = unsafe { native.instance.get_physical_device_features(physical) };
    let features = to_vk_features(enabled_features(&supported));

    let priorities = [1.0_f32];
    let queue_infos = [vk::DeviceQueueCreateInfo::default()
        .queue_family_index(family)
        .queue_priorities(&priorities)];
    let device_info = vk::DeviceCreateInfo::default()
        .queue_create_infos(&queue_infos)
        .enabled_features(&features);
    // SAFETY: `device_info` と、それが指す `queue_infos`・`priorities`・`features` は呼び出しの間だけ
    // 生きる局所の値。`family` は上の読み取りで得た添字で、その家族の queue 数は 1 以上。層・拡張は無い。
    let device = unsafe { native.instance.create_device(physical, &device_info, None) }
        .map_err(|e| vk_failure("create_device", e))?;
    let device = native.device.insert(device);

    let pool_info = vk::CommandPoolCreateInfo::default().queue_family_index(family);
    // SAFETY: `device` は上で作った生きている device で、`family` はその device の作成に使った家族。
    let pool = unsafe { device.create_command_pool(&pool_info, None) }
        .map_err(|e| vk_failure("create_command_pool", e))?;
    native.command_pool = Some(pool);

    let id = __backend_next_device_id().ok_or_else(exhausted)?;
    let generation = take_next(&NEXT_GENERATION).ok_or_else(exhausted)?;
    Ok(VulkanDevice {
        id,
        generation,
        ownership: DeviceOwnership::ProductIndependent,
        adapter_name,
        limits,
        native,
    })
}

impl Capabilities for VulkanDevice {
    fn query(&self, _feature: FeatureId) -> CapabilityLevel {
        CapabilityLevel::Unsupported
    }

    fn limits(&self) -> &LimitsTable {
        &self.limits
    }

    fn query_ex(&self, _feature: FeatureId) -> CapabilityQuery {
        UNLISTED_FEATURE_QUERY
    }
}

impl Device for VulkanDevice {
    fn backend(&self) -> BackendKind {
        BACKEND
    }
    fn ownership(&self) -> &DeviceOwnership {
        &self.ownership
    }
    fn generation(&self) -> u64 {
        self.generation
    }
    fn memory(&self) -> &dyn MemoryOps {
        self
    }
    fn queue(&self) -> &dyn CommandQueue {
        self
    }
    fn diagnostics(&self) -> DeviceDiagnostics {
        DeviceDiagnostics {
            adapter_name: self.adapter_name.clone(),
            backend_name: BACKEND_NAME,
        }
    }
}

/// 操作が `Unsupported` であることを表す値。
fn unsupported(op: &'static str) -> ContractError {
    ContractError::Unsupported {
        op,
        backend: BACKEND,
    }
}

impl MemoryOps for VulkanDevice {
    fn allocate(&self, _byte_len: u64, _usage: BufferUsage) -> Result<BufferHandle, ContractError> {
        Err(unsupported("allocate"))
    }

    fn upload(
        &self,
        _dst: &BufferHandle,
        _dst_offset: u64,
        _bytes: &[u8],
    ) -> Result<(), ContractError> {
        Err(unsupported("upload"))
    }

    #[cfg(feature = "native-blocking-wait")]
    fn download_blocking(
        &self,
        _src: &BufferHandle,
        _src_offset: u64,
        _byte_len: u64,
    ) -> Result<Vec<u8>, ContractError> {
        Err(unsupported("download_blocking"))
    }

    fn download_async(
        &self,
        _src: &BufferHandle,
        _src_offset: u64,
        _byte_len: u64,
        callback: Box<dyn FnOnce(Result<Vec<u8>, ContractError>) + Send>,
    ) {
        callback(Err(unsupported("download_async")));
    }

    fn free(&self, _buffer: BufferHandle) {}
}

impl CommandQueue for VulkanDevice {
    /// 記録先の識別子を払い出す。投入を受け付けないので、開いている記録先の集合は持たない。
    fn create_encoder(&self) -> CommandEncoder {
        // 識別子が尽きたときの 0 は CUDA・Metal の背後実装と同じ扱い。この device はどの識別子も
        // 投入で受け付けない（`submit` は常に `Unsupported`）ので、0 を返しても誤って通る経路は無い。
        CommandEncoder::__backend_new(__backend_next_encoder_id().unwrap_or(0), None)
    }

    fn submit(
        &self,
        _commands: CommandEncoder,
        _wait: &[ImportedSemaphoreHandle],
    ) -> Result<SubmitToken, ContractError> {
        Err(unsupported("submit"))
    }

    fn submit_many(
        &self,
        _commands: Vec<CommandEncoder>,
        _after: &[SubmitToken],
        _wait: &[ImportedSemaphoreHandle],
    ) -> Result<SubmitToken, ContractError> {
        Err(unsupported("submit_many"))
    }

    /// この device は印を払い出さないので、どの印も `Lost`。
    fn poll(&self, _token: SubmitToken) -> CompletionState {
        CompletionState::Lost
    }

    fn on_complete(&self, _token: SubmitToken, callback: Box<dyn FnOnce(CompletionState) + Send>) {
        callback(CompletionState::Lost);
    }

    #[cfg(feature = "native-blocking-wait")]
    fn wait_blocking(
        &self,
        _token: SubmitToken,
        _timeout: Option<std::time::Duration>,
    ) -> Result<(), ContractError> {
        Err(unsupported("wait_blocking"))
    }

    fn dispatch_ex(
        &self,
        _encoder: &mut CommandEncoder,
        _pipeline: &PipelineHandle,
        _bindings: &[Binding<'_>],
        _consts: &[ConstantBinding<'_>],
        _dispatch_dims: [u32; 3],
        _threads_per_threadgroup: [u32; 3],
        _threadgroup_memory: &[ThreadgroupMemory],
    ) -> Result<(), ContractError> {
        Err(unsupported("dispatch_ex"))
    }

    /// `limits()` の `max_buffer_bytes` と同じ値を返す。
    fn max_buffer_bytes(&self) -> u64 {
        self.limits.max_buffer_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn family(flags: vk::QueueFlags, count: u32) -> vk::QueueFamilyProperties {
        vk::QueueFamilyProperties::default()
            .queue_flags(flags)
            .queue_count(count)
    }

    /// DEV-69 のポインタ。
    #[test]
    fn adapter_name_has_the_documented_shape() {
        assert_eq!(
            adapter_name_of("GPU", vk::make_api_version(0, 1, 3, 2), 0x1234_5678),
            "GPU (vulkan api=1.3.2 driver_version=0x12345678)"
        );
    }

    #[test]
    fn first_physical_device_rejects_an_empty_list() {
        let r = first_physical_device::<u8>(&[]);
        assert!(
            matches!(&r, Err(ContractError::DeviceUnavailable(m)) if m.starts_with("enumerate_physical_devices: "))
        );
    }

    #[test]
    fn first_physical_device_takes_the_first() {
        assert_eq!(first_physical_device(&[7_u8, 8, 9]).unwrap(), 7);
    }

    #[test]
    fn select_queue_family_compute_only_takes_a_compute_family() {
        let fams = [
            family(vk::QueueFlags::TRANSFER, 1),
            family(vk::QueueFlags::COMPUTE, 1),
        ];
        assert_eq!(select_queue_family(&fams, Profile::ComputeOnly).unwrap(), 1);
    }

    #[test]
    fn select_queue_family_graphics_skips_a_compute_only_family() {
        let fams = [
            family(vk::QueueFlags::COMPUTE, 1),
            family(vk::QueueFlags::COMPUTE | vk::QueueFlags::GRAPHICS, 1),
        ];
        assert_eq!(select_queue_family(&fams, Profile::Graphics).unwrap(), 1);
    }

    #[test]
    fn select_queue_family_skips_a_family_without_queues() {
        let fams = [
            family(vk::QueueFlags::COMPUTE, 0),
            family(vk::QueueFlags::COMPUTE, 2),
        ];
        assert_eq!(select_queue_family(&fams, Profile::ComputeOnly).unwrap(), 1);
    }

    #[test]
    fn select_queue_family_without_a_match_is_device_unavailable() {
        let fams = [family(vk::QueueFlags::TRANSFER, 1)];
        for profile in [Profile::ComputeOnly, Profile::Graphics, Profile::CpuOnly] {
            let r = select_queue_family(&fams, profile);
            assert!(matches!(&r, Err(ContractError::DeviceUnavailable(_))));
        }
        assert!(select_queue_family(&[], Profile::ComputeOnly).is_err());
    }

    #[test]
    fn enabled_features_follow_the_physical_support() {
        let on = vk::PhysicalDeviceFeatures {
            draw_indirect_first_instance: vk::TRUE,
            robust_buffer_access: vk::TRUE,
            ..Default::default()
        };
        let off = vk::PhysicalDeviceFeatures::default();
        assert!(enabled_features(&on).draw_indirect_first_instance);
        assert!(!enabled_features(&off).draw_indirect_first_instance);

        let written = to_vk_features(enabled_features(&on));
        assert_eq!(written.draw_indirect_first_instance, vk::TRUE);
        assert_eq!(written.robust_buffer_access, vk::FALSE);
        assert_eq!(written.geometry_shader, vk::FALSE);
        assert_eq!(written.shader_float64, vk::FALSE);
        let none = to_vk_features(enabled_features(&off));
        assert_eq!(none.draw_indirect_first_instance, vk::FALSE);
    }

    #[test]
    fn vk_failure_puts_the_call_first() {
        let e = vk_failure("create_device", vk::Result::ERROR_INITIALIZATION_FAILED);
        assert!(
            matches!(&e, ContractError::DeviceUnavailable(m) if m.starts_with("create_device: "))
        );
    }

    #[test]
    fn release_in_order_releases_children_first() {
        use std::cell::RefCell;
        let log = RefCell::new(Vec::new());
        let push = |name: &'static str| {
            let log = &log;
            move |_: &'static str| log.borrow_mut().push(name)
        };
        release_in_order(
            Some("p"),
            Some("d"),
            "i",
            push("pool"),
            push("device"),
            push("instance"),
        );
        assert_eq!(*log.borrow(), ["pool", "device", "instance"]);

        log.borrow_mut().clear();
        release_in_order(
            None::<&str>,
            Some("d"),
            "i",
            push("pool"),
            push("device"),
            push("instance"),
        );
        assert_eq!(*log.borrow(), ["device", "instance"]);

        log.borrow_mut().clear();
        release_in_order(
            None::<&str>,
            None::<&str>,
            "i",
            push("pool"),
            push("device"),
            push("instance"),
        );
        assert_eq!(*log.borrow(), ["instance"]);
    }

    #[test]
    fn absent_loader_is_device_unavailable() {
        let req = DeviceRequest {
            backend: BackendKind::Vulkan,
            profile: Profile::ComputeOnly,
            ownership: DeviceOwnership::ProductIndependent,
        };
        match load_entry() {
            Ok(_) => println!("not-run: Vulkan loader is present"),
            Err(e) => {
                assert!(
                    matches!(&e, ContractError::DeviceUnavailable(m) if m.starts_with("Entry::load:"))
                );
                let r = VulkanDeviceProvider::request(req);
                assert!(matches!(r, Err(ContractError::DeviceUnavailable(_))));
            }
        }
    }
}
