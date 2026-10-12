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
//! 相対パス・無い・ディレクトリでない・シンボリックリンクのディレクトリは失敗にする。
//! 既存のファイルは上書きしない（`create_new`。同名のシンボリックリンクがあっても辿らず、失敗にする）。
//! 書き込みが途中で失敗したファイルは消し、1 回の呼び出しで先に作ったファイルも消す。
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
use std::path::Path;

/// 記録のファイルを置くディレクトリ（絶対パス）を渡す環境変数の名前。未設定なら何も書かない。
pub const RECORD_DIR_ENV: &str = "FANDHE_SILICON_TEST_RECORD_DIR";

/// 書き出しの失敗。パスや入力の値は含めず、種類だけを表す。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportError {
    /// ディレクトリの指定が絶対パスでない（空を含む）。
    NotAbsolute,
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
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportError::NotAbsolute => f.write_str("record directory is not an absolute path"),
            ExportError::DirectoryMissing => f.write_str("record directory does not exist"),
            ExportError::DirectoryIsSymlink => f.write_str("record directory is a symbolic link"),
            ExportError::NotADirectory => f.write_str("record directory is not a directory"),
            ExportError::AlreadyExists => f.write_str("record file already exists"),
            ExportError::Io(kind) => write!(f, "record file io failed: {kind}"),
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

/// 書き出し先のディレクトリを検査する。末尾の要素はシンボリックリンクなら辿らずに失敗にする。
fn check_dir(dir: &Path) -> Result<(), ExportError> {
    if !dir.is_absolute() {
        return Err(ExportError::NotAbsolute);
    }
    match fs::symlink_metadata(dir) {
        Err(e) if e.kind() == ErrorKind::NotFound => Err(ExportError::DirectoryMissing),
        Err(e) => Err(ExportError::Io(e.kind())),
        Ok(m) if m.file_type().is_symlink() => Err(ExportError::DirectoryIsSymlink),
        Ok(m) if !m.is_dir() => Err(ExportError::NotADirectory),
        Ok(_) => Ok(()),
    }
}

/// 新しいファイルを作って書く。既存（シンボリックリンクを含む）なら失敗にし、書き込みに失敗したら消す。
fn write_new(path: &Path, content: &[u8]) -> Result<(), ExportError> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| map_io(&e))?;
    if let Err(e) = file.write_all(content).and_then(|()| file.flush()) {
        drop(file);
        // 作ったファイルだけを消す（ここに来るのは create_new が成功した後だけ）。
        let _ = fs::remove_file(path);
        return Err(ExportError::Io(e.kind()));
    }
    Ok(())
}

/// 部品をディレクトリの直下へ 1 件 1 ファイルで書き、書いたファイル名を部品の順に返す。
///
/// 途中で失敗したら、この呼び出しで作ったファイルを消してから `Err` を返す。既存のファイルには触れない。
pub fn export_parts(dir: &Path, parts: &[RecordPart]) -> Result<Vec<String>, ExportError> {
    check_dir(dir)?;
    let mut created: Vec<String> = Vec::with_capacity(parts.len());
    for part in parts {
        let name = record_file_name(part.test_id(), part.combination());
        let mut content = part.to_json();
        content.push('\n');
        if let Err(e) = write_new(&dir.join(&name), content.as_bytes()) {
            for done in &created {
                let _ = fs::remove_file(dir.join(done));
            }
            return Err(e);
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
