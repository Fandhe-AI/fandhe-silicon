//! run ごとの記録を読むための厳格な JSON の読み込み（標準ライブラリだけ）。
//!
//! `runs`（run 間の集計）とコマンド `across-runs` から呼ばれる。書く側の `json` と対になるが、
//! こちらは信頼しない入力を受けるので fail-closed にする。重複キー・JSON に無い定数（`NaN` など）・
//! 末尾のゴミ・制御文字・不正なエスケープ・非 ASCII を拒否し、入力の大きさと入れ子の深さに上限を置く。
//! 数は字句のまま持ち、`f64` を経由しない。失敗の種類だけを返し、入力の値や位置は埋め込まない。

use std::collections::HashSet;
use std::fmt;

/// 読み込める入力の上限（バイト）。
pub const MAX_BYTES: usize = 8 * 1024 * 1024;

/// 入れ子の深さの上限。
pub const MAX_DEPTH: usize = 64;

/// 読み込みの失敗の種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReadError {
    /// 入力が上限（[`MAX_BYTES`]）を超えている。
    TooLarge,
    /// 入れ子が上限（[`MAX_DEPTH`]）を超えている。
    TooDeep,
    /// JSON の文法に合わない（末尾のゴミ・不正な数・不正なエスケープ・制御文字を含む）。
    Syntax,
    /// 非 ASCII のバイトを含む。
    NonAscii,
    /// object に同じキーが 2 回ある。
    DuplicateKey,
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::TooLarge => "input too large",
            Self::TooDeep => "nesting too deep",
            Self::Syntax => "invalid json",
            Self::NonAscii => "non-ascii input",
            Self::DuplicateKey => "duplicate key",
        })
    }
}

impl std::error::Error for ReadError {}

/// 読み込んだ JSON の値。数は字句のまま持つ。
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// `null`。
    Null,
    /// `true` / `false`。
    Bool(bool),
    /// 数の字句（検証済み）。
    Num(String),
    /// 文字列（エスケープ解決後）。
    Str(String),
    /// 配列。
    Array(Vec<Value>),
    /// object。出てきた順を保つ。
    Object(Vec<(String, Value)>),
}

impl Value {
    /// object のキーの値。object でない・キーが無ければ `None`。
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Self::Object(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// 文字列の中身。
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }

    /// 配列の要素。
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Self::Array(a) => Some(a),
            _ => None,
        }
    }

    /// 符号・小数・指数のない `u64`（先頭 0 は `0` 単独だけ）。溢れれば `None`。
    pub fn as_u64(&self) -> Option<u64> {
        let Self::Num(s) = self else { return None };
        parse_u64(s)
    }

    /// 整数または「整数 + .5」を 1/4 単位の整数（4 倍）にして返す。それ以外・溢れは `None`。
    pub fn as_quarter_units(&self) -> Option<u64> {
        let Self::Num(s) = self else { return None };
        match s.split_once('.') {
            None => parse_u64(s)?.checked_mul(4),
            Some((whole, "5")) => parse_u64(whole)?.checked_mul(4)?.checked_add(2),
            Some(_) => None,
        }
    }
}

fn parse_u64(s: &str) -> Option<u64> {
    let b = s.as_bytes();
    if b.is_empty() || !b.iter().all(u8::is_ascii_digit) || (b.len() > 1 && b[0] == b'0') {
        return None;
    }
    b.iter().try_fold(0u64, |acc, &d| {
        acc.checked_mul(10)?.checked_add(u64::from(d - b'0'))
    })
}

/// JSON の文書 1 つを読む。前後の空白（空白・タブ・改行）は許し、それ以外の余りは拒否する。
pub fn parse(input: &[u8]) -> Result<Value, ReadError> {
    if input.len() > MAX_BYTES {
        return Err(ReadError::TooLarge);
    }
    if !input.is_ascii() {
        return Err(ReadError::NonAscii);
    }
    let mut p = Parser { b: input, i: 0 };
    p.ws();
    let v = p.value(0)?;
    p.ws();
    if p.i == p.b.len() {
        Ok(v)
    } else {
        Err(ReadError::Syntax)
    }
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.i += 1;
        }
    }

    fn eat(&mut self, c: u8) -> Result<(), ReadError> {
        if self.peek() == Some(c) {
            self.i += 1;
            Ok(())
        } else {
            Err(ReadError::Syntax)
        }
    }

    fn lit(&mut self, word: &[u8], v: Value) -> Result<Value, ReadError> {
        if self.b.get(self.i..self.i + word.len()) == Some(word) {
            self.i += word.len();
            Ok(v)
        } else {
            Err(ReadError::Syntax)
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, ReadError> {
        match self.peek().ok_or(ReadError::Syntax)? {
            b'{' | b'[' if depth >= MAX_DEPTH => Err(ReadError::TooDeep),
            b'{' => self.object(depth),
            b'[' => self.array(depth),
            b'"' => Ok(Value::Str(self.string()?)),
            b't' => self.lit(b"true", Value::Bool(true)),
            b'f' => self.lit(b"false", Value::Bool(false)),
            b'n' => self.lit(b"null", Value::Null),
            b'-' | b'0'..=b'9' => self.number(),
            _ => Err(ReadError::Syntax),
        }
    }

    fn digits(&mut self) -> usize {
        let start = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.i += 1;
        }
        self.i - start
    }

    fn number(&mut self) -> Result<Value, ReadError> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        match self.peek() {
            Some(b'0') => self.i += 1,
            Some(b'1'..=b'9') => {
                self.digits();
            }
            _ => return Err(ReadError::Syntax),
        }
        if self.peek() == Some(b'.') {
            self.i += 1;
            if self.digits() == 0 {
                return Err(ReadError::Syntax);
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.i += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.i += 1;
            }
            if self.digits() == 0 {
                return Err(ReadError::Syntax);
            }
        }
        let s = self.b.get(start..self.i).ok_or(ReadError::Syntax)?;
        let s = std::str::from_utf8(s).map_err(|_| ReadError::Syntax)?;
        Ok(Value::Num(s.to_owned()))
    }

    fn hex4(&mut self) -> Result<u32, ReadError> {
        let h = self.b.get(self.i..self.i + 4).ok_or(ReadError::Syntax)?;
        let mut n = 0u32;
        for &c in h {
            let d = char::from(c).to_digit(16).ok_or(ReadError::Syntax)?;
            n = n * 16 + d;
        }
        self.i += 4;
        Ok(n)
    }

    fn string(&mut self) -> Result<String, ReadError> {
        self.eat(b'"')?;
        let mut out = String::new();
        loop {
            let c = self.peek().ok_or(ReadError::Syntax)?;
            self.i += 1;
            match c {
                b'"' => return Ok(out),
                0..=0x1f => return Err(ReadError::Syntax),
                b'\\' => {
                    let e = self.peek().ok_or(ReadError::Syntax)?;
                    self.i += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hi = self.hex4()?;
                            let cp = if (0xd800..0xdc00).contains(&hi) {
                                self.eat(b'\\')?;
                                self.eat(b'u')?;
                                let lo = self.hex4()?;
                                if !(0xdc00..0xe000).contains(&lo) {
                                    return Err(ReadError::Syntax);
                                }
                                0x10000 + ((hi - 0xd800) << 10) + (lo - 0xdc00)
                            } else {
                                hi
                            };
                            out.push(char::from_u32(cp).ok_or(ReadError::Syntax)?);
                        }
                        _ => return Err(ReadError::Syntax),
                    }
                }
                c => out.push(char::from(c)),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value, ReadError> {
        self.eat(b'[')?;
        let mut items = Vec::new();
        self.ws();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Ok(Value::Array(items));
        }
        loop {
            self.ws();
            items.push(self.value(depth + 1)?);
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(ReadError::Syntax),
            }
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, ReadError> {
        self.eat(b'{')?;
        let mut members: Vec<(String, Value)> = Vec::new();
        // 重複キーの検出を平均 O(1) にする（既存キーとの線形比較だと O(n^2) になるため）
        let mut seen: HashSet<String> = HashSet::new();
        self.ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(Value::Object(members));
        }
        loop {
            self.ws();
            let key = self.string()?;
            if !seen.insert(key.clone()) {
                return Err(ReadError::DuplicateKey);
            }
            self.ws();
            self.eat(b':')?;
            self.ws();
            members.push((key, self.value(depth + 1)?));
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Value::Object(members));
                }
                _ => return Err(ReadError::Syntax),
            }
        }
    }
}
