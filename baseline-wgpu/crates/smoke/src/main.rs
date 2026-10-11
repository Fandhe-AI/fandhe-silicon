//! 比較基準の別 workspace（`baseline-wgpu/`）が wgpu をリンクして単独でビルド・実行できることを確かめる疎通確認用 bin。
//!
//! 基盤の workspace からは呼ばれない。`wgpu::Instance` を作って全 backend のアダプタを列挙し、
//! 名前・backend・device type を標準出力へ出して終了する。GPU やドライバが無くアダプタが 0 件でも
//! panic せず "0 adapter(s)" を出して正常終了する。非同期 API は外部 executor を使わず、
//! std の noop な `Waker` による busy-poll で待つ（依存は wgpu のみに保つため）。

use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

/// `Future` を noop の `Waker` で完了まで poll し続けて結果を返す。
///
/// `main` から `Instance::enumerate_adapters` の完了待ちにだけ使う最小の実装。
/// 他スレッドからの起床を要する future には向かない（本 bin の用途では busy-poll で足りる）。
fn block_on<F: Future>(fut: F) -> F::Output {
    let mut fut = pin!(fut);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(out) = fut.as_mut().poll(&mut cx) {
            return out;
        }
        std::thread::yield_now();
    }
}

fn main() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapters = block_on(instance.enumerate_adapters(wgpu::Backends::all()));

    println!("wgpu adapters: {} adapter(s)", adapters.len());
    for (i, adapter) in adapters.iter().enumerate() {
        let info = adapter.get_info();
        println!(
            "[{i}] name={:?} backend={:?} device_type={:?}",
            info.name, info.backend, info.device_type
        );
    }
}
