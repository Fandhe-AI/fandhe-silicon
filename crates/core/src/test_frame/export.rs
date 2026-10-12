//! 試験ごとの記録の部品（`record::RecordPart`）のファイルへの書き出しと、判定から書き出しまでを 1 回で行う入口。
//!
//! 試験の関数の末尾（[`judge_and_export`]）と、手順のスクリプト・CI が設定する環境変数
//! [`RECORD_DIR_ENV`] から呼ばれる。枠の中でファイルを触るのはこのモジュールだけ。
//! 環境変数が未設定なら何も書かない（既定はファイルを作らない）。設定されていれば、
//! 部品 1 件につき 1 ファイルを、そのディレクトリの直下に `<試験の ID>.<組み合わせの名前>.json` で作る。
//! 試験の ID は `[a-z0-9_-]`、組み合わせの名前は `[a-z0-9-]` だけで、どちらも `.`・パスの区切りを含まない。
//! だから名前はディレクトリの外に出られず、別の (試験の ID, 組み合わせ) の対と衝突しない。
//!
//! 環境変数の値（パス）は信頼しない入力として扱う。ディレクトリは作らない（無ければ失敗）。
//! 相対パス・`..` の要素を含むパス・無い・ディレクトリでない・シンボリックリンクのディレクトリは失敗にする。
//! 末尾の区切りと `.` の要素は、リンクかどうかを見る前に取り除く（`Path::components` による字句の正規化で、`canonicalize` は使わない）。
//! 限界: 途中の要素のシンボリックリンクは辿る。検査と書き込みのあいだにディレクトリが差し替えられることは防がない。
//! 既存のファイルは上書きしない（`create_new`。同名のシンボリックリンクがあっても辿らず、失敗にする）。
//! 書き込みが途中で失敗したファイルは消し、1 回の呼び出しで先に作ったファイルも消す。1 つ消せなくても残りは消し、
//! 消せないものがあれば `CleanupFailed`（元の失敗と削除の失敗の種類だけ）を返す。すでに無いファイルは失敗に数えない。
//! エラーは種類だけで、パスを含めない。書き出しの失敗は `Err` で返り、ラベルを変えない。
//! 環境変数を読む関数と、ディレクトリを引数で受けて書く関数を分けてある（テストは後者に一時ディレクトリを渡す）。
//! 標準出力・標準エラーには何も出さない。spec TASK-23。

use super::combination::MachineCombination;
use super::declaration::Declaration;
use super::identifier::TestId;
use super::outcome::{Outcome, RunResult, Subject, judge};
use super::record::RecordPart;
use super::setting::RunSetting;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Component, Path, PathBuf};

/// 記録のファイルを置くディレクトリ（絶対パス）を渡す環境変数の名前。未設定なら何も書かない。
pub const RECORD_DIR_ENV: &str = "FANDHE_SILICON_TEST_RECORD_DIR";

/// 書き出しの失敗。パスや入力の値は含めず、種類だけを表す。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportError {
    /// ディレクトリの指定が絶対パスでない（空を含む）。
    NotAbsolute,
    /// ディレクトリの指定に `..` の要素がある。
    ParentComponent,
    /// ディレクトリが無い。
    DirectoryMissing,
    /// ディレクトリの指定がシンボリックリンク。
    DirectoryIsSymlink,
    /// ディレクトリの指定がディレクトリでない。
    NotADirectory,
    /// 同じ名前のファイルが既にある。
    AlreadyExists,
    /// その他の入出力の失敗。値は失敗の種類だけ。
    Io(ErrorKind),
    /// 書き出しに失敗し、その後始末の削除にも失敗した。作ったファイル（途中までの内容を含む）が残りうる。
    /// 値は元の失敗の種類と削除の失敗の種類だけで、パス・ファイル名・OS のメッセージは持たない。
    CleanupFailed {
        /// 元の失敗の種類。
        cause: ErrorKind,
        /// 削除の失敗の種類。
        cleanup: ErrorKind,
    },
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportError::NotAbsolute => f.write_str("record directory is not an absolute path"),
            ExportError::ParentComponent => {
                f.write_str("record directory has a parent directory component")
            }
            ExportError::DirectoryMissing => f.write_str("record directory does not exist"),
            ExportError::DirectoryIsSymlink => f.write_str("record directory is a symbolic link"),
            ExportError::NotADirectory => f.write_str("record directory is not a directory"),
            ExportError::AlreadyExists => f.write_str("record file already exists"),
            ExportError::Io(kind) => write!(f, "record file io failed: {kind}"),
            ExportError::CleanupFailed { cause, cleanup } => write!(
                f,
                "record export failed ({cause}) and cleanup failed ({cleanup}); written files may remain"
            ),
        }
    }
}

impl std::error::Error for ExportError {}

fn map_io(e: &std::io::Error) -> ExportError {
    if e.kind() == ErrorKind::AlreadyExists {
        ExportError::AlreadyExists
    } else {
        ExportError::Io(e.kind())
    }
}

/// 部品 1 件のファイル名。`<試験の ID>.<組み合わせの名前>.json`。
pub fn record_file_name(test_id: &TestId, combination: MachineCombination) -> String {
    format!("{}.{}.json", test_id.as_str(), combination.name())
}

/// 書き出し先のディレクトリを検査し、正規化したパスを返す。末尾の要素はシンボリックリンクなら辿らずに失敗にする。
///
/// 末尾の区切りと `.` の要素は取り除いてから見るので、`link/`・`link/.` でもリンクとして拒否される。
fn check_dir(dir: &Path) -> Result<PathBuf, ExportError> {
    if !dir.is_absolute() {
        return Err(ExportError::NotAbsolute);
    }
    if dir.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(ExportError::ParentComponent);
    }
    let dir: PathBuf = dir.components().collect();
    match fs::symlink_metadata(&dir) {
        Err(e) if e.kind() == ErrorKind::NotFound => Err(ExportError::DirectoryMissing),
        Err(e) => Err(ExportError::Io(e.kind())),
        Ok(m) if m.file_type().is_symlink() => Err(ExportError::DirectoryIsSymlink),
        Ok(m) if !m.is_dir() => Err(ExportError::NotADirectory),
        Ok(_) => Ok(dir),
    }
}

/// 新しいファイルを作って書く。既存（シンボリックリンクを含む）なら失敗にする。
///
/// 失敗したときは、失敗の種類と、この呼び出しがファイルを作っていたか（途中までの内容が残りうるか）を返す。
/// 後始末（削除）は呼び出し側が行う。
fn write_new<W>(path: &Path, content: &[u8], write: &W) -> Result<(), (ExportError, bool)>
where
    W: Fn(&mut fs::File, &[u8]) -> std::io::Result<()>,
{
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| (map_io(&e), false))?;
    write(&mut file, content).map_err(|e| (ExportError::Io(e.kind()), true))
}

/// 本番の書き込み: 全部書いて flush する。
fn write_all_flush(file: &mut fs::File, content: &[u8]) -> std::io::Result<()> {
    file.write_all(content)?;
    file.flush()
}

/// 後始末: 渡したファイルを全て消そうとする。1 つ失敗しても残りを続ける。
/// 既に無い（`NotFound`）ものは失敗に数えない。最初の失敗の種類を返す（全て消せたら `None`）。
fn remove_all<R>(paths: &[PathBuf], remove: &R) -> Option<ErrorKind>
where
    R: Fn(&Path) -> std::io::Result<()>,
{
    let mut first = None;
    for p in paths {
        if let Err(e) = remove(p)
            && e.kind() != ErrorKind::NotFound
            && first.is_none()
        {
            first = Some(e.kind());
        }
    }
    first
}

/// 元の失敗の種類（`CleanupFailed` に持たせる形）。
fn cause_kind(e: ExportError) -> ErrorKind {
    match e {
        ExportError::AlreadyExists => ErrorKind::AlreadyExists,
        ExportError::Io(k) => k,
        _ => ErrorKind::Other,
    }
}

/// 部品をディレクトリの直下へ 1 件 1 ファイルで書き、書いたファイル名を部品の順に返す。
///
/// 途中で失敗したら、この呼び出しで作ったファイルを消してから `Err` を返す。既存のファイルには触れない。
/// 消すのに失敗したものがあれば（残りがありうるので）[`ExportError::CleanupFailed`] を返す。
pub fn export_parts(dir: &Path, parts: &[RecordPart]) -> Result<Vec<String>, ExportError> {
    export_parts_with(dir, parts, &write_all_flush, &|p| fs::remove_file(p))
}

/// [`export_parts`] の本体。書き込みと削除の関数だけを差し替えられる（単体テストが失敗を作るために使う）。
fn export_parts_with<W, R>(
    dir: &Path,
    parts: &[RecordPart],
    write: &W,
    remove: &R,
) -> Result<Vec<String>, ExportError>
where
    W: Fn(&mut fs::File, &[u8]) -> std::io::Result<()>,
    R: Fn(&Path) -> std::io::Result<()>,
{
    let dir = check_dir(dir)?;
    let mut created: Vec<String> = Vec::with_capacity(parts.len());
    for part in parts {
        let name = record_file_name(part.test_id(), part.combination());
        let mut content = part.to_json();
        content.push('\n');
        let path = dir.join(&name);
        if let Err((e, left_partial)) = write_new(&path, content.as_bytes(), write) {
            // この呼び出しで作ったものだけを消す（途中までのファイルを含む。もとからあったファイルには触れない）。
            let mut to_remove: Vec<PathBuf> = created.iter().map(|n| dir.join(n)).collect();
            if left_partial {
                to_remove.push(path);
            }
            return Err(match remove_all(&to_remove, remove) {
                None => e,
                Some(cleanup) => ExportError::CleanupFailed {
                    cause: cause_kind(e),
                    cleanup,
                },
            });
        }
        created.push(name);
    }
    Ok(created)
}

/// 名前から値を引く関数でディレクトリを得て、結果の部品を書く。未設定なら何も書かず空の列を返す。
pub fn export_from_lookup<F>(lookup: F, outcome: &Outcome) -> Result<Vec<String>, ExportError>
where
    F: Fn(&str) -> Option<OsString>,
{
    match lookup(RECORD_DIR_ENV) {
        None => Ok(Vec::new()),
        Some(dir) => export_parts(Path::new(&dir), &outcome.record_parts()),
    }
}

/// プロセスの環境変数からディレクトリを得て、結果の部品を書く。`export_from_lookup` へ `std::env::var_os` を渡すだけ。
pub fn export_from_env(outcome: &Outcome) -> Result<Vec<String>, ExportError> {
    export_from_lookup(|name| std::env::var_os(name), outcome)
}

/// 宣言 → 実行の可否 → 本体の実行 → 結果の決定 → 部品の書き出しを行う。ディレクトリは `lookup` から得る。
///
/// 結果の決め方は [`judge`] と同じ。書き出しに失敗したら `Err` を返す（結果を捨てて失敗として伝える）ので、
/// 試験の側は `.expect(..)` などで試験の失敗にする。ラベルは書き出しの成否で変わらない。
pub fn judge_and_export_with<L, F>(
    lookup: L,
    subject: &Subject,
    declaration: &Declaration,
    setting: &RunSetting,
    body: F,
) -> Result<Outcome, ExportError>
where
    L: Fn(&str) -> Option<OsString>,
    F: FnOnce(MachineCombination) -> RunResult,
{
    let outcome = judge(subject, declaration, setting, body);
    export_from_lookup(lookup, &outcome)?;
    Ok(outcome)
}

/// [`judge_and_export_with`] へプロセスの環境変数を渡す入口。試験の関数が最後に呼ぶ。
pub fn judge_and_export<F>(
    subject: &Subject,
    declaration: &Declaration,
    setting: &RunSetting,
    body: F,
) -> Result<Outcome, ExportError>
where
    F: FnOnce(MachineCombination) -> RunResult,
{
    judge_and_export_with(
        |name| std::env::var_os(name),
        subject,
        declaration,
        setting,
        body,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_frame::{Declaration, RunSetting, Subject, Verdict, judge};
    use std::cell::RefCell;
    use std::io;

    fn parts() -> Vec<RecordPart> {
        let d = Declaration::from_parts(
            &[
                MachineCombination::MetalMacMini,
                MachineCombination::CudaNodeB,
                MachineCombination::VulkanAmd,
            ],
            &[],
        )
        .unwrap();
        judge(
            &Subject::from_parts("unit_case", &[], &[]).unwrap(),
            &d,
            &RunSetting::from_lookup(|_| None),
            |_| RunResult::Verdict(Verdict::Match),
        )
        .record_parts()
    }

    fn temp(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "fandhe-silicon-export-unit-{}-{tag}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir(&p).unwrap();
        p
    }

    fn real_write(f: &mut fs::File, c: &[u8]) -> io::Result<()> {
        write_all_flush(f, c)
    }

    /// 2 件目の書き込みを失敗させる書き込み関数。1 件目は書き、2 件目は途中までのつもりで書いてから失敗する。
    fn failing_second_write() -> impl Fn(&mut fs::File, &[u8]) -> io::Result<()> {
        let n = RefCell::new(0);
        move |f, c| {
            *n.borrow_mut() += 1;
            if *n.borrow() == 2 {
                f.write_all(&c[..3])?;
                Err(io::Error::from(ErrorKind::StorageFull))
            } else {
                real_write(f, c)
            }
        }
    }

    #[test]
    fn a_failed_cleanup_gives_the_dedicated_error_and_keeps_deleting_the_rest() {
        let dir = temp("cleanup-fails");
        let attempted = RefCell::new(Vec::new());
        let remove = |p: &Path| {
            attempted
                .borrow_mut()
                .push(p.file_name().unwrap().to_owned());
            if attempted.borrow().len() == 1 {
                Err(io::Error::from(ErrorKind::PermissionDenied))
            } else {
                fs::remove_file(p)
            }
        };
        let r = export_parts_with(&dir, &parts(), &failing_second_write(), &remove);
        assert_eq!(
            r,
            Err(ExportError::CleanupFailed {
                cause: ErrorKind::StorageFull,
                cleanup: ErrorKind::PermissionDenied
            })
        );
        // 1 件目（削除に失敗）と 2 件目の途中までのファイルの両方を消そうとした。2 件目は実際に消えた。
        assert_eq!(attempted.borrow().len(), 2);
        let left: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(left.len(), 1);
        let e = r.unwrap_err();
        let text = format!("{e:?} {e}");
        assert!(!text.contains("fandhe-silicon-export-unit") && !text.contains("unit_case"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_successful_cleanup_returns_the_original_error() {
        let dir = temp("cleanup-ok");
        let r = export_parts_with(&dir, &parts(), &failing_second_write(), &|p| {
            fs::remove_file(p)
        });
        assert_eq!(r, Err(ExportError::Io(ErrorKind::StorageFull)));
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_that_is_already_gone_is_not_a_cleanup_failure() {
        let dir = temp("cleanup-gone");
        let r = export_parts_with(&dir, &parts(), &failing_second_write(), &|p| {
            fs::remove_file(p)?;
            Err(io::Error::from(ErrorKind::NotFound))
        });
        assert_eq!(r, Err(ExportError::Io(ErrorKind::StorageFull)));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_already_exists_failure_keeps_its_kind_inside_the_cleanup_error() {
        let dir = temp("cleanup-exists");
        let p = parts();
        fs::write(
            dir.join(record_file_name(p[1].test_id(), p[1].combination())),
            "keep",
        )
        .unwrap();
        let r = export_parts_with(&dir, &p, &real_write, &|_| {
            Err(io::Error::from(ErrorKind::Other))
        });
        assert_eq!(
            r,
            Err(ExportError::CleanupFailed {
                cause: ErrorKind::AlreadyExists,
                cleanup: ErrorKind::Other
            })
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
