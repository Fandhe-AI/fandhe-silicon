//! `ExternalInterop` の拡張 4 操作の返しを、上書きの有無ごとに別の crate から固定する公開テスト（TASK-276・TASK-718）。
//!
//! `interop_types.rs` の既定値の確認を、上書きの有無で対にして補う。
//! 試験用の型はこのファイルの中だけにあり、公開 API ではない。機材には触れず、実際の背後実装の返しは含まない。
//! `RequiredOnly` は必須の 4 操作だけを実装した基盤の外の利用者の型（上書きなし）、
//! `Overriding` は拡張 4 操作を上書きして `Unsupported { op, backend }` を返す試験用の型。

use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::execution::{ImportedSemaphoreHandle, SubmitToken};
use fandhe_silicon_core::interop::{
    ExternalImageDescriptor, ExternalImageFormat, ExternalImageHandle, ExternalImageLayout,
    ExternalImageTiling, ExternalInterop, ExternalMemoryHandle, ExternalSemaphoreHandle,
    ImportedImageHandle,
};
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};

// 拡張 4 操作の名前。`op` の期待値に使う。
const EXPORT_IMAGE: &str = "export_image";
const IMPORT_IMAGE: &str = "import_image";
const EXPORT_TIMELINE: &str = "export_semaphore_timeline";
const IMPORT_TIMELINE: &str = "import_semaphore_timeline";

/// 拡張 4 操作の名前を呼ぶ順に並べた期待値。
const EXTENSION_OPS: [&str; 4] = [EXPORT_IMAGE, IMPORT_IMAGE, EXPORT_TIMELINE, IMPORT_TIMELINE];

/// 必須の 4 操作の穴埋め。呼ばれない。値に意味はない。
fn unused() -> ContractError {
    ContractError::Unsupported {
        op: "unused",
        backend: BackendKind::Vulkan,
    }
}

/// 必須の 4 操作を `unused()` で埋める本体を、`impl ExternalInterop` の中へ展開する。
macro_rules! required_ops {
    () => {
        fn export_memory(&self, _: &BufferHandle) -> Result<ExternalMemoryHandle, ContractError> {
            Err(unused())
        }

        fn import_memory(
            &self,
            _: ExternalMemoryHandle,
            _: u64,
            _: BufferUsage,
        ) -> Result<BufferHandle, ContractError> {
            Err(unused())
        }

        fn export_semaphore(
            &self,
            _: SubmitToken,
        ) -> Result<ExternalSemaphoreHandle, ContractError> {
            Err(unused())
        }

        fn import_semaphore(
            &self,
            _: ExternalSemaphoreHandle,
        ) -> Result<ImportedSemaphoreHandle, ContractError> {
            Err(unused())
        }
    };
}

/// 必須の 4 操作だけを実装する、基盤の外の利用者の型。
struct RequiredOnly;

impl ExternalInterop for RequiredOnly {
    required_ops!();
}

/// 契約の型の返し方を確かめるための試験用の型。実際の背後実装の返しとは無関係。
/// 拡張 4 操作を上書きして `Unsupported { op, backend }` を返す。
struct Overriding {
    backend: BackendKind,
}

impl ExternalInterop for Overriding {
    required_ops!();

    fn export_image(
        &self,
        _: ExternalImageDescriptor,
    ) -> Result<ExternalImageHandle, ContractError> {
        Err(ContractError::Unsupported {
            op: EXPORT_IMAGE,
            backend: self.backend,
        })
    }

    fn import_image(
        &self,
        _: ExternalImageHandle,
        _: ExternalImageDescriptor,
    ) -> Result<ImportedImageHandle, ContractError> {
        Err(ContractError::Unsupported {
            op: IMPORT_IMAGE,
            backend: self.backend,
        })
    }

    fn export_semaphore_timeline(
        &self,
        _: SubmitToken,
    ) -> Result<(ExternalSemaphoreHandle, u64), ContractError> {
        Err(ContractError::Unsupported {
            op: EXPORT_TIMELINE,
            backend: self.backend,
        })
    }

    fn import_semaphore_timeline(
        &self,
        _: ExternalSemaphoreHandle,
        _: u64,
    ) -> Result<ImportedSemaphoreHandle, ContractError> {
        Err(ContractError::Unsupported {
            op: IMPORT_TIMELINE,
            backend: self.backend,
        })
    }
}

fn descriptor() -> ExternalImageDescriptor {
    ExternalImageDescriptor {
        format: ExternalImageFormat::Rgba8Unorm,
        extent: [2, 2, 1],
        tiling: ExternalImageTiling::Optimal,
        initial_layout: ExternalImageLayout::Undefined,
        dedicated: None,
    }
}

fn token() -> SubmitToken {
    SubmitToken::__backend_new(1, 1, 1)
}

/// 拡張 4 操作を、同じ引数で `dyn` 越しに呼んで各操作のエラーを順に返す。
fn call_extension_ops(interop: &dyn ExternalInterop) -> [ContractError; 4] {
    [
        interop.export_image(descriptor()).unwrap_err(),
        interop
            .import_image(ExternalImageHandle::__backend_new(1, false), descriptor())
            .unwrap_err(),
        interop.export_semaphore_timeline(token()).unwrap_err(),
        interop
            .import_semaphore_timeline(ExternalSemaphoreHandle::__backend_new(1, true), 1)
            .unwrap_err(),
    ]
}

/// ERR-17: 上書きしない型では、4 操作それぞれが `UnsupportedExtensionDefault { op: <操作の名前> }` を返す。
#[test]
fn non_overriding_type_returns_default_with_each_op_name() {
    let expected = EXTENSION_OPS.map(|op| ContractError::UnsupportedExtensionDefault { op });
    assert_eq!(call_extension_ops(&RequiredOnly), expected);
}

/// ERR-17: 上書きした型では、4 操作それぞれが `Unsupported { op, backend }` を返す。
#[test]
fn overriding_type_returns_unsupported_with_backend() {
    for backend in [BackendKind::Vulkan, BackendKind::Cuda] {
        let expected = EXTENSION_OPS.map(|op| ContractError::Unsupported { op, backend });
        assert_eq!(call_extension_ops(&Overriding { backend }), expected);
    }
}

/// ERR-17: 一部だけ上書きした型では、上書きした操作だけが `Unsupported` を返し、残りは既定の値のまま。
#[test]
fn partial_override_changes_only_overridden_op() {
    struct OnlyExportImage;
    impl ExternalInterop for OnlyExportImage {
        required_ops!();

        fn export_image(
            &self,
            _: ExternalImageDescriptor,
        ) -> Result<ExternalImageHandle, ContractError> {
            Err(ContractError::Unsupported {
                op: EXPORT_IMAGE,
                backend: BackendKind::Cuda,
            })
        }
    }

    let mut expected = EXTENSION_OPS.map(|op| ContractError::UnsupportedExtensionDefault { op });
    expected[0] = ContractError::Unsupported {
        op: EXPORT_IMAGE,
        backend: BackendKind::Cuda,
    };
    assert_eq!(call_extension_ops(&OnlyExportImage), expected);
}

/// INTEROP-43: 必須の 4 操作だけの外の利用者の型で、拡張 4 操作が `Box<dyn ExternalInterop>` 越しでも既定の値を返す。
/// コンパイルが通ること自体が、必須の 4 操作の実装だけで足りることの確かめになる。
#[test]
fn required_only_type_gets_default_through_boxed_dyn() {
    let boxed: Box<dyn ExternalInterop> = Box::new(RequiredOnly);
    let expected = EXTENSION_OPS.map(|op| ContractError::UnsupportedExtensionDefault { op });
    assert_eq!(call_extension_ops(boxed.as_ref()), expected);
}
