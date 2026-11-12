//! 実行時に渡されるカーネルの一覧のファイルを読み、形を検査する（TASK-485 のポインタ）。
//!
//! 一覧は UTF-8 のテキストで、1 行が 1 入口。1 行は半角の空白 1 つで区切った 5 欄
//! `<rev> <path> <sha256> <mode> <entry>`。改行は LF で、最後の LF は付けても付けなくてもよい。
//! 後続の段が [`Manifest`] の各行からパイプラインを作る。ここは device に触れず、値の中身も持たない
//! （欄ごとの形だけを検査する）。失敗の文言は固定の英語と行の番号だけで、入力の値を含めない。

use std::fmt;
use std::fs;
use std::io::Read;
use std::path::Path;

use measure_metal_gemm_arm::source::{check_rev, check_source_path, parse_sha256_hex};

/// 一覧のファイルの最大バイト数。超えるものは読む前（と読んだ後）に拒否する。
pub const MAX_MANIFEST_BYTES: usize = 64 * 1024;
/// 一覧の最大行数。超えるものは行ごとの検査より前に拒否する。
pub const MAX_MANIFEST_LINES: usize = 1024;
/// 入口名の最大バイト数。
pub const MAX_ENTRY_BYTES: usize = 64;

/// 入口の種類。一覧の字句 `msl`・`specialized` に対応する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// 定数の特殊化を使わない入口。
    Msl,
    /// 定数の特殊化を使う入口。
    Specialized,
}

impl Mode {
    fn from_word(s: &str) -> Option<Self> {
        match s {
            "msl" => Some(Self::Msl),
            "specialized" => Some(Self::Specialized),
            _ => None,
        }
    }
}

/// 一覧の 1 行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestEntry {
    line: usize,
    path: String,
    sha256: [u8; 32],
    mode: Mode,
    entry: String,
}

impl ManifestEntry {
    /// 一覧の中の行の番号（1 始まり）。
    pub fn line(&self) -> usize {
        self.line
    }
    /// ソースのパス。
    pub fn path(&self) -> &str {
        &self.path
    }
    /// ソースの sha256。
    pub fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
    /// 入口の種類。
    pub fn mode(&self) -> Mode {
        self.mode
    }
    /// 入口名。
    pub fn entry(&self) -> &str {
        &self.entry
    }
}

/// 同じパスのソースと、そこに属する入口の行。
#[derive(Debug, Clone, Copy)]
pub struct ManifestFile<'a> {
    entries: &'a [ManifestEntry],
}

impl<'a> ManifestFile<'a> {
    /// ソースのパス。
    pub fn path(&self) -> &'a str {
        self.entries[0].path()
    }
    /// ソースの sha256。
    pub fn sha256(&self) -> &'a [u8; 32] {
        self.entries[0].sha256()
    }
    /// このソースに属する入口の行（一覧の順）。
    pub fn entries(&self) -> &'a [ManifestEntry] {
        self.entries
    }
}

/// 形の検査を通った一覧。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    rev: String,
    entries: Vec<ManifestEntry>,
}

impl Manifest {
    /// 全行に共通の rev。
    pub fn rev(&self) -> &str {
        &self.rev
    }
    /// 全行（一覧の順）。
    pub fn entries(&self) -> &[ManifestEntry] {
        &self.entries
    }
    /// パスの初出の順に、そのパスの行を並べる。同じパスの行は一覧の中で連続している。
    pub fn files(&self) -> Vec<ManifestFile<'_>> {
        let mut out = Vec::new();
        let mut start = 0;
        for i in 1..=self.entries.len() {
            if i == self.entries.len() || self.entries[i].path != self.entries[start].path {
                out.push(ManifestFile {
                    entries: &self.entries[start..i],
                });
                start = i;
            }
        }
        out
    }
    /// 入口名で行を引く。
    pub fn find(&self, entry: &str) -> Option<&ManifestEntry> {
        self.entries.iter().find(|e| e.entry == entry)
    }
    /// 種類ごとの行の数。
    pub fn count(&self, mode: Mode) -> usize {
        self.entries.iter().filter(|e| e.mode == mode).count()
    }
}

/// 一覧の検査の失敗の種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ManifestErrorKind {
    /// ファイルを読めない（通常のファイルでない・開けない・読み取りの失敗）。
    Unreadable,
    /// 大きさが上限を超える。
    TooLarge,
    /// 行数が上限を超える。
    TooManyLines,
    /// UTF-8 でない。
    NotUtf8,
    /// 空である。
    Empty,
    /// 空の行がある。
    EmptyLine,
    /// CR・タブなどの制御文字がある。
    ControlCharacter,
    /// 欄の数が 5 でない。
    FieldCount,
    /// rev の形が違う。
    Rev,
    /// パスの形が違う。
    Path,
    /// sha256 の形が違う。
    Sha256,
    /// 種類の字句が違う。
    Mode,
    /// 入口名の形が違う。
    Entry,
    /// rev が 1 行目と違う。
    RevMismatch,
    /// 同じパスで sha256 が違う。
    Sha256Mismatch,
    /// 同じパスの行が連続していない。
    PathNotContiguous,
    /// 違うパスで最後の成分（ファイル名）が重なる。
    FileNameCollision,
    /// 入口名が重なる。
    DuplicateEntry,
}

impl ManifestErrorKind {
    fn message(self) -> &'static str {
        match self {
            Self::Unreadable => "file is not readable",
            Self::TooLarge => "file is too large",
            Self::TooManyLines => "too many lines",
            Self::NotUtf8 => "not valid UTF-8",
            Self::Empty => "empty manifest",
            Self::EmptyLine => "empty line",
            Self::ControlCharacter => "control character",
            Self::FieldCount => "wrong number of fields",
            Self::Rev => "rev must be 40 lowercase hex digits",
            Self::Path => "path must be a plain relative path",
            Self::Sha256 => "sha256 must be 64 lowercase hex digits",
            Self::Mode => "unknown mode",
            Self::Entry => "invalid entry name",
            Self::RevMismatch => "rev differs from the first line",
            Self::Sha256Mismatch => "sha256 differs within the same path",
            Self::PathNotContiguous => "lines of one path are not contiguous",
            Self::FileNameCollision => "file name collides across paths",
            Self::DuplicateEntry => "duplicate entry name",
        }
    }
}

/// 一覧の検査の失敗。種類と（あれば）行の番号だけを持ち、入力の値は持たない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManifestError {
    kind: ManifestErrorKind,
    line: Option<usize>,
}

impl ManifestError {
    fn new(kind: ManifestErrorKind) -> Self {
        Self { kind, line: None }
    }
    fn at(kind: ManifestErrorKind, line: usize) -> Self {
        Self {
            kind,
            line: Some(line),
        }
    }
    /// 失敗の種類。
    pub fn kind(&self) -> ManifestErrorKind {
        self.kind
    }
    /// 失敗した行の番号（1 始まり）。行に依らない失敗では `None`。
    pub fn line(&self) -> Option<usize> {
        self.line
    }
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(n) => write!(f, "manifest line {n}: {}", self.kind.message()),
            None => write!(f, "manifest: {}", self.kind.message()),
        }
    }
}

impl std::error::Error for ManifestError {}

fn is_entry_name(s: &str) -> bool {
    let mut it = s.bytes();
    s.len() <= MAX_ENTRY_BYTES
        && matches!(it.next(), Some(b'A'..=b'Z' | b'a'..=b'z' | b'_'))
        && it.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// 一覧のテキストを検査して読む。device には触れない。
pub fn parse(text: &str) -> Result<Manifest, ManifestError> {
    use ManifestErrorKind as K;
    if text.len() > MAX_MANIFEST_BYTES {
        return Err(ManifestError::new(K::TooLarge));
    }
    if text.is_empty() {
        return Err(ManifestError::new(K::Empty));
    }
    let body = text.strip_suffix('\n').unwrap_or(text);
    if body.matches('\n').count() >= MAX_MANIFEST_LINES {
        return Err(ManifestError::new(K::TooManyLines));
    }

    let mut rev: Option<&str> = None;
    let mut entries: Vec<ManifestEntry> = Vec::new();
    for (i, raw) in body.split('\n').enumerate() {
        let n = i + 1;
        if raw.is_empty() {
            return Err(ManifestError::at(K::EmptyLine, n));
        }
        if raw.chars().any(char::is_control) {
            return Err(ManifestError::at(K::ControlCharacter, n));
        }
        let fields: Vec<&str> = raw.split(' ').collect();
        let [f_rev, f_path, f_sha, f_mode, f_entry] = fields[..] else {
            return Err(ManifestError::at(K::FieldCount, n));
        };
        check_rev(f_rev).map_err(|_| ManifestError::at(K::Rev, n))?;
        check_source_path(f_path).map_err(|_| ManifestError::at(K::Path, n))?;
        let sha256 = parse_sha256_hex(f_sha).map_err(|_| ManifestError::at(K::Sha256, n))?;
        let mode = Mode::from_word(f_mode).ok_or(ManifestError::at(K::Mode, n))?;
        if !is_entry_name(f_entry) {
            return Err(ManifestError::at(K::Entry, n));
        }

        match rev {
            None => rev = Some(f_rev),
            Some(first) if first != f_rev => return Err(ManifestError::at(K::RevMismatch, n)),
            Some(_) => {}
        }
        match entries.last() {
            Some(last) if last.path == f_path => {
                if last.sha256 != sha256 {
                    return Err(ManifestError::at(K::Sha256Mismatch, n));
                }
            }
            _ => {
                if entries.iter().any(|e| e.path == f_path) {
                    return Err(ManifestError::at(K::PathNotContiguous, n));
                }
                if entries
                    .iter()
                    .any(|e| file_name(&e.path) == file_name(f_path))
                {
                    return Err(ManifestError::at(K::FileNameCollision, n));
                }
            }
        }
        if entries.iter().any(|e| e.entry == f_entry) {
            return Err(ManifestError::at(K::DuplicateEntry, n));
        }
        entries.push(ManifestEntry {
            line: n,
            path: f_path.to_owned(),
            sha256,
            mode,
            entry: f_entry.to_owned(),
        });
    }
    // 空でない本文からは少なくとも 1 行が入るので、rev は必ず決まっている。
    let rev = rev.ok_or(ManifestError::new(K::Empty))?.to_owned();
    Ok(Manifest { rev, entries })
}

/// 一覧のファイルを読んで [`parse`] に渡す。通常のファイルだけを、上限つきで読む。
pub fn load(path: &Path) -> Result<Manifest, ManifestError> {
    use ManifestErrorKind as K;
    let meta = fs::metadata(path).map_err(|_| ManifestError::new(K::Unreadable))?;
    if !meta.is_file() {
        return Err(ManifestError::new(K::Unreadable));
    }
    if meta.len() > MAX_MANIFEST_BYTES as u64 {
        return Err(ManifestError::new(K::TooLarge));
    }
    let file = fs::File::open(path).map_err(|_| ManifestError::new(K::Unreadable))?;
    let opened = file
        .metadata()
        .map_err(|_| ManifestError::new(K::Unreadable))?;
    if !opened.is_file() {
        return Err(ManifestError::new(K::Unreadable));
    }
    let mut buf = Vec::new();
    file.take(MAX_MANIFEST_BYTES as u64 + 1)
        .read_to_end(&mut buf)
        .map_err(|_| ManifestError::new(K::Unreadable))?;
    if buf.len() > MAX_MANIFEST_BYTES {
        return Err(ManifestError::new(K::TooLarge));
    }
    let text = String::from_utf8(buf).map_err(|_| ManifestError::new(K::NotUtf8))?;
    parse(&text)
}
