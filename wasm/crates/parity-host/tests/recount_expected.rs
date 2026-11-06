//! 照合スクリプトの `--recount --expect-cases` を、実物の入力（`wasm/node/cases.json` と `generate` の出力）で確かめる試験
//! （spec TASK-994 のポインタ）。保存の形の行の集合・build と、simd128 版の gemm_simd の別の和の順の印を、
//! 期待の入力から決めて突き合わせる。期待の入力の `cases` の欄が host の表と食い違えば基準の試験が落ちる。
//! 寸法と種類を書き換えた保存の形は合成の入力で `scripts/test-wasm-node-check.sh` が確かめる。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use fandhe_silicon_core::hash::sha256_bytes;
use wasm_parity_host::generate;

/// 試験ごとの一時ディレクトリ。破棄で消す。
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("wasm-parity-expect-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).unwrap();
        TempDir(path)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
}

/// 期待の入力の native-scalar の生成の行。
#[derive(Clone)]
struct Row {
    id: String,
    export: String,
    build: String,
    case: String,
}

impl Row {
    /// 別の和の順で走ることを期待した行（simd128 版の gemm_simd で、gemm 3 ケースの行）か。
    fn expects_other_order(&self) -> bool {
        self.export == "gemm_simd"
            && self.build == "simd128"
            && ["gemm-64", "gemm-256", "gemm-37x41x53"].contains(&self.case.as_str())
    }
}

/// `cases.json` の行を 1 行 1 鍵の整形のまま走査し、native-scalar の生成の行を集める。
fn expected_rows() -> Vec<Row> {
    let text = fs::read_to_string(root().join("wasm/node/cases.json")).unwrap();
    let value = |l: &str, key: &str| -> Option<String> {
        l.trim()
            .strip_prefix(&format!("\"{key}\": \""))
            .map(|v| v.trim_end_matches(',').trim_end_matches('"').to_string())
    };
    let mut rows = Vec::new();
    let mut cur: [Option<String>; 5] = Default::default();
    let flush = |cur: &mut [Option<String>; 5], rows: &mut Vec<Row>| {
        if let [Some(id), Some(export), Some(build), Some(case), Some(src)] = cur.clone()
            && src == "native-scalar"
        {
            rows.push(Row {
                id,
                export,
                build,
                case,
            });
        }
        *cur = Default::default();
    };
    for l in text.lines() {
        if let Some(v) = value(l, "id") {
            flush(&mut cur, &mut rows);
            cur[0] = Some(v);
        }
        for (i, key) in ["export", "build", "case", "reference_source"]
            .into_iter()
            .enumerate()
        {
            if let Some(v) = value(l, key) {
                cur[i + 1] = Some(v);
            }
        }
    }
    flush(&mut cur, &mut rows);
    rows
}

/// 行と出力のバイト列の組の列。
type Outputs = Vec<(Row, Vec<u8>)>;
/// 書く行を選ぶ関数。
type Pick = Box<dyn Fn(Outputs) -> Outputs>;

/// `outputs.json` の本文。
fn outputs_json(rows: &[(Row, Vec<u8>)]) -> String {
    let body: Vec<String> = rows
        .iter()
        .map(|(r, bytes)| {
            format!(
                "{{\"id\": \"{}\", \"export\": \"{}\", \"build\": \"{}\", \"case\": \"{}\", \"output_file\": \"{}.bin\", \"output_sha256\": \"{}\"}}",
                r.id,
                r.export,
                r.build,
                r.case,
                r.id,
                hex(&sha256_bytes(bytes))
            )
        })
        .collect();
    format!("{{\"rows\": [{}]}}", body.join(", "))
}

/// 行ごとの出力。参照の写しを基本にし、別の和の順を期待した行は `exact` が偽なら最初の語を 1 ビット動かす。
fn row_outputs(dir: &Path, rows: &[Row], exact: bool) -> Outputs {
    rows.iter()
        .map(|r| {
            let mut bytes = fs::read(dir.join(format!("{}.native-scalar.bin", r.case))).unwrap();
            if r.expects_other_order() && !exact {
                bytes[0] ^= 1;
            }
            (r.clone(), bytes)
        })
        .collect()
}

/// 保存の形を書く（`generate` の出力に行の出力と `outputs.json` を足す）。
fn write_form(dir: &Path, rows: &[(Row, Vec<u8>)]) {
    for (r, bytes) in rows {
        fs::write(dir.join(format!("{}.bin", r.id)), bytes).unwrap();
    }
    fs::write(dir.join("outputs.json"), outputs_json(rows)).unwrap();
}

/// 期待の入力を付けた `--recount` の終了コードと標準出力・標準エラー。
fn recount(dir: &Path) -> (i32, String, String) {
    let out = Command::new("bash")
        .arg(root().join("scripts/wasm-node-check.sh"))
        .arg("--recount")
        .arg(dir)
        .arg("--expect-cases")
        .arg(root().join("wasm/node/cases.json"))
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// 保存の形を作って `--recount --expect-cases` を流す。`pick` が行の出力から実際に書く行を選ぶ。
fn run(tag: &str, exact: bool, pick: impl Fn(Outputs) -> Outputs) -> (i32, String, String) {
    let dir = TempDir::new(tag);
    generate(dir.path()).unwrap();
    let rows = pick(row_outputs(dir.path(), &expected_rows(), exact));
    write_form(dir.path(), &rows);
    recount(dir.path())
}

#[test]
fn expected_form_passes() {
    let n = expected_rows().len();
    assert!(n > 100, "{n}");
    let (code, out, err) = run("pass", false, |r| r);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains(&format!(
            "recount-summary: rows={n} zero={n} nonzero=0 uncompared=0"
        )),
        "{out}"
    );
}

#[test]
fn missing_or_extra_rows_fail() {
    let cases: [(&str, Pick); 4] = [
        (
            "drop-one",
            Box::new(|mut r| {
                r.remove(0);
                r
            }),
        ),
        (
            "drop-three",
            Box::new(|mut r| {
                r.drain(0..3);
                r
            }),
        ),
        ("keep-one", Box::new(|r| r.into_iter().take(1).collect())),
        (
            "drop-last",
            Box::new(|mut r| {
                r.pop();
                r
            }),
        ),
    ];
    for (tag, pick) in cases {
        let (code, out, err) = run(tag, false, pick);
        assert_eq!(code, 2, "{tag}: {out}{err}");
        assert!(
            err.contains("saved rows differ from the expected rows"),
            "{tag}: {err}"
        );
        assert!(!out.contains("recount-summary:"), "{tag}: {out}");
    }
}

#[test]
fn changed_build_fails() {
    // 行の build を書き換えると、保存の形の中の値だけでは比べ方が閾値に切り替わる。期待の入力と食い違えば 2 にする。
    let (code, out, err) = run("build", false, |mut r| {
        let i = r.iter().position(|(r, _)| r.expects_other_order()).unwrap();
        r[i].0.build = "scalar".to_string();
        r
    });
    assert_eq!(code, 2, "{out}{err}");
    assert!(
        err.contains("saved rows differ from the expected rows"),
        "{err}"
    );
}

#[test]
fn simd_rows_identical_to_scalar_reference_fail() {
    // simd128 版の gemm_simd の 3 行が参照と全語同じ（別の和の順で走らなかった）なら、件数が 0 でも失敗にする。
    let (code, out, err) = run("flat", true, |r| r);
    assert_eq!(code, 1, "{out}{err}");
    let flat: Vec<&str> = out
        .lines()
        .filter(|l| l.starts_with("recount-flat: "))
        .collect();
    assert_eq!(flat.len(), 3, "{out}");
    for case in ["gemm-64", "gemm-256", "gemm-37x41x53"] {
        let id = format!("recount-flat: {case}-simd128-gemm-simd max-abs=0.0");
        assert!(flat.contains(&id.as_str()), "{id}: {out}");
    }
    assert!(out.contains("nonzero=3"), "{out}");
}
