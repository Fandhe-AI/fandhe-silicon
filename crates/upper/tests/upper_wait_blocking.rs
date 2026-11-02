//! 上の層の `wait_blocking` が印と待ちの上限を値のまま渡し、結果を値のまま返すことの統合テスト（TASK-2386）。
//!
//! feature `native-blocking-wait` の下でだけ組む。呼び出しを記録する試験用の device を使う。

mod common;

use std::time::Duration;

use common::{Call, Overrides, Recorder};
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_upper::UpperDevice;

#[test]
fn wait_blocking_passes_token_and_timeout_as_is() {
    // UPPER-38
    let dev = Recorder::new(Overrides::default());
    let queue = UpperDevice::new(&dev).queue();
    let id = queue
        .submit(queue.create_command_encoder().finish())
        .expect("submit");

    let timeouts = [None, Some(Duration::from_millis(5))];
    for timeout in timeouts {
        assert_eq!(queue.wait_blocking(id, timeout), Ok(()));
    }

    let calls = dev.calls();
    let token = calls
        .iter()
        .find_map(|c| match c {
            Call::Submit { token, .. } => *token,
            _ => None,
        })
        .expect("token");
    let waits: Vec<_> = calls
        .iter()
        .filter(|c| matches!(c, Call::WaitBlocking { .. }))
        .collect();
    assert_eq!(
        waits,
        vec![
            &Call::WaitBlocking {
                token,
                timeout: None
            },
            &Call::WaitBlocking {
                token,
                timeout: Some(Duration::from_millis(5))
            },
        ]
    );

    let err = ContractError::InvalidArgument("injected".to_string());
    let dev = Recorder::new(Overrides {
        wait_blocking: Some(err.clone()),
        ..Overrides::default()
    });
    let queue = UpperDevice::new(&dev).queue();
    let id = queue
        .submit(queue.create_command_encoder().finish())
        .expect("submit");
    assert_eq!(queue.wait_blocking(id, None), Err(err));
}
