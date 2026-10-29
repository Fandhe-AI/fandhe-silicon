//! Metal の背後実装（下-1）。`backend-metal` feature で有効になる。
//!
//! この背後実装の `unsafe` / FFI はこのモジュールに閉じ込め、外へは safe な API だけを出す。
//! macOS のときだけ、既定の device の取得（`DeviceProvider::request`）・診断・バッファの確保と読み書きを持つ
//! （TASK-158、DEV-1・DEV-16・DEV-67・DEV-68・DEV-206・DEV-210・DEV-211・TASK-265 のポインタ）。
//! macOS 以外（Linux・wasm32）では、feature を付けてもこのモジュールは空のままで、公開の項目を持たない。
//! Objective-C のバインディングは `objc2` 系の 4 crate（macOS 向けにだけ宣言。承認の記録は `deps-approved.toml`）。
//!
//! 公開するのは `MetalDeviceProvider` だけ。device の型 `MetalDevice` は非公開で、上・下-2 は
//! `request` が返す `Box<dyn Device>` 越しに使う。
//!
//! 投入の列・compute の口・ロストの通知はこの段では持たない（コマンドバッファを積まない）。
//! したがって device のメモリへの CPU からの読み書きと GPU の読み書きが重なることはない。
//! 読み書きの `unsafe` ブロックの同期の根拠は、この前提に頼る。
//! 投入（コマンドバッファ）を足す変更では、これらのブロックの同期の根拠を必ず書き直す。

#[cfg(target_os = "macos")]
mod imp {
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Mutex, MutexGuard};

    use fandhe_silicon_core::capability::{
        BackendKind, Capabilities, CapabilityLevel, CapabilityQuery, FeatureId, LimitsTable,
    };
    use fandhe_silicon_core::device::{
        Device, DeviceDiagnostics, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
    };
    use fandhe_silicon_core::error::{ContractError, UnsupportedReason};
    use fandhe_silicon_core::memory::{
        BufferHandle, BufferUsage, MemoryOps, validate_allocation_size, validate_buffer_range,
    };
    use objc2::rc::Retained;
    use objc2::runtime::ProtocolObject;
    // コマンドラインのプログラムで `MTLCreateSystemDefaultDevice` が既定の device を返すには、
    // Core Graphics がリンクされている必要がある。参照を置いてリンクを確実にする（シンボルは使わない）。
    use objc2_core_graphics as _;
    use objc2_metal::{
        MTLBuffer, MTLCommandQueue, MTLCreateSystemDefaultDevice, MTLDevice, MTLResourceOptions,
    };

    /// この背後実装の種類。エラーの `backend` と `Device::backend` の値。
    const BACKEND: BackendKind = BackendKind::Metal;

    /// 診断の `backend_name`。
    const BACKEND_NAME: &str = "metal";

    /// Metal が扱うテクスチャの 1 辺の上限として `LimitsTable` に載せる値。
    const MAX_TEXTURE_SIZE: u32 = 16384;

    /// プロセスの中で device ごとに別の値を払い出す識別子の源。0 は払い出さない。
    static NEXT_DEVICE_ID: AtomicU64 = AtomicU64::new(1);

    /// この背後実装のプロセスの中で、device を作るごとに 1 つ増える世代の源。1 から始まる。
    static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

    /// 源から次の値を取る。あふれたら `None` を返し、源は動かさない。
    fn take_next(counter: &AtomicU64) -> Option<u64> {
        // toolchain の版に依らず警告の出ない、比較交換のループで払い出す。
        let mut current = counter.load(Ordering::Relaxed);
        loop {
            let next = current.checked_add(1)?;
            match counter.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(_) => return Some(current),
                Err(actual) => current = actual,
            }
        }
    }

    /// `request` の中の呼び出しの結果を `None` に差し替える指示。
    ///
    /// 本物の `request` は `Faults::default()`（何も差し替えない）で同じ関数を通る。
    /// feature `fault-injection` の提供型だけが、どちらかを `true` にして呼ぶ。
    #[derive(Clone, Copy, Default)]
    struct Faults {
        /// `MTLCreateSystemDefaultDevice` を呼ばずに、結果を `None` として扱う。
        default_device_none: bool,
        /// `newCommandQueue` の結果を `None` として扱う（device は本物を取る）。
        command_queue_none: bool,
    }

    /// 要求の組み合わせを照らし、device を作って返す。本物の `request` と差し込みの型が共有する。
    ///
    /// 照らす順は、背後実装、プロファイル（`Graphics` の feature、`CpuOnly`、未知の値）、所有形態。
    /// 最初に当たったものを返し、そのあいだは Metal の呼び出しをしない。
    fn request_with(req: DeviceRequest, faults: Faults) -> Result<Box<dyn Device>, ContractError> {
        if req.backend != BACKEND {
            return Err(ContractError::InvalidCombination(format!(
                "Metal provider cannot serve backend {:?}",
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
            DeviceOwnership::ProductIndependent => {}
            DeviceOwnership::Shared => {
                return Err(ContractError::Unsupported {
                    op: "request(DeviceOwnership::Shared)",
                    backend: BACKEND,
                });
            }
            DeviceOwnership::Imported { .. } => {
                return Err(ContractError::Unsupported {
                    op: "request(DeviceOwnership::Imported)",
                    backend: BACKEND,
                });
            }
            _ => {
                return Err(ContractError::InvalidCombination(
                    "unknown device ownership".into(),
                ));
            }
        }

        let injected = if faults.default_device_none {
            " (injected)"
        } else {
            ""
        };
        let device = if faults.default_device_none {
            None
        } else {
            MTLCreateSystemDefaultDevice()
        }
        .ok_or_else(|| {
            ContractError::DeviceUnavailable(format!(
                "MTLCreateSystemDefaultDevice returned None{injected}"
            ))
        })?;

        let queue_injected = if faults.command_queue_none {
            " (injected)"
        } else {
            ""
        };
        let queue = if faults.command_queue_none {
            None
        } else {
            device.newCommandQueue()
        }
        .ok_or_else(|| {
            ContractError::DeviceUnavailable(format!(
                "newCommandQueue returned None{queue_injected}"
            ))
        })?;

        let exhausted =
            || ContractError::DeviceUnavailable("device counter space is exhausted".into());
        let id = take_next(&NEXT_DEVICE_ID).ok_or_else(exhausted)?;
        let generation = take_next(&NEXT_GENERATION).ok_or_else(exhausted)?;

        let limits = LimitsTable {
            max_buffer_bytes: u64::try_from(device.maxBufferLength()).unwrap_or(u64::MAX),
            max_dispatch_dims: [u32::MAX; 3],
            max_texture_size: MAX_TEXTURE_SIZE,
        };
        let adapter_name = device.name().to_string();

        Ok(Box::new(MetalDevice {
            id,
            generation,
            ownership: DeviceOwnership::ProductIndependent,
            adapter_name,
            limits,
            device,
            _queue: queue,
            buffers: Mutex::new(Buffers {
                next_id: 1,
                map: HashMap::new(),
            }),
        }))
    }

    /// Metal の背後実装で device を作る入口。
    ///
    /// `request` は `Metal` の背後実装・`ComputeOnly`（`graphics` feature が有効なら `Graphics` も）・
    /// `ProductIndependent` の組だけを受ける。`ProductIndependent` では、呼ぶたびに新しい device と
    /// コマンドキューを作り、共有しない。
    #[derive(Debug, Clone, Copy, Default)]
    pub struct MetalDeviceProvider;

    impl DeviceProvider for MetalDeviceProvider {
        /// 要求を照らしてから Metal の既定の device を取り、device を作る。
        ///
        /// - 背後実装が Metal でない、`CpuOnly`、`graphics` が無効での `Graphics`:
        ///   `InvalidCombination`（device を作る前）。
        /// - `Shared`・`Imported`: `Unsupported`（device を作る前）。
        /// - 既定の device またはコマンドキューが得られない: `DeviceUnavailable`。
        ///
        /// 別の背後実装へは切り替えない。
        fn request(req: DeviceRequest) -> Result<Box<dyn Device>, ContractError> {
            request_with(req, Faults::default())
        }
    }

    /// 既定の device の取得が `None` を返した場合を作る差し込み用の入口。実際の故障ではない。
    ///
    /// 要求の検査は `MetalDeviceProvider` と同じで、検査を通ったら `MTLCreateSystemDefaultDevice` を
    /// 呼ばずに、その結果を `None` として扱い、`DeviceUnavailable` を返す（device に触らない）。
    /// 初期化失敗の経路を、実機の故障なしで通すための試験・診断用で、feature `fault-injection` の裏にある。
    #[cfg(feature = "fault-injection")]
    #[derive(Debug, Clone, Copy, Default)]
    pub struct MetalDefaultDeviceNoneProvider;

    #[cfg(feature = "fault-injection")]
    impl DeviceProvider for MetalDefaultDeviceNoneProvider {
        /// 検査を通ったら、既定の device の取得の結果を `None` に差し替えて `DeviceUnavailable` を返す。
        fn request(req: DeviceRequest) -> Result<Box<dyn Device>, ContractError> {
            request_with(
                req,
                Faults {
                    default_device_none: true,
                    ..Faults::default()
                },
            )
        }
    }

    /// コマンドキューの作成が `None` を返した場合を作る差し込み用の入口。実際の故障ではない。
    ///
    /// 要求の検査は `MetalDeviceProvider` と同じで、検査を通ったら本物の既定の device を取り、
    /// `newCommandQueue` の結果を `None` として扱い、`DeviceUnavailable` を返す（device に触る）。
    /// 初期化失敗の経路を通すための試験・診断用で、feature `fault-injection` の裏にある。
    #[cfg(feature = "fault-injection")]
    #[derive(Debug, Clone, Copy, Default)]
    pub struct MetalCommandQueueNoneProvider;

    #[cfg(feature = "fault-injection")]
    impl DeviceProvider for MetalCommandQueueNoneProvider {
        /// 検査を通ったら、コマンドキューの作成の結果を `None` に差し替えて `DeviceUnavailable` を返す。
        fn request(req: DeviceRequest) -> Result<Box<dyn Device>, ContractError> {
            request_with(
                req,
                Faults {
                    command_queue_none: true,
                    ..Faults::default()
                },
            )
        }
    }

    /// device が持つバッファの 1 本。
    struct BufferEntry {
        /// Metal のバッファ。`free` で表から外すまで device が保持する。
        buffer: Retained<ProtocolObject<dyn MTLBuffer>>,
    }

    /// device のバッファの表。識別子は device ごとに 1 から払い出す。
    struct Buffers {
        /// 次に払い出すバッファの識別子。
        next_id: u64,
        /// 識別子からバッファへの表。
        map: HashMap<u64, BufferEntry>,
    }

    /// Metal の device 1 つ。`DeviceProvider::request` が作る。
    ///
    /// device・コマンドキュー・バッファは、この値が持つ（Drop で解放される）。
    /// バッファは shared の storage で確保し、読み書きは CPU からの `contents()` 越しのコピーで行う。
    struct MetalDevice {
        /// この背後実装の中で device ごとに別の識別子。ハンドルの払い出し元の照合に使う。
        id: u64,
        /// device を作るときに取った世代。
        generation: u64,
        /// 作るときに指定された所有形態。
        ownership: DeviceOwnership,
        /// `MTLDevice::name` の文字列。
        adapter_name: String,
        /// 作るときに計算した上限の表。
        limits: LimitsTable,
        /// 既定の device。
        device: Retained<ProtocolObject<dyn MTLDevice>>,
        /// コマンドキュー。この段では投入に使わず、device の生存の間だけ持つ。
        _queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
        /// バッファの表。
        buffers: Mutex<Buffers>,
    }

    impl MetalDevice {
        /// 表の錠を取る。他のスレッドが panic して錠が汚れていても、表は壊れない更新だけなので中身を使う。
        fn lock_buffers(&self) -> MutexGuard<'_, Buffers> {
            self.buffers.lock().unwrap_or_else(|e| e.into_inner())
        }

        /// ハンドルを照合し、この device のバッファに対して `f` を呼ぶ。`f` の間は表の錠を持つ。
        ///
        /// 照合の順は、払い出し元の device の識別子（違えば `DeviceMismatch`）、表にある識別子、
        /// 世代の一致（表に無い・世代が違うものは偽造として `InvalidArgument`）。
        fn with_buffer<R>(
            &self,
            handle: &BufferHandle,
            f: impl FnOnce(&ProtocolObject<dyn MTLBuffer>) -> Result<R, ContractError>,
        ) -> Result<R, ContractError> {
            if handle.__backend_device_id() != self.id {
                return Err(ContractError::DeviceMismatch);
            }
            let buffers = self.lock_buffers();
            let Some(entry) = buffers.map.get(&handle.__backend_id()) else {
                return Err(ContractError::InvalidArgument(
                    "buffer handle is not in this device's table".into(),
                ));
            };
            if handle.__backend_generation() != self.generation {
                return Err(ContractError::InvalidArgument(
                    "buffer handle generation does not match this device".into(),
                ));
            }
            f(&entry.buffer)
        }

        /// 範囲を検査してから `[offset, offset + len)` を読み出す。`download_*` が共有する。
        fn read_range(
            &self,
            src: &BufferHandle,
            src_offset: u64,
            byte_len: u64,
        ) -> Result<Vec<u8>, ContractError> {
            self.with_buffer(src, |buffer| {
                let allocation_len = u64::try_from(buffer.length()).map_err(|_| {
                    ContractError::InvalidArgument("buffer length does not fit u64".into())
                })?;
                validate_buffer_range(src_offset, byte_len, allocation_len)?;
                // 検査を通ったので、offset と len は確保長以下で usize に収まる。
                let (Ok(offset), Ok(len)) =
                    (usize::try_from(src_offset), usize::try_from(byte_len))
                else {
                    return Err(ContractError::InvalidArgument(
                        "buffer range does not fit usize".into(),
                    ));
                };
                // 確保できなければ abort せず `AllocationFailed` で返す。
                let mut out: Vec<u8> = Vec::new();
                out.try_reserve_exact(len)
                    .map_err(|_| ContractError::AllocationFailed {
                        requested_bytes: byte_len,
                    })?;
                out.resize(len, 0);
                if len > 0 {
                    let base = buffer.contents().as_ptr().cast::<u8>().cast_const();
                    // SAFETY: バッファは shared の storage なので `contents()` は CPU から読める
                    // `buffer.length()` バイトの領域を指す。`validate_buffer_range` が
                    // `offset + len <= length()` を保証している。GPU の書き込みと重ならないことは、
                    // この背後実装がコマンドバッファを投入しないことに根拠を置く（投入を足す変更では、
                    // このブロックの同期の根拠を必ず書き直す）。`with_buffer` が表の錠を持ったまま
                    // `f` を呼ぶため、コピーの間は `free` でバッファが落ちない。`out` は新しく確保した
                    // `len` バイトの領域で、Metal の領域とは重ならない。
                    unsafe {
                        core::ptr::copy_nonoverlapping(base.add(offset), out.as_mut_ptr(), len);
                    }
                }
                Ok(out)
            })
        }
    }

    impl Capabilities for MetalDevice {
        /// この段の `query` は、どの機能にも `Unsupported` を返す。
        fn query(&self, _feature: FeatureId) -> CapabilityLevel {
            CapabilityLevel::Unsupported
        }

        fn limits(&self) -> &LimitsTable {
            &self.limits
        }

        /// どの機能も未知として、理由 `UnknownFeature` を添えて `Unsupported` を返す。
        fn query_ex(&self, _feature: FeatureId) -> CapabilityQuery {
            CapabilityQuery {
                level: CapabilityLevel::Unsupported,
                unsupported_reason: Some(UnsupportedReason::UnknownFeature),
                evidence: None,
            }
        }
    }

    impl Device for MetalDevice {
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

        fn diagnostics(&self) -> DeviceDiagnostics {
            DeviceDiagnostics {
                adapter_name: self.adapter_name.clone(),
                backend_name: BACKEND_NAME,
            }
        }
    }

    /// `allocate` が Metal のバッファを作る前に通す検査。
    ///
    /// 共通の `validate_allocation_size` は上限だけを見て 0 を通す。長さ 0 を Metal に渡したときの
    /// 振る舞いには頼らず、呼び出し側の引数の誤りとして、この背後実装では 0 を先に
    /// `InvalidArgument` で拒む。
    fn validate_allocation_request(
        byte_len: u64,
        max_buffer_bytes: u64,
    ) -> Result<(), ContractError> {
        if byte_len == 0 {
            return Err(ContractError::InvalidArgument(
                "allocation size must be greater than zero".into(),
            ));
        }
        validate_allocation_size(byte_len, max_buffer_bytes)
    }

    impl MemoryOps for MetalDevice {
        /// shared の storage でバッファを確保する。
        ///
        /// 長さ 0 と上限（`limits().max_buffer_bytes`）超えは `InvalidArgument`。
        /// 返すハンドルの `size`・`usage` は、受けた `byte_len`・`usage`。
        fn allocate(
            &self,
            byte_len: u64,
            usage: BufferUsage,
        ) -> Result<BufferHandle, ContractError> {
            validate_allocation_request(byte_len, self.limits.max_buffer_bytes)?;
            let failed = || ContractError::AllocationFailed {
                requested_bytes: byte_len,
            };
            let length = usize::try_from(byte_len).map_err(|_| failed())?;
            let buffer = self
                .device
                .newBufferWithLength_options(length, MTLResourceOptions::StorageModeShared)
                .ok_or_else(failed)?;
            let mut buffers = self.lock_buffers();
            let id = buffers.next_id;
            buffers.next_id = id.checked_add(1).ok_or_else(failed)?;
            buffers.map.insert(id, BufferEntry { buffer });
            Ok(BufferHandle::__backend_new(
                self.generation,
                id,
                self.id,
                byte_len,
                usage,
            ))
        }

        /// `bytes` を `dst` の `dst_offset` から書き込む。範囲の検査に当たると、バッファの内容を変えずに返す。
        fn upload(
            &self,
            dst: &BufferHandle,
            dst_offset: u64,
            bytes: &[u8],
        ) -> Result<(), ContractError> {
            self.with_buffer(dst, |buffer| {
                let len = u64::try_from(bytes.len()).map_err(|_| {
                    ContractError::InvalidArgument("data length does not fit u64".into())
                })?;
                let allocation_len = u64::try_from(buffer.length()).map_err(|_| {
                    ContractError::InvalidArgument("buffer length does not fit u64".into())
                })?;
                validate_buffer_range(dst_offset, len, allocation_len)?;
                let Ok(offset) = usize::try_from(dst_offset) else {
                    return Err(ContractError::InvalidArgument(
                        "buffer offset does not fit usize".into(),
                    ));
                };
                if !bytes.is_empty() {
                    let base = buffer.contents().as_ptr().cast::<u8>();
                    // SAFETY: バッファは shared の storage なので `contents()` は CPU から書ける
                    // `buffer.length()` バイトの領域を指す。`validate_buffer_range` が
                    // `offset + bytes.len() <= length()` を保証している。GPU の読み書きと重ならないことは、
                    // この背後実装がコマンドバッファを投入しないことに根拠を置く（投入を足す変更では、
                    // このブロックの同期の根拠を必ず書き直す）。`with_buffer` が表の錠を持ったまま
                    // `f` を呼ぶため、コピーの間は `free` でバッファが落ちない。`bytes` は呼び出し側の
                    // 借用した領域で、Metal の領域とは重ならない。
                    unsafe {
                        core::ptr::copy_nonoverlapping(
                            bytes.as_ptr(),
                            base.add(offset),
                            bytes.len(),
                        );
                    }
                }
                Ok(())
            })
        }

        /// `src` の `src_offset` から `byte_len` バイトを読み出す。GPU の投入が無いので、待たずに返る。
        #[cfg(feature = "native-blocking-wait")]
        fn download_blocking(
            &self,
            src: &BufferHandle,
            src_offset: u64,
            byte_len: u64,
        ) -> Result<Vec<u8>, ContractError> {
            self.read_range(src, src_offset, byte_len)
        }

        /// 読み出してから、その場で `callback` を 1 回だけ呼ぶ。失敗も `callback` に渡す。
        fn download_async(
            &self,
            src: &BufferHandle,
            src_offset: u64,
            byte_len: u64,
            callback: Box<dyn FnOnce(Result<Vec<u8>, ContractError>) + Send>,
        ) {
            callback(self.read_range(src, src_offset, byte_len));
        }

        /// この device のハンドルなら、表から外してバッファを解放する。
        ///
        /// 別の device のハンドルは何も解放しない（同じ識別子を持つ自 device のバッファを巻き込まない）。
        fn free(&self, buffer: BufferHandle) {
            if buffer.__backend_device_id() != self.id
                || buffer.__backend_generation() != self.generation
            {
                return;
            }
            let removed = self.lock_buffers().map.remove(&buffer.__backend_id());
            // 錠を放してから落とす。Metal のバッファの解放を錠の中で行わない。
            drop(removed);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// `allocate` の前段の検査の境界。device には触らない。
        const LIMIT: u64 = 1024;

        #[test]
        fn zero_length_is_invalid_argument() {
            let r = validate_allocation_request(0, LIMIT);
            assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
        }

        #[test]
        fn zero_length_is_rejected_even_when_the_limit_is_zero() {
            let r = validate_allocation_request(0, 0);
            assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
        }

        #[test]
        fn one_byte_and_the_limit_itself_pass() {
            assert!(validate_allocation_request(1, LIMIT).is_ok());
            assert!(validate_allocation_request(LIMIT, LIMIT).is_ok());
        }

        #[test]
        fn one_past_the_limit_is_invalid_argument() {
            let r = validate_allocation_request(LIMIT + 1, LIMIT);
            assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
        }
    }
}

#[cfg(target_os = "macos")]
pub use imp::MetalDeviceProvider;

#[cfg(all(target_os = "macos", feature = "fault-injection"))]
pub use imp::{MetalCommandQueueNoneProvider, MetalDefaultDeviceNoneProvider};
