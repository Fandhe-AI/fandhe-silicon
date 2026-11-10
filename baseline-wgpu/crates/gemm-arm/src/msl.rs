//! 受け取ったシェーダーのテキストの 1 行目の言語版と、入口の引数の束縛の番号を読む純粋関数
//! （spec TASK-1010 のポインタ）。
//!
//! [`crate::capture`] が返したテキストを呼び出し側が読むときに使う。device・コンパイラには触れず、
//! `unsafe` も書かない。検査は fail-closed で、形が 1 つでも外れたら値を持たない [`MslError`] を返す。

use std::fmt;

/// 束縛の番号の上限（この値未満だけを受ける）。
const SLOT_LIMIT: u32 = 31;

/// 読む束縛の名前 5 つ。`Slots` の並び（a・b・c・dims・sizes）と同じ。
const SLOT_NAMES: [&str; 5] = ["a", "b", "c", "dims", "_buffer_sizes"];

/// 束縛の属性の先頭の字句。
const BUFFER_ATTR: &str = "[[buffer(";

/// 1 行目の接頭辞。
const LANGUAGE_PREFIX: &str = "// language: metal";

/// 大きさの表の構造体の宣言の先頭。
const SIZES_STRUCT: &str = "struct _mslBufferSizes {";

/// [`language_version`]・[`buffer_slots`] の失敗。`Display` は値・テキストの断片・入口名を含まない固定の文。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MslError {
    /// 1 行目が言語版の形でない。
    LanguageLine,
    /// 入口名が識別子でない、または入口の宣言がちょうど 1 つでない。
    EntryPoint,
    /// 束縛の番号が 1〜2 桁の 10 進でない、後ろが閉じていない、または 31 以上。
    SlotNumber,
    /// 5 つ以外の名前の束縛がある。
    UnknownSlot,
    /// 5 つの名前のどれかが 2 つ以上ある。
    DuplicateSlot,
    /// 5 つの名前のどれかが無い。
    MissingSlot,
    /// 5 つの番号が互いに違わない。
    SlotCollision,
    /// 大きさの表の構造体が 3 行の形でない、またはちょうど 1 つでない。
    SizesStruct,
}

impl fmt::Display for MslError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::LanguageLine => "language line is malformed",
            Self::EntryPoint => "entry point declaration is not unique or malformed",
            Self::SlotNumber => "slot number is malformed or out of range",
            Self::UnknownSlot => "unexpected slot name",
            Self::DuplicateSlot => "slot name appears more than once",
            Self::MissingSlot => "required slot name is missing",
            Self::SlotCollision => "slot numbers are not distinct",
            Self::SizesStruct => "sizes struct is malformed or not unique",
        })
    }
}

impl std::error::Error for MslError {}

/// 入口の引数の束縛の番号 5 つ。[`buffer_slots`] だけが作る。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slots {
    a: u32,
    b: u32,
    c: u32,
    dims: u32,
    sizes: u32,
}

impl Slots {
    /// 引数 `a` の番号。
    pub fn a(&self) -> u32 {
        self.a
    }

    /// 引数 `b` の番号。
    pub fn b(&self) -> u32 {
        self.b
    }

    /// 引数 `c` の番号。
    pub fn c(&self) -> u32 {
        self.c
    }

    /// 引数 `dims` の番号。
    pub fn dims(&self) -> u32 {
        self.dims
    }

    /// 大きさの表の引数の番号。
    pub fn sizes(&self) -> u32 {
        self.sizes
    }
}

/// 1〜2 桁の ASCII 数字だけの列を値にする。それ以外は `None`。
fn small_decimal(digits: &str) -> Option<u32> {
    if digits.is_empty() || digits.len() > 2 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(
        digits
            .bytes()
            .fold(0, |acc, b| acc * 10 + u32::from(b - b'0')),
    )
}

/// 1 行目が `// language: metal<major>.<minor>`（各 1〜2 桁）なら `(major, minor)` を返す。
///
/// 1 行目は最初の改行まで。前後の空白・`\r`・余分な文字は拒否する。受ける版の判定はしない。
pub fn language_version(text: &str) -> Result<(u32, u32), MslError> {
    let line = text.split('\n').next().unwrap_or("");
    let rest = line
        .strip_prefix(LANGUAGE_PREFIX)
        .ok_or(MslError::LanguageLine)?;
    let (major, minor) = rest.split_once('.').ok_or(MslError::LanguageLine)?;
    match (small_decimal(major), small_decimal(minor)) {
        (Some(major), Some(minor)) => Ok((major, minor)),
        _ => Err(MslError::LanguageLine),
    }
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// 先頭が ASCII 英字か `_`、残りが ASCII 英数字か `_` の識別子か。
fn is_ident(s: &str) -> bool {
    let mut bytes = s.bytes();
    matches!(bytes.next(), Some(b) if b.is_ascii_alphabetic() || b == b'_')
        && bytes.all(is_ident_byte)
}

/// 入口 `entry` の引数の束縛の番号 5 つを読む。
///
/// 検査の順: 入口の形、束縛の読み取り（番号・名前）、名前の欠け、番号の重なり、大きさの表の構造体。
/// 最初に当たった失敗を返す。
pub fn buffer_slots(text: &str, entry: &str) -> Result<Slots, MslError> {
    if !is_ident(entry) {
        return Err(MslError::EntryPoint);
    }
    // コメントと文字列リテラルを空白に替えた同じ長さの文字列で、以降の走査をする（束縛の読み取りも入口の宣言の数えも、コメント・文字列の中は見ない）。
    let masked = mask_comments_and_strings(text);
    let text = masked.as_str();
    let decl = format!("kernel void {entry}(");
    if text.matches(decl.as_str()).count() != 1 || text.matches("kernel void ").count() != 1 {
        return Err(MslError::EntryPoint);
    }
    let bytes = text.as_bytes();
    // 指定した入口の引数リスト（宣言の `(` から対応する `)` の手前まで）だけを走査する。
    let args_start = text.find(decl.as_str()).ok_or(MslError::EntryPoint)? + decl.len();
    let mut depth = 1usize;
    let mut args_end = None;
    for (i, b) in bytes[args_start..].iter().enumerate() {
        match b {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    args_end = Some(args_start + i);
                    break;
                }
            }
            _ => {}
        }
    }
    let args_end = args_end.ok_or(MslError::EntryPoint)?;
    let args = &text[args_start..args_end];
    let bytes = args.as_bytes();
    let mut found: [Option<u32>; 5] = [None; 5];
    for (at, _) in args.match_indices("[[") {
        // `[[` の後ろの空白を飛ばして `buffer` で始まるものを束縛の属性とみなす。厳密な `[[buffer(` の形でなければ拒否する。
        let after = args[at + 2..].trim_start();
        let is_buffer = after
            .strip_prefix("buffer")
            .is_some_and(|r| !r.bytes().next().is_some_and(is_ident_byte));
        if !is_buffer {
            continue;
        }
        if !args[at..].starts_with(BUFFER_ATTR) {
            return Err(MslError::SlotNumber);
        }
        let digits_start = at + BUFFER_ATTR.len();
        let mut end = digits_start;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        // 数字は ASCII なので、境界はどちらも char の境界。
        let number = small_decimal(&args[digits_start..end]).ok_or(MslError::SlotNumber)?;
        if !args[end..].starts_with(")]]") || number >= SLOT_LIMIT {
            return Err(MslError::SlotNumber);
        }

        let mut name_end = at;
        while name_end > 0 && bytes[name_end - 1].is_ascii_whitespace() {
            name_end -= 1;
        }
        let mut name_start = name_end;
        while name_start > 0 && is_ident_byte(bytes[name_start - 1]) {
            name_start -= 1;
        }
        let name = &args[name_start..name_end];
        let index = SLOT_NAMES
            .iter()
            .position(|known| *known == name)
            .ok_or(MslError::UnknownSlot)?;
        if found[index].is_some() {
            return Err(MslError::DuplicateSlot);
        }
        found[index] = Some(number);
    }

    let [Some(a), Some(b), Some(c), Some(dims), Some(sizes)] = found else {
        return Err(MslError::MissingSlot);
    };
    let all = [a, b, c, dims, sizes];
    for (i, x) in all.iter().enumerate() {
        if all[i + 1..].contains(x) {
            return Err(MslError::SlotCollision);
        }
    }

    check_sizes_struct(text)?;
    Ok(Slots {
        a,
        b,
        c,
        dims,
        sizes,
    })
}

/// `//` の行コメント・`/* */` のブロックコメント・`"` の文字列リテラルを空白に替える。
///
/// 各文字を同じバイト数の空白に替えるので長さと位置は保たれ、改行は残す。閉じていないものは末尾まで替える。
fn mask_comments_and_strings(text: &str) -> String {
    #[derive(Clone, Copy, PartialEq)]
    enum State {
        Code,
        Line,
        Block,
        Str,
    }
    let mut out = String::with_capacity(text.len());
    let mut state = State::Code;
    let mut chars = text.chars().peekable();
    let blank = |out: &mut String, c: char| {
        if c == '\n' {
            out.push('\n');
        } else {
            out.extend(std::iter::repeat_n(' ', c.len_utf8()));
        }
    };
    while let Some(c) = chars.next() {
        match state {
            State::Code => match (c, chars.peek().copied()) {
                ('/', Some('/')) => {
                    chars.next();
                    out.push_str("  ");
                    state = State::Line;
                }
                ('/', Some('*')) => {
                    chars.next();
                    out.push_str("  ");
                    state = State::Block;
                }
                ('"', _) => {
                    out.push(' ');
                    state = State::Str;
                }
                _ => out.push(c),
            },
            State::Line => {
                if c == '\n' {
                    state = State::Code;
                }
                blank(&mut out, c);
            }
            State::Block => {
                if c == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    out.push_str("  ");
                    state = State::Code;
                } else {
                    blank(&mut out, c);
                }
            }
            State::Str => {
                if c == '\\' {
                    blank(&mut out, c);
                    if let Some(n) = chars.next() {
                        blank(&mut out, n);
                    }
                } else {
                    if c == '"' || c == '\n' {
                        state = State::Code;
                    }
                    blank(&mut out, c);
                }
            }
        }
    }
    out
}

/// 大きさの表の構造体がちょうど 1 つで、本体が 3 行の形かを確かめる。
fn check_sizes_struct(text: &str) -> Result<(), MslError> {
    let masked = mask_comments_and_strings(text);
    let text = masked.as_str();
    if text.matches(SIZES_STRUCT).count() != 1 {
        return Err(MslError::SizesStruct);
    }
    let start = text.find(SIZES_STRUCT).ok_or(MslError::SizesStruct)? + SIZES_STRUCT.len();
    let body = text[start..]
        .split_once('}')
        .ok_or(MslError::SizesStruct)?
        .0;
    let lines: Vec<&str> = body
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if lines == ["uint size0;", "uint size1;", "uint size2;"] {
        Ok(())
    } else {
        Err(MslError::SizesStruct)
    }
}
