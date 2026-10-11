//! backend-* feature の cfg ゲートが効き、対応するモジュールが公開されることの確認。
//! 骨格段階のため、モジュールのパスが解決できること（コンパイルが通ること）を確かめる。

#[cfg(feature = "backend-metal")]
#[test]
fn backend_metal_module_is_visible() {
    #[allow(unused_imports)]
    use fandhe_silicon_contract::backend_metal;
}

#[cfg(all(feature = "backend-vulkan", not(target_arch = "wasm32")))]
#[test]
fn backend_vulkan_module_is_visible() {
    #[allow(unused_imports)]
    use fandhe_silicon_contract::backend_vulkan;
}

#[cfg(all(feature = "backend-cuda", not(target_arch = "wasm32")))]
#[test]
fn backend_cuda_module_is_visible() {
    #[allow(unused_imports)]
    use fandhe_silicon_contract::backend_cuda;
}
