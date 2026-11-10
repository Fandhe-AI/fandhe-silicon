//! 処理の間に `log` facade へ流れる Debug の記録から、入口名を持つシェーダーのテキストを 1 つ受け取る口
//! （spec TASK-1010 のポインタ）。
//!
//! 腕の device の経路が pipeline を作る間の記録を、同じプロセスの中で受け取るために使う。
//! 受け取ったテキストはここでは解釈せず、呼び出し側へ返すだけ。`unsafe` は書かない。
//!
//! 数える記録は、処理の間の Debug の記録のうち、本文の 1 行目に単引用符で囲んだ入口名を持ち、
//! 2 行目以降が `// language: metal` で始まるもの。ほかの記録は保持しない。

use std::fmt::{self, Write as _};
use std::sync::{Mutex, OnceLock, PoisonError};

use log::{LevelFilter, Log, Metadata, Record};

/// 受け取る 1 件のテキストの上限（バイト）。
pub const MAX_SHADER_TEXT_BYTES: usize = 1024 * 1024;

/// 整形した本文を溜める上限。1 行目の長さの余裕（4096 バイト）を上限に足し、
/// 1 行目が余裕に収まる記録では、2 行目以降が上限を超えたことを見分けられる。
const MESSAGE_CAP: usize = MAX_SHADER_TEXT_BYTES + 4098;

/// 数える記録の 2 行目以降の先頭の字句。
const LANGUAGE_MARK: &str = "// language: metal";

/// [`capture_one`] の失敗。`Display` は入口名・本文を含まない固定の文。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CaptureError {
    /// ほかの記録の受け口が既に設置されている。
    LoggerTaken,
    /// 数える記録が 0 件だった。
    NotFound,
    /// 数える記録が 2 件以上だった。
    Ambiguous,
    /// 数える記録が 1 件で、テキストが [`MAX_SHADER_TEXT_BYTES`] を超えた。
    TooLarge,
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::LoggerTaken => "another logger is already installed",
            Self::NotFound => "no matching shader record was captured",
            Self::Ambiguous => "more than one matching shader record was captured",
            Self::TooLarge => "captured shader record exceeds the size limit",
        })
    }
}

impl std::error::Error for CaptureError {}

/// 記録の本文が数える記録なら、2 行目以降を返す。
///
/// 最初の改行で分け、1 行目が `'<entry>'` を含み、2 行目以降が `// language: metal` で始まるときだけ `Some`。
/// 改行が無い・入口名が空・2 行目が空などは `None`。[`capture_one`] の判定に使う。
pub fn shader_text<'a>(message: &'a str, entry: &str) -> Option<&'a str> {
    if entry.is_empty() {
        return None;
    }
    let (first, rest) = message.split_once('\n')?;
    let quoted = format!("'{entry}'");
    if first.contains(&quoted) && rest.starts_with(LANGUAGE_MARK) {
        Some(rest)
    } else {
        None
    }
}

/// 受け取りの状態。`capture_one` の間だけ `Some`。
struct Capturing {
    entry: &'static str,
    text: Option<String>,
    count: usize,
    too_large: bool,
}

static STATE: Mutex<Option<Capturing>> = Mutex::new(None);
static SERIAL: Mutex<()> = Mutex::new(());
static INSTALLED: OnceLock<bool> = OnceLock::new();

/// [`MESSAGE_CAP`] バイトで打ち切って溜める書き手。
struct Bounded {
    buf: String,
}

impl fmt::Write for Bounded {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let room = MESSAGE_CAP.saturating_sub(self.buf.len());
        if s.len() <= room {
            self.buf.push_str(s);
        } else {
            let mut cut = room;
            while !s.is_char_boundary(cut) {
                cut -= 1;
            }
            self.buf.push_str(&s[..cut]);
        }
        Ok(())
    }
}

/// `log` facade の受け口。受け取りが有効なときだけ Debug の記録を判定する。
struct Sink;

static SINK: Sink = Sink;

impl Log for Sink {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() == log::Level::Debug
    }

    fn log(&self, record: &Record<'_>) {
        if record.level() != log::Level::Debug {
            return;
        }
        let entry = {
            let state = STATE.lock().unwrap_or_else(PoisonError::into_inner);
            match state.as_ref() {
                Some(c) => c.entry,
                None => return,
            }
        };
        // 整形は STATE の lock の外で行う（整形の中でほかの記録が出ても詰まらないように）。
        let mut w = Bounded { buf: String::new() };
        if write!(w, "{}", record.args()).is_err() {
            return;
        }
        let Some(text) = shader_text(&w.buf, entry) else {
            return;
        };
        let mut state = STATE.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(c) = state.as_mut() {
            c.count += 1;
            if c.count == 1 {
                if text.len() > MAX_SHADER_TEXT_BYTES {
                    c.too_large = true;
                } else {
                    c.text = Some(text.to_owned());
                }
            }
        }
    }

    fn flush(&self) {}
}

/// `f` の終わり（panic を含む）で受け取りを止め、最大の level を `Off` に戻す。
struct StopGuard;

impl Drop for StopGuard {
    fn drop(&mut self) {
        log::set_max_level(LevelFilter::Off);
        *STATE.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }
}

/// `f` を呼ぶ間の Debug の記録から、入口名 `entry` のシェーダーのテキストをちょうど 1 つ受け取る。
///
/// 成功なら `f` の戻り値と受け取ったテキスト（2 行目以降）を返す。判定の順は、数える記録が
/// 0 件で `NotFound`、2 件以上で `Ambiguous`、1 件で上限超えなら `TooLarge`、1 件なら `Ok`。
/// ほかの記録の受け口が既にあるときは `f` を呼ばずに `LoggerTaken`。
///
/// `f` の間は最大の level を Debug にし、終わったら（panic でも）`Off` に戻す。
/// process 全体で直列に動き、`f` の間はほかのスレッドが出した記録も数える。
pub fn capture_one<R>(
    entry: &'static str,
    f: impl FnOnce() -> R,
) -> Result<(R, String), CaptureError> {
    let _serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    let installed = *INSTALLED.get_or_init(|| log::set_logger(&SINK).is_ok());
    if !installed {
        return Err(CaptureError::LoggerTaken);
    }
    *STATE.lock().unwrap_or_else(PoisonError::into_inner) = Some(Capturing {
        entry,
        text: None,
        count: 0,
        too_large: false,
    });
    log::set_max_level(LevelFilter::Debug);
    let guard = StopGuard;
    let result = f();
    let captured = STATE.lock().unwrap_or_else(PoisonError::into_inner).take();
    drop(guard);
    let Some(c) = captured else {
        return Err(CaptureError::NotFound);
    };
    match (c.count, c.too_large, c.text) {
        (0, _, _) => Err(CaptureError::NotFound),
        (1, true, _) => Err(CaptureError::TooLarge),
        (1, false, Some(text)) => Ok((result, text)),
        (1, false, None) => Err(CaptureError::NotFound),
        _ => Err(CaptureError::Ambiguous),
    }
}
