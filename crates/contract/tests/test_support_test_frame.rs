//! contract の feature `test-support` だけで、core の試験の枠（宣言と実行の可否、結果のラベルの決定、組み合わせごとの集計）に届くことの確認（spec TASK-23）。
//! `make build-profiles` の `test-support` を含む各組で、背後実装の feature に依らず同じ結果になることも確かめる。
//! 別 crate からのマクロ・端から端の表・背後実装の feature ごとの宣言も確かめる。
//! 置き場・feature の説明は docs/test-frame.md の「置き場と有効になる条件」。
//! Metal の宣言の試験は、枠が走らせると決めた経路で device を取って照らし、記録の部品を 1 件書き出す（TASK-2945）。
//! Vulkan・CUDA の宣言の試験は、走らせない経路でだけ記録の部品を書き出す（走らせる経路では device に触れないので書かない）。
//! 環境変数 `FANDHE_SILICON_TEST_RECORD_DIR` が未設定なら何も書かず、設定されていれば（`make verify` が渡す）、
//! 機材の無い環境では宣言した組み合わせごとの「未実施」の部品が書かれる。

use fandhe_silicon_core::device::{Device, DeviceDiagnostics};
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::test_frame::{
    Backend, COMBINATION_ENV, Decision, Declaration, ExportError, Machine, MachineCombination,
    NotRunReason, Outcome, RECORD_DIR_ENV, RunResult, RunSetting, Subject, decide,
    judge_and_export_with, on_acquired_device,
};
use std::ffi::OsString;

#[test]
fn contract_test_support_enables_core_test_frame() {
    let d = Declaration::from_parts(&[MachineCombination::MetalMacMini], &[]).unwrap();
    let st = RunSetting::from_lookup(|n| {
        (n == COMBINATION_ENV).then(|| OsString::from("metal-mac-mini"))
    });
    assert_eq!(
        decide(&d, &st),
        Decision::Run(MachineCombination::MetalMacMini)
    );
}

#[test]
fn decisions_are_fixed_under_any_backend_feature_set() {
    let d = Declaration::from_parts(&[MachineCombination::CudaNodeA], &["x"]).unwrap();
    let unset = RunSetting::from_lookup(|_| None);
    assert_eq!(
        decide(&d, &unset),
        Decision::NotRun(NotRunReason::NotConfigured)
    );
    let other =
        RunSetting::from_lookup(|n| (n == COMBINATION_ENV).then(|| OsString::from("vulkan-amd")));
    assert_eq!(
        decide(&d, &other),
        Decision::NotRun(NotRunReason::CombinationNotDeclared)
    );
}

#[test]
fn judge_is_reachable_and_fixed_under_any_backend_feature_set() {
    use fandhe_silicon_core::test_frame::{Label, RunResult, Subject, Verdict, judge};
    let s = Subject::from_parts("reach_check", &["BUILD-54"], &[]).unwrap();
    let d = Declaration::from_parts(&[MachineCombination::MetalMacMini], &[]).unwrap();
    let st = RunSetting::from_lookup(|n| {
        (n == COMBINATION_ENV).then(|| OsString::from("metal-mac-mini"))
    });
    let ok = judge(&s, &d, &st, |_| RunResult::Verdict(Verdict::Match));
    assert_eq!(ok.label(), &Label::Ok);
    let ng = judge(&s, &d, &st, |_| RunResult::Verdict(Verdict::Mismatch));
    assert_eq!(ng.label(), &Label::Ng);
    let unset = RunSetting::from_lookup(|_| None);
    let nr = judge(&s, &d, &unset, |_| RunResult::Verdict(Verdict::Match));
    assert_eq!(nr.label(), &Label::NotRun(NotRunReason::NotConfigured));
}

#[test]
fn summary_is_reachable_and_fixed_under_any_backend_feature_set() {
    use fandhe_silicon_core::test_frame::{LabelKind, RunResult, Subject, Summary, Verdict, judge};
    let s = Subject::from_parts("summary_reach", &["BUILD-54"], &[]).unwrap();
    let d = Declaration::from_parts(
        &[
            MachineCombination::MetalMacMini,
            MachineCombination::CudaNodeA,
        ],
        &[],
    )
    .unwrap();
    let st = RunSetting::from_lookup(|n| {
        (n == COMBINATION_ENV).then(|| OsString::from("metal-mac-mini"))
    });
    let o = judge(&s, &d, &st, |_| RunResult::Verdict(Verdict::Mismatch));
    let sum = Summary::from_outcomes(&[o]).unwrap();
    let metal = sum.combination(MachineCombination::MetalMacMini);
    assert_eq!(metal.get(LabelKind::Ng), 1);
    let cuda = sum.combination(MachineCombination::CudaNodeA);
    assert_eq!(cuda.get(LabelKind::NotRun), 1);
    assert!(sum.has_ng());
    assert_eq!(sum.totals().total(), 1);
}

#[test]
fn declaration_macro_is_reachable_from_another_crate() {
    let via_macro = fandhe_silicon_core::declaration!(
        [MachineCombination::VulkanAmd, MachineCombination::CudaNodeA,],
        ["fault",]
    );
    let via_fn = Declaration::from_parts(
        &[MachineCombination::VulkanAmd, MachineCombination::CudaNodeA],
        &["fault"],
    );
    assert!(via_macro.is_ok());
    assert_eq!(via_macro, via_fn);
}

#[test]
fn end_to_end_table_is_fixed_under_any_backend_feature_set() {
    use fandhe_silicon_core::test_frame::{Subject, Summary, Verdict, judge};
    let s = Subject::from_parts("e2e_reach", &["BUILD-54"], &[]).unwrap();
    let st = RunSetting::from_lookup(|n| {
        (n == COMBINATION_ENV).then(|| OsString::from("vulkan-node-a"))
    });
    let mut outcomes = Vec::new();
    for backend in [Backend::Metal, Backend::Vulkan, Backend::Cuda] {
        let cs: Vec<MachineCombination> = MachineCombination::ALL
            .into_iter()
            .filter(|c| c.backend() == backend)
            .collect();
        let d = Declaration::from_parts(&cs, &[]).unwrap();
        outcomes.push(judge(&s, &d, &st, |_| {
            RunResult::Verdict(Verdict::Mismatch)
        }));
    }
    let table = Summary::from_outcomes(&outcomes).unwrap().to_string();
    let expected = "\
| combination | ok | ng | pending | not-run | indeterminate |
| --- | ---: | ---: | ---: | ---: | ---: |
| metal-mac-mini | 0 | 0 | 0 | 1 | 0 |
| vulkan-node-b | 0 | 0 | 0 | 1 | 0 |
| vulkan-node-a | 0 | 1 | 0 | 0 | 0 |
| vulkan-amd | 0 | 0 | 0 | 1 | 0 |
| vulkan-intel | 0 | 0 | 0 | 1 | 0 |
| cuda-node-b | 0 | 0 | 0 | 1 | 0 |
| cuda-node-a | 0 | 0 | 0 | 1 | 0 |";
    assert_eq!(table, expected);
}

/// 背後実装の feature を持つ組でだけ使う補助。その背後実装の組み合わせだけを宣言し、設定がその組み合わせのときだけ走ることを確かめる。
#[cfg(any(
    feature = "backend-metal",
    feature = "backend-vulkan",
    feature = "backend-cuda"
))]
fn declares_and_runs_only_on(backend: Backend) {
    use fandhe_silicon_core::test_frame::{Label, Subject, Verdict, judge};
    let s = Subject::from_parts("backend_decl", &["BUILD-54"], &[]).unwrap();
    let cs: Vec<MachineCombination> = MachineCombination::ALL
        .into_iter()
        .filter(|c| c.backend() == backend)
        .collect();
    let d = Declaration::from_parts(&cs, &[]).unwrap();
    for c in MachineCombination::ALL {
        let st =
            RunSetting::from_lookup(|n| (n == COMBINATION_ENV).then(|| OsString::from(c.name())));
        let o = judge(&s, &d, &st, |_| RunResult::Verdict(Verdict::Match));
        if c.backend() == backend {
            assert_eq!(o.label(), &Label::Ok);
            assert_eq!(o.ran_on(), Some(c));
        } else {
            assert_eq!(o.ran_on(), None);
            assert_eq!(
                o.label(),
                &Label::NotRun(NotRunReason::CombinationNotDeclared)
            );
        }
    }
}

/// 書き出しつきの入口を通す試験の ID（背後実装ごとに 1 つ）。
///
/// 部品のファイル名は試験の ID と組み合わせの名前で決まり、既にあるファイルは上書きされない。
/// 同じ ID を workspace の別の試験が使うと、同じディレクトリへの 2 回目の書き出しが `AlreadyExists` で失敗し、
/// その試験が落ちる（`make verify` は毎回新しいディレクトリを渡すので、重複はそこで見つかる）。
/// このファイルの中での重複は、下の `export_test_ids_are_distinct` が確かめる。
const METAL_EXPORT_ID: &str = "contract_metal_declaration";
const VULKAN_EXPORT_ID: &str = "contract_vulkan_declaration";
const CUDA_EXPORT_ID: &str = "contract_cuda_declaration";

/// 背後実装の組み合わせだけを宣言し、走らせない経路のときだけ記録の部品を書き出す。
///
/// この試験は device に触れないので、枠が「走らせる」と決めた組み合わせ（機材の組み合わせの指定が宣言に当たる）では
/// 何も書かず `Ok(None)` を返す。走らせない経路では、宣言した組み合わせごとの「未実施」の部品を 1 件ずつ書き出す。
/// 書き出し先は `lookup` から、組み合わせの指定は `setting` から得る（環境を書き換えずに確かめられる）。
/// 書き出しの失敗は `Err` で返す。
fn export_when_not_run<L>(
    lookup: L,
    test_id: &str,
    backend: Backend,
    setting: &RunSetting,
) -> Result<Option<Outcome>, ExportError>
where
    L: Fn(&str) -> Option<OsString>,
{
    let s = Subject::from_parts(test_id, &["BUILD-54"], &[]).unwrap();
    let cs: Vec<MachineCombination> = MachineCombination::ALL
        .into_iter()
        .filter(|c| c.backend() == backend)
        .collect();
    let d = Declaration::from_parts(&cs, &[]).unwrap();
    if let Decision::Run(_) = decide(&d, setting) {
        return Ok(None);
    }
    // 走らせない経路なので本体は呼ばれない。呼ばれたら、結果を作らず試験を落とす。
    judge_and_export_with(lookup, &s, &d, setting, |_| {
        unreachable!("the body must not run when the frame decided not to run")
    })
    .map(Some)
}

/// 背後実装の feature を持つ組でだけ使う補助。プロセスの環境で `export_when_not_run` を呼ぶ。
/// 書き出しの失敗は握りつぶさず試験の失敗にする。
#[cfg(any(feature = "backend-vulkan", feature = "backend-cuda"))]
fn declares_and_exports(test_id: &str, backend: Backend) {
    export_when_not_run(
        |n| std::env::var_os(n),
        test_id,
        backend,
        &RunSetting::from_env(),
    )
    .expect("exporting the record parts must succeed");
}

/// 背後実装の組み合わせだけを宣言し、枠が走らせると決めたら `acquire` で device を取って照らしてから `body` を呼ぶ。
/// どの経路でも記録の部品を書き出す（走らせない経路は宣言した組み合わせごとに 1 件、走らせる経路は 1 件）。
fn export_with_device<L, A, M, B>(
    lookup: L,
    test_id: &str,
    backend: Backend,
    setting: &RunSetting,
    acquire: A,
    machine_of: M,
    body: B,
) -> Result<Outcome, ExportError>
where
    L: Fn(&str) -> Option<OsString>,
    A: FnOnce(MachineCombination) -> Result<Box<dyn Device>, ContractError>,
    M: FnOnce(&DeviceDiagnostics) -> Option<Machine>,
    B: FnOnce(MachineCombination, &dyn Device) -> RunResult,
{
    let s = Subject::from_parts(test_id, &["BUILD-54"], &[]).unwrap();
    let cs: Vec<MachineCombination> = MachineCombination::ALL
        .into_iter()
        .filter(|c| c.backend() == backend)
        .collect();
    let d = Declaration::from_parts(&cs, &[]).unwrap();
    judge_and_export_with(lookup, &s, &d, setting, |c| {
        on_acquired_device(c, acquire, machine_of, body)
    })
}

/// 取った device でバッファを確保し、書いたバイト列を読み戻して比べる本体。
#[cfg(feature = "backend-metal")]
fn round_trip(_: MachineCombination, device: &dyn Device) -> RunResult {
    use fandhe_silicon_core::memory::BufferUsage;
    use fandhe_silicon_core::test_frame::Verdict;
    let memory = device.memory();
    let payload: Vec<u8> = (0..64_u8).collect();
    let usage = BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD;
    let Ok(handle) = memory.allocate(64, usage) else {
        return RunResult::Verdict(Verdict::Mismatch);
    };
    let matched = memory.upload(&handle, 0, &payload).is_ok() && {
        let (tx, rx) = std::sync::mpsc::channel();
        memory.download_async(
            &handle,
            0,
            64,
            Box::new(move |r| {
                let _ = tx.send(r);
            }),
        );
        matches!(
            rx.recv_timeout(std::time::Duration::from_secs(10)),
            Ok(Ok(back)) if back == payload
        )
    };
    memory.free(handle);
    RunResult::Verdict(Verdict::from_match(matched))
}

#[cfg(all(feature = "backend-metal", target_os = "macos"))]
fn acquire_metal(_: MachineCombination) -> Result<Box<dyn Device>, ContractError> {
    use fandhe_silicon_contract::backend_metal::MetalDeviceProvider;
    use fandhe_silicon_core::device::DeviceProvider;
    MetalDeviceProvider::request(compute_request(BackendKind::Metal))
}

/// macOS 以外のビルドには Metal の provider が無いので、device は取れない（機材に届かない）。
#[cfg(all(feature = "backend-metal", not(target_os = "macos")))]
fn acquire_metal(_: MachineCombination) -> Result<Box<dyn Device>, ContractError> {
    Err(ContractError::DeviceUnavailable(
        "metal is not built for this target".into(),
    ))
}

/// Metal の device の診断の値を機材の呼び名に写す。
/// 背後実装の名前と adapter の名前の完全一致だけを写し、ほかの値はどれも `None`（device の食い違い）にする。
#[cfg(feature = "backend-metal")]
fn metal_machine(d: &DeviceDiagnostics) -> Option<Machine> {
    (d.backend_name == "metal" && d.adapter_name == "Apple M4").then_some(Machine::MacMini)
}

/// device を取る関数の型（試験の中で差し替える）。
#[cfg(feature = "backend-metal")]
type Acquire = fn(MachineCombination) -> Result<Box<dyn Device>, ContractError>;

#[cfg(feature = "backend-metal")]
use fandhe_silicon_core::capability::BackendKind;

#[cfg(feature = "backend-metal")]
fn compute_request(backend: BackendKind) -> fandhe_silicon_core::device::DeviceRequest {
    use fandhe_silicon_core::device::{DeviceOwnership, DeviceRequest, Profile};
    DeviceRequest {
        backend,
        profile: Profile::ComputeOnly,
        ownership: DeviceOwnership::ProductIndependent,
    }
}

#[cfg(feature = "backend-metal")]
#[test]
fn metal_test_declares_on_the_frame() {
    declares_and_runs_only_on(Backend::Metal);
    let o = export_with_device(
        |n| std::env::var_os(n),
        METAL_EXPORT_ID,
        Backend::Metal,
        &RunSetting::from_env(),
        acquire_metal,
        metal_machine,
        round_trip,
    )
    .expect("exporting the record parts must succeed");
    assert!(!o.label().fails_test(), "{:?}", o.label());
}

/// 本物の試験と同じ口・同じ写しに、試験用の device と取れない device を通す（device には触らない）。
#[cfg(feature = "backend-metal")]
#[test]
fn the_device_path_writes_one_indeterminate_part_when_the_device_does_not_match_or_is_unreachable()
{
    use fandhe_silicon_core::device::DeviceProvider;
    use fandhe_silicon_core::stub_device::StubDeviceProvider;
    let st = RunSetting::from_lookup(|n| {
        (n == COMBINATION_ENV).then(|| OsString::from("metal-mac-mini"))
    });
    let cases: [(Acquire, &str); 2] = [
        (
            |_| StubDeviceProvider::request(compute_request(BackendKind::Metal)),
            "device-mismatch",
        ),
        (
            |_| Err(ContractError::DeviceUnavailable("none".into())),
            "machine-unreachable",
        ),
    ];
    for (i, (acquire, reason)) in cases.into_iter().enumerate() {
        let dir = temp_dir(&format!("device-{i}"));
        let dir_value = OsString::from(dir.as_os_str());
        let o = export_with_device(
            |n| (n == RECORD_DIR_ENV).then(|| dir_value.clone()),
            "device_path_check",
            Backend::Metal,
            &st,
            acquire,
            metal_machine,
            |_, _| unreachable!("the body must not run on a device that does not match"),
        )
        .unwrap();
        assert_eq!(o.ran_on(), Some(MachineCombination::MetalMacMini));
        assert_eq!(file_count(&dir), 1);
        let json =
            std::fs::read_to_string(dir.join("device_path_check.metal-mac-mini.json")).unwrap();
        assert!(json.contains(r#""label":"indeterminate""#), "{json}");
        assert!(
            json.contains(&format!(r#""label_reason":"{reason}""#)),
            "{json}"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

#[test]
fn the_device_path_does_not_acquire_when_the_frame_does_not_run() {
    let dir = temp_dir("device-not-run");
    let dir_value = OsString::from(dir.as_os_str());
    let o = export_with_device(
        |n| (n == RECORD_DIR_ENV).then(|| dir_value.clone()),
        "device_path_not_run",
        Backend::Metal,
        &RunSetting::from_lookup(|_| None),
        |_| unreachable!("acquire must not run when the frame decided not to run"),
        |_| unreachable!("the map must not run when the frame decided not to run"),
        |_, _| unreachable!("the body must not run when the frame decided not to run"),
    )
    .unwrap();
    assert_eq!(o.ran_on(), None);
    assert_eq!(
        file_count(&dir),
        MachineCombination::ALL
            .into_iter()
            .filter(|c| c.backend() == Backend::Metal)
            .count()
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// 初期化の失敗を差し込む Metal の provider を通すと、機材に届かない判定不能になる（device には触らない）。
#[cfg(all(
    feature = "backend-metal",
    feature = "fault-injection",
    target_os = "macos"
))]
#[test]
fn an_injected_metal_request_failure_is_machine_unreachable() {
    use fandhe_silicon_contract::backend_metal::MetalDefaultDeviceNoneProvider;
    use fandhe_silicon_core::device::DeviceProvider;
    use fandhe_silicon_core::test_frame::{IndeterminateReason, Label};
    let st = RunSetting::from_lookup(|n| {
        (n == COMBINATION_ENV).then(|| OsString::from("metal-mac-mini"))
    });
    let o = export_with_device(
        |_| None,
        "device_path_injected",
        Backend::Metal,
        &st,
        |_| MetalDefaultDeviceNoneProvider::request(compute_request(BackendKind::Metal)),
        metal_machine,
        |_, _| unreachable!("the body must not run when the device was not acquired"),
    )
    .unwrap();
    assert_eq!(
        o.label(),
        &Label::Indeterminate(IndeterminateReason::MachineUnreachable)
    );
}
#[cfg(feature = "backend-vulkan")]
#[test]
fn vulkan_test_declares_on_the_frame() {
    declares_and_runs_only_on(Backend::Vulkan);
    declares_and_exports(VULKAN_EXPORT_ID, Backend::Vulkan);
}

#[cfg(feature = "backend-cuda")]
#[test]
fn cuda_test_declares_on_the_frame() {
    declares_and_runs_only_on(Backend::Cuda);
    declares_and_exports(CUDA_EXPORT_ID, Backend::Cuda);
}

#[test]
fn export_test_ids_are_distinct() {
    let ids = [METAL_EXPORT_ID, VULKAN_EXPORT_ID, CUDA_EXPORT_ID];
    for (i, a) in ids.iter().enumerate() {
        assert!(fandhe_silicon_core::test_frame::TestId::new(a).is_ok());
        for b in &ids[i + 1..] {
            assert_ne!(a, b);
        }
    }
}

#[test]
fn record_parts_are_reachable_and_fixed_under_any_backend_feature_set() {
    use fandhe_silicon_core::test_frame::{Subject, Verdict, judge};
    let s = Subject::from_parts("record_reach", &["BUILD-54"], &[]).unwrap();
    let d = Declaration::from_parts(
        &[
            MachineCombination::MetalMacMini,
            MachineCombination::CudaNodeA,
        ],
        &[],
    )
    .unwrap();
    let unset = RunSetting::from_lookup(|_| None);
    let o = judge(&s, &d, &unset, |_| RunResult::Verdict(Verdict::Match));
    let parts = o.record_parts();
    assert_eq!(parts.len(), 2);
    assert_eq!(
        parts[1].to_json(),
        r#"{"behavior_ids":["BUILD-54"],"machine_combination":"cuda-node-a","machine":"node-a","label":"not-run","label_reason":"not-configured","body_kind":"contract-test","body":{"test_id":"record_reach"}}"#
    );
}

/// 書き出しの確認用の一時ディレクトリを作る（試験ごとに別の名前。既にあれば作り直す）。
fn temp_dir(tag: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!(
        "fandhe-silicon-contract-export-{}-{tag}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir(&p).unwrap();
    p
}

fn file_count(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir).unwrap().count()
}

fn backend_tag(b: Backend) -> &'static str {
    match b {
        Backend::Metal => "metal",
        Backend::Vulkan => "vulkan",
        Backend::Cuda => "cuda",
    }
}

#[test]
fn a_run_path_writes_no_record_and_a_not_run_path_writes_one_per_declared_combination() {
    for backend in [Backend::Vulkan, Backend::Cuda] {
        let declared = MachineCombination::ALL
            .into_iter()
            .filter(|c| c.backend() == backend)
            .count();
        // 組み合わせの指定が無い場合と、全ての組み合わせの指定。
        let mut settings: Vec<Option<MachineCombination>> = vec![None];
        settings.extend(MachineCombination::ALL.into_iter().map(Some));
        for (i, chosen) in settings.into_iter().enumerate() {
            let dir = temp_dir(&format!("{}-{i}", backend_tag(backend)));
            let dir_value = OsString::from(dir.as_os_str());
            let st = RunSetting::from_lookup(|n| {
                chosen.and_then(|c| (n == COMBINATION_ENV).then(|| OsString::from(c.name())))
            });
            let out = export_when_not_run(
                |n| (n == RECORD_DIR_ENV).then(|| dir_value.clone()),
                "export_path_check",
                backend,
                &st,
            )
            .unwrap();
            let runs = chosen.is_some_and(|c| c.backend() == backend);
            if runs {
                assert!(out.is_none());
                assert_eq!(file_count(&dir), 0);
            } else {
                assert!(out.is_some());
                assert_eq!(file_count(&dir), declared);
            }
            std::fs::remove_dir_all(&dir).unwrap();
        }
    }
}

#[test]
fn an_unset_record_directory_is_not_an_error_on_the_not_run_path() {
    let st = RunSetting::from_lookup(|_| None);
    let out = export_when_not_run(|_| None, "export_path_unset", Backend::Vulkan, &st).unwrap();
    assert!(out.is_some());
}

#[test]
fn an_export_failure_on_the_not_run_path_is_returned() {
    let dir = temp_dir("failure");
    let dir_value = OsString::from(dir.as_os_str());
    let st = RunSetting::from_lookup(|_| None);
    let lookup = |n: &str| (n == RECORD_DIR_ENV).then(|| dir_value.clone());
    assert!(export_when_not_run(lookup, "export_path_twice", Backend::Cuda, &st).is_ok());
    // 同じ試験の ID と組み合わせの部品は上書きされず、2 回目は失敗として返る。
    assert_eq!(
        export_when_not_run(lookup, "export_path_twice", Backend::Cuda, &st).unwrap_err(),
        ExportError::AlreadyExists
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
