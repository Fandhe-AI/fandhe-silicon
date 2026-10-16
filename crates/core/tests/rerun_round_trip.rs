//! 流し直しの実行の口（`rerun::run_and_export`）が書いた部品を、前の記録と比べる道具
//! （`scripts/rerun-compare.sh`。中で `scripts/record-summary.sh` を通る）にそのまま通す確認（spec TASK-29）。
//! 枠の単体の試験（`rerun_run.rs`）と道具の単体の試験（`scripts/test-rerun-compare.sh`）が
//! それぞれ別の入力を使うのに対し、ここでは枠が実際に書いた部品を道具が読めることを固定する。
//! 値はすべてダミー。書き出し先は試験ごとの一時ディレクトリで、`FANDHE_SILICON_TEST_RECORD_DIR` には書かない。
//! bash と python3 が無い環境では skip せず失敗にする。置き場と範囲の説明は docs/rerun-frame.md。

use fandhe_silicon_core::rerun::{Check, CheckSet, run_and_export};
use fandhe_silicon_core::test_frame::{
    COMBINATION_ENV, Declaration, IndeterminateReason, MachineCombination, RunResult, RunSetting,
    Subject, Verdict,
};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// 目印のビヘイビア ID。道具の出力に出てはならない。
const MARKER: &str = "MARKER-1";
/// 目印の閾値 ID（正規の綴りで、承認の表に無い）。道具の出力に出てはならない。
const THRESHOLD: &str = "t-4";

const METAL: MachineCombination = MachineCombination::MetalMacMini;
const VULKAN: MachineCombination = MachineCombination::VulkanNodeB;

/// 結果を固定で返すダミーの照合。
struct Dummy {
    subject: Subject,
    declaration: Declaration,
    result: RunResult,
}

impl Dummy {
    fn new(
        id: &str,
        combos: &[MachineCombination],
        thresholds: &[&str],
        result: RunResult,
    ) -> Dummy {
        Dummy {
            subject: Subject::from_parts(id, &[MARKER], thresholds).unwrap(),
            declaration: Declaration::from_parts(combos, &[]).unwrap(),
            result,
        }
    }
}

impl Check for Dummy {
    fn subject(&self) -> &Subject {
        &self.subject
    }
    fn declaration(&self) -> &Declaration {
        &self.declaration
    }
    fn run(&self, _on: MachineCombination) -> RunResult {
        self.result
    }
}

fn setting(comb: Option<&str>) -> RunSetting {
    let comb = comb.map(OsString::from);
    RunSetting::from_lookup(move |n| {
        if n == COMBINATION_ENV {
            comb.clone()
        } else {
            None
        }
    })
}

/// 試験ごとの一時ディレクトリ。先に消してから作り、終わったら消す。
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> TempDir {
        let p = std::env::temp_dir().join(format!(
            "fandhe-rerun-round-trip-{}-{tag}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir(&p).unwrap();
        TempDir(p)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 行を並べ、各行の末尾に改行を付けた文字列にする（期待の出力の組み立て用）。
fn lines(ls: &[&str]) -> String {
    ls.iter().map(|l| format!("{l}\n")).collect()
}

fn ok() -> RunResult {
    RunResult::Verdict(Verdict::Match)
}

fn ng() -> RunResult {
    RunResult::Verdict(Verdict::Mismatch)
}

fn ind(r: IndeterminateReason) -> RunResult {
    RunResult::Indeterminate(r)
}

/// 照合の一覧を流して部品を `dir` へ書き、ディレクトリにあるファイル数を返す。
fn write_record(dir: &Path, comb: Option<&str>, checks: &[&Dummy]) -> usize {
    let list: Vec<&dyn Check> = checks.iter().map(|c| *c as &dyn Check).collect();
    let set = CheckSet::new(&list).unwrap();
    run_and_export(Some(dir), &set, &setting(comb)).unwrap();
    fs::read_dir(dir).unwrap().count()
}

/// 道具を起動して (終了コード, 標準出力, 標準エラー) を返す。引数は配列で渡し、シェルを介さない。
fn run_script(rel: &str, args: &[&Path]) -> (i32, String, String) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel);
    let out = Command::new("bash")
        .arg(path)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("bash and python3 are required");
    (
        out.status.code().expect("script was killed by a signal"),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

/// 比べる道具を通し、出力に目印の ID と一時ディレクトリの名前が出ていないことも確かめる。
fn compare(prev: &Path, new: &Path) -> (i32, String, String) {
    let r = run_script("scripts/rerun-compare.sh", &[prev, new]);
    for text in [&r.1, &r.2] {
        assert!(!text.contains(MARKER), "behavior id leaked");
        assert!(!text.contains(THRESHOLD), "threshold id leaked");
        assert!(!text.contains("fandhe-rerun-round-trip"), "path leaked");
    }
    r
}

#[test]
fn same_unconfigured_run_twice_has_no_differences() {
    let a = Dummy::new("dummy-a", &[METAL, VULKAN], &[], ok());
    let b = Dummy::new("dummy-b", &[METAL], &[], ok());
    let (p, n) = (TempDir::new("same-p"), TempDir::new("same-n"));
    assert_eq!(write_record(&p.0, None, &[&a, &b]), 3);
    assert_eq!(write_record(&n.0, None, &[&a, &b]), 3);
    let (code, out, err) = compare(&p.0, &n.0);
    assert_eq!(code, 0);
    assert_eq!(
        out,
        lines(&[
            "summary: same=3 regressed=0 improved=0 changed=0 removed=0 added=0",
            "ok: no differences from the previous record",
        ])
    );
    assert_eq!(err, "");
}

#[test]
fn every_label_kind_the_frame_writes_passes_the_tool() {
    let checks = [
        Dummy::new("dummy-ok", &[METAL], &[], ok()),
        Dummy::new("dummy-ng", &[METAL], &[], ng()),
        Dummy::new(
            "dummy-unreach",
            &[METAL],
            &[],
            ind(IndeterminateReason::MachineUnreachable),
        ),
        Dummy::new(
            "dummy-mismatch",
            &[METAL],
            &[],
            ind(IndeterminateReason::DeviceMismatch),
        ),
        Dummy::new("dummy-pending", &[METAL], &[THRESHOLD], ok()),
        Dummy::new("dummy-undeclared", &[VULKAN], &[], ok()),
    ];
    let refs: Vec<&Dummy> = checks.iter().collect();
    let (p, n) = (TempDir::new("kinds-p"), TempDir::new("kinds-n"));
    assert_eq!(write_record(&p.0, Some("metal-mac-mini"), &refs), 6);
    assert_eq!(write_record(&n.0, Some("metal-mac-mini"), &refs), 6);

    let (code, _, err) = run_script("scripts/record-summary.sh", &[&p.0]);
    assert_eq!(code, 0, "record-summary rejected the written parts: {err}");

    let (code, out, err) = compare(&p.0, &n.0);
    assert_eq!(code, 0);
    assert_eq!(
        out,
        lines(&[
            "summary: same=6 regressed=0 improved=0 changed=0 removed=0 added=0",
            "ok: no differences from the previous record",
        ])
    );
    assert_eq!(err, "");
}

#[test]
fn changed_results_are_reported_in_the_fixed_order() {
    let unreach = ind(IndeterminateReason::MachineUnreachable);
    let mismatch = ind(IndeterminateReason::DeviceMismatch);
    let d = |id: &str, r: RunResult| Dummy::new(id, &[METAL], &[], r);
    let (pa, pb, pc, pd, pf) = (
        d("dummy-a", ok()),
        d("dummy-b", ng()),
        d("dummy-c", unreach),
        d("dummy-d", ok()),
        d("dummy-f", ok()),
    );
    let (na, nb, nc, ne, nf) = (
        d("dummy-a", ng()),
        d("dummy-b", ok()),
        d("dummy-c", mismatch),
        d("dummy-e", ok()),
        d("dummy-f", ok()),
    );
    let (p, n) = (TempDir::new("chg-p"), TempDir::new("chg-n"));
    // 一覧の順をわざと入れ替え、出力が鍵の昇順で決まることを示す。
    let m = Some("metal-mac-mini");
    assert_eq!(write_record(&p.0, m, &[&pf, &pd, &pc, &pb, &pa]), 5);
    assert_eq!(write_record(&n.0, m, &[&nf, &ne, &nc, &nb, &na]), 5);
    let (code, out, err) = compare(&p.0, &n.0);
    assert_eq!(code, 1);
    assert_eq!(
        out,
        lines(&[
            "regressed: test=dummy-a combination=metal-mac-mini before=ok after=ng",
            "improved: test=dummy-b combination=metal-mac-mini before=ng after=ok",
            "changed: test=dummy-c combination=metal-mac-mini before=indeterminate/machine-unreachable after=indeterminate/device-mismatch",
            "removed: test=dummy-d combination=metal-mac-mini before=ok",
            "added: test=dummy-e combination=metal-mac-mini after=ok",
            "summary: same=1 regressed=1 improved=1 changed=1 removed=1 added=1",
        ])
    );
    assert_eq!(err, "DIFFERENT: 5 difference(s)\n");
}

#[test]
fn unconfigured_then_configured_is_compared_per_combination() {
    let c = Dummy::new("dummy-a", &[METAL, VULKAN], &[], ok());
    let (p, n) = (TempDir::new("cfg-p"), TempDir::new("cfg-n"));
    assert_eq!(write_record(&p.0, None, &[&c]), 2);
    assert_eq!(write_record(&n.0, Some("metal-mac-mini"), &[&c]), 1);
    let (code, out, err) = compare(&p.0, &n.0);
    assert_eq!(code, 1);
    assert_eq!(
        out,
        lines(&[
            "improved: test=dummy-a combination=metal-mac-mini before=not-run/not-configured after=ok",
            "removed: test=dummy-a combination=vulkan-node-b before=not-run/not-configured",
            "summary: same=0 regressed=0 improved=1 changed=0 removed=1 added=0",
        ])
    );
    assert_eq!(err, "DIFFERENT: 2 difference(s)\n");
}

#[test]
fn an_empty_new_record_is_an_error_not_no_difference() {
    let a = Dummy::new("dummy-a", &[METAL], &[], ok());
    let (p, n) = (TempDir::new("empty-p"), TempDir::new("empty-n"));
    assert_eq!(write_record(&p.0, None, &[&a]), 1);
    let (code, out, err) = compare(&p.0, &n.0);
    assert_eq!(code, 2);
    assert_eq!(out, "");
    assert!(
        err.lines().any(|l| l.starts_with("error:")),
        "unexpected stderr: {err}"
    );
}
