//! Metal の device の `pipeline_cache_key` で、function constant の値の変更と版の欄の材料を確かめる（TASK-502 のポインタ）。
//!
//! どの試験も Metal の device に触るので `#[ignore]` を付ける。macOS 以外ではこのファイルは 0 件になる。

#![cfg(target_os = "macos")]

use fandhe_silicon_contract::backend_metal::MetalDeviceProvider;
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::execution::{
    CacheKey, FunctionConstant, FunctionConstantValue, MslFloatFunctions, MslMathMode,
    PipelineInput,
};
use fandhe_silicon_core::hash::sha256_u64;
use objc2_foundation::NSProcessInfo;
use objc2_metal::{MTLCreateSystemDefaultDevice, MTLDevice};

const KERNEL: &str = "#include <metal_stdlib>\n\
    using namespace metal;\n\
    constant uint N [[function_constant(0)]];\n\
    kernel void k(device uint* out [[buffer(0)]]) { out[0] = N; }\n";

fn new_device() -> Box<dyn Device> {
    match MetalDeviceProvider::request(DeviceRequest {
        backend: BackendKind::Metal,
        profile: Profile::ComputeOnly,
        ownership: DeviceOwnership::ProductIndependent,
    }) {
        Ok(d) => d,
        Err(e) => panic!("request failed: {e:?}"),
    }
}

/// function constant の値だけを変えると、`specialization_hash` だけが変わる（ART-39 のポインタ）。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn device_key_changes_only_the_specialization_hash_when_a_constant_value_changes() {
    let device = new_device();
    let compute = device
        .compute()
        .expect("a Metal device must expose compute()");
    let four = [FunctionConstant {
        index: 0,
        value: FunctionConstantValue::U32(4),
    }];
    let five = [FunctionConstant {
        index: 0,
        value: FunctionConstantValue::U32(5),
    }];
    let pairs = [
        (
            PipelineInput::MslSpecialized {
                source: KERNEL,
                entry_point: "k",
                function_constants: &four,
            },
            PipelineInput::MslSpecialized {
                source: KERNEL,
                entry_point: "k",
                function_constants: &five,
            },
        ),
        (
            PipelineInput::MslSpecializedWithOptions {
                source: KERNEL,
                entry_point: "k",
                function_constants: &four,
                math_mode: MslMathMode::Safe,
                float_functions: MslFloatFunctions::Precise,
            },
            PipelineInput::MslSpecializedWithOptions {
                source: KERNEL,
                entry_point: "k",
                function_constants: &five,
                math_mode: MslMathMode::Safe,
                float_functions: MslFloatFunctions::Precise,
            },
        ),
    ];
    for (a, b) in &pairs {
        let ka = compute.pipeline_cache_key(a, false).expect("a");
        let kb = compute.pipeline_cache_key(b, false).expect("b");
        assert_ne!(ka.specialization_hash, kb.specialization_hash, "{a:?}");
        assert_eq!(
            CacheKey {
                specialization_hash: ka.specialization_hash,
                ..kb
            },
            ka,
            "{a:?}"
        );
    }
}

/// 版の 2 つの欄は、2 回呼んで同じで、OS の版の文字列と device 名を NUL でつないだ文字列の `sha256_u64`。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn version_fields_are_stable_and_come_from_the_os_version_and_the_device_name() {
    let device = new_device();
    let compute = device
        .compute()
        .expect("a Metal device must expose compute()");
    let os_version = NSProcessInfo::processInfo()
        .operatingSystemVersionString()
        .to_string();
    let device_name = MTLCreateSystemDefaultDevice()
        .expect("the default Metal device must exist in this session")
        .name()
        .to_string();
    let expected = sha256_u64(format!("{os_version}\0{device_name}").as_bytes());
    let constants = [FunctionConstant {
        index: 0,
        value: FunctionConstantValue::U32(4),
    }];
    let inputs = [
        PipelineInput::Msl {
            source: KERNEL,
            entry_point: "k",
        },
        PipelineInput::MslSpecialized {
            source: KERNEL,
            entry_point: "k",
            function_constants: &constants,
        },
        PipelineInput::MslWithOptions {
            source: KERNEL,
            entry_point: "k",
            math_mode: MslMathMode::Safe,
        },
        PipelineInput::MslSpecializedWithOptions {
            source: KERNEL,
            entry_point: "k",
            function_constants: &constants,
            math_mode: MslMathMode::Fast,
            float_functions: MslFloatFunctions::Fast,
        },
    ];
    for input in &inputs {
        let a = compute.pipeline_cache_key(input, false).expect("a");
        let b = compute.pipeline_cache_key(input, false).expect("b");
        assert_eq!(a, b, "{input:?}");
        assert_eq!(a.compiler_version, expected, "{input:?}");
        assert_eq!(a.driver_version, Some(expected), "{input:?}");
    }
}
