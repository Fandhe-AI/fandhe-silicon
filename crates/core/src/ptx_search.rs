//! PTX のテキストを関数（本文のある `.entry`・`.func`）ごとに分け、命令の opcode に名前つきのパターンを当てて件数を数える純粋関数の置き場。
//!
//! 入力は文字列とパターンの列だけで、ファイル・環境変数・device は読まない。
//! 字句は自前で読み、コメントと文字列の中身は空白に置き換えてから関数・文・命令に分ける。
//! 閉じない括弧や名前のない関数など、読み切れない入力は `Err` で返し、件数 0 の `Ok` と区別する。
//!
//! feature `test-support` の裏にだけあり、試験が使う。製品の経路からは使わない。
//! ポインタ: `docs/spec/05-tasks.md` の TASK-100。

use std::collections::BTreeMap;
use std::fmt;

/// opcode 1 つに当てる規則。opcode を `.` で分け、最初の部分を根、残りを部分として照らす。
///
/// 次の 3 つがすべて成り立つと当たる。部分の一致は語ごとの完全一致で、部分文字列では当てない。
/// - `root` が `Some` なら根と一致する。
/// - `all_parts` の語がすべて部分の中にある。
/// - `any_parts` が空でなければ、そのどれかが部分の中にある。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpcodeRule {
    /// 根の指定。`None` なら根を問わない。
    pub root: Option<&'static str>,
    /// すべてが部分の中に要る語。
    pub all_parts: &'static [&'static str],
    /// 空でなければ、どれか 1 つが部分の中に要る語。
    pub any_parts: &'static [&'static str],
}

impl OpcodeRule {
    /// opcode がこの規則に当たるか。[`search`] が命令ごとに呼ぶ。
    fn matches(&self, opcode: &str) -> bool {
        let mut it = opcode.split('.');
        let root = it.next().unwrap_or("");
        let parts: Vec<&str> = it.collect();
        if let Some(r) = self.root
            && r != root
        {
            return false;
        }
        if !self.all_parts.iter().all(|w| parts.contains(w)) {
            return false;
        }
        self.any_parts.is_empty() || self.any_parts.iter().any(|w| parts.contains(w))
    }
}

/// 名前つきのパターン。どれか 1 つの規則に当たる命令を 1 件と数える（1 つの命令は 1 つのパターンの中で 1 回だけ数える）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstructionPattern {
    /// 結果で引くための名前。1 回の [`search`] の中で重複しない。
    pub id: &'static str,
    /// 当てる規則。
    pub rules: &'static [OpcodeRule],
}

/// 部分に `f16` か `f16x2` を持つ opcode に当たるパターン（根は問わない）。
pub const F16_INSTRUCTIONS: InstructionPattern = InstructionPattern {
    id: "f16",
    rules: &[OpcodeRule {
        root: None,
        all_parts: &[],
        any_parts: &["f16", "f16x2"],
    }],
};

/// 根が `shfl`・`redux`・`vote` の opcode に当たるパターン。
pub const SUBGROUP_INSTRUCTIONS: InstructionPattern = InstructionPattern {
    id: "subgroup",
    rules: &[
        OpcodeRule {
            root: Some("shfl"),
            all_parts: &[],
            any_parts: &[],
        },
        OpcodeRule {
            root: Some("redux"),
            all_parts: &[],
            any_parts: &[],
        },
        OpcodeRule {
            root: Some("vote"),
            all_parts: &[],
            any_parts: &[],
        },
    ],
};

/// 根が `atom` か `red` で、部分に `add` と `f32` を持つ opcode に当たるパターン。
pub const F32_ATOMIC_ADD_INSTRUCTIONS: InstructionPattern = InstructionPattern {
    id: "f32-atomic-add",
    rules: &[
        OpcodeRule {
            root: Some("atom"),
            all_parts: &["add", "f32"],
            any_parts: &[],
        },
        OpcodeRule {
            root: Some("red"),
            all_parts: &["add", "f32"],
            any_parts: &[],
        },
    ],
};

/// 根が `mma` の opcode と、根が `wmma` で部分に `mma` を持つ opcode に当たるパターン。
///
/// ポインタ: `docs/spec/04-behavior/` の ART-22。
pub const MATRIX_MMA_INSTRUCTIONS: InstructionPattern = InstructionPattern {
    id: "matrix-mma",
    rules: &[
        OpcodeRule {
            root: Some("mma"),
            all_parts: &[],
            any_parts: &[],
        },
        OpcodeRule {
            root: Some("wmma"),
            all_parts: &["mma"],
            any_parts: &[],
        },
    ],
};

/// 組み込みの 4 つのパターン（`f16`・`subgroup`・`f32-atomic-add`・`matrix-mma` の順）。
///
/// ポインタ: `docs/spec/04-behavior/` の FB-21。
pub const BUILTIN_PATTERNS: [InstructionPattern; 4] = [
    F16_INSTRUCTIONS,
    SUBGROUP_INSTRUCTIONS,
    F32_ATOMIC_ADD_INSTRUCTIONS,
    MATRIX_MMA_INSTRUCTIONS,
];

/// 関数の種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitKind {
    /// `.entry`。
    Entry,
    /// `.func`。
    Func,
}

/// 1 つのパターンの件数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternCount {
    /// パターンの名前。
    pub id: &'static str,
    /// 当たった命令の件数。
    pub count: u64,
    /// 当たった opcode の字句ごとの件数（opcode の昇順）。
    pub opcodes: Vec<(String, u64)>,
}

/// 本文のある関数 1 つの結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitReport {
    /// 関数の種類。
    pub kind: UnitKind,
    /// 関数の名前。
    pub name: String,
    /// 本文の命令の数。
    pub instructions: u64,
    /// 渡したパターンの順の件数。
    pub counts: Vec<PatternCount>,
}

/// [`search`] の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchReport {
    /// 本文のある関数。出現の順。
    pub units: Vec<UnitReport>,
    /// 本文の無い `.entry`・`.func` の名前。出現の順。
    pub declarations: Vec<String>,
}

impl SearchReport {
    /// 全関数のその id の件数の和。どの関数にもその id が無いとき、和があふれたときは `None`。
    pub fn total(&self, id: &str) -> Option<u64> {
        let mut seen = false;
        let mut sum: u64 = 0;
        for unit in &self.units {
            for c in unit.counts.iter().filter(|c| c.id == id) {
                seen = true;
                sum = sum.checked_add(c.count)?;
            }
        }
        seen.then_some(sum)
    }
}

/// [`search`] が `Err` で返す入力の種類。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PtxSearchError {
    /// パターンの列が空。
    EmptyPatterns,
    /// パターンの id が重複する。
    DuplicatePatternId,
    /// `/*` が閉じない。
    UnterminatedBlockComment,
    /// `"` が閉じない。
    UnterminatedString,
    /// `{` が閉じない、関数の外に余分な `}` がある、または関数の頭の後に `{` も `;` も無いまま入力が終わる。
    UnbalancedBraces,
    /// `.entry`・`.func` の後に名前が無い。
    MissingFunctionName,
    /// 本文のある関数が同じ名前で 2 つある。
    DuplicateFunctionName,
    /// 本文のある `.entry` が 1 つも無い。
    NoEntry,
    /// 件数の加算があふれる。
    CountOverflow,
}

impl PtxSearchError {
    /// 固定の英語の文言。入力の値は入らない。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EmptyPatterns => "pattern list is empty",
            Self::DuplicatePatternId => "pattern id is duplicated",
            Self::UnterminatedBlockComment => "block comment is not terminated",
            Self::UnterminatedString => "string literal is not terminated",
            Self::UnbalancedBraces => "braces are unbalanced",
            Self::MissingFunctionName => "function name is missing",
            Self::DuplicateFunctionName => "function name is duplicated",
            Self::NoEntry => "no entry function with a body",
            Self::CountOverflow => "count overflowed",
        }
    }
}

impl fmt::Display for PtxSearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::error::Error for PtxSearchError {}

/// PTX のテキストを関数ごとに分け、パターンごとに命令の件数を数える。
///
/// 先にパターンの列を検査し（空・id の重複）、その後に字句を読む。
/// 読み切れない入力は `Err` で返し、件数 0 の `Ok` にしない。
pub fn search(ptx: &str, patterns: &[InstructionPattern]) -> Result<SearchReport, PtxSearchError> {
    if patterns.is_empty() {
        return Err(PtxSearchError::EmptyPatterns);
    }
    for (i, p) in patterns.iter().enumerate() {
        if patterns.iter().take(i).any(|q| q.id == p.id) {
            return Err(PtxSearchError::DuplicatePatternId);
        }
    }
    let text = blank_comments_and_strings(ptx.as_bytes())?;
    let mut units: Vec<UnitReport> = Vec::new();
    let mut declarations: Vec<String> = Vec::new();
    let mut p: usize = 0;
    while p < text.len() {
        let rest = text.get(p..).unwrap_or(&[]);
        let Some(off) = rest.iter().position(|b| matches!(b, b';' | b'{' | b'}')) else {
            if find_unit_word(rest).is_some() {
                return Err(PtxSearchError::UnbalancedBraces);
            }
            break;
        };
        let end = p + off;
        let delim = text.get(end).copied().unwrap_or(b';');
        let stmt = text.get(p..end).unwrap_or(&[]);
        let found = find_unit_word(stmt);
        match (delim, found) {
            (b'}', _) => return Err(PtxSearchError::UnbalancedBraces),
            (b';', None) => p = end + 1,
            (b';', Some((kind, after))) => {
                let name = read_name(stmt, after, kind)?;
                declarations.push(name);
                p = end + 1;
            }
            (_, None) => {
                let close = matching_close(&text, end + 1)?;
                p = close + 1;
                let tail = text.get(p..).unwrap_or(&[]);
                let skipped = tail.iter().take_while(|b| b.is_ascii_whitespace()).count();
                if tail.get(skipped) == Some(&b';') {
                    p += skipped + 1;
                }
            }
            (_, Some((kind, after))) => {
                let name = read_name(stmt, after, kind)?;
                let close = matching_close(&text, end + 1)?;
                let body = text.get(end + 1..close).unwrap_or(&[]);
                if units.iter().any(|u| u.name == name) {
                    return Err(PtxSearchError::DuplicateFunctionName);
                }
                units.push(count_body(kind, name, body, patterns)?);
                p = close + 1;
            }
        }
    }
    if !units.iter().any(|u| u.kind == UnitKind::Entry) {
        return Err(PtxSearchError::NoEntry);
    }
    Ok(SearchReport {
        units,
        declarations,
    })
}

/// コメントと文字列を字ごとに空白へ置き換える（改行は残す）。
fn blank_comments_and_strings(src: &[u8]) -> Result<Vec<u8>, PtxSearchError> {
    let blank = |c: u8| if c == b'\n' { b'\n' } else { b' ' };
    let mut out = Vec::with_capacity(src.len());
    let mut i = 0;
    while let Some(&b) = src.get(i) {
        let next = src.get(i + 1).copied();
        if b == b'/' && next == Some(b'/') {
            while let Some(&c) = src.get(i) {
                if c == b'\n' {
                    break;
                }
                out.push(b' ');
                i += 1;
            }
        } else if b == b'/' && next == Some(b'*') {
            out.extend_from_slice(b"  ");
            i += 2;
            loop {
                match (src.get(i), src.get(i + 1)) {
                    (Some(b'*'), Some(b'/')) => {
                        out.extend_from_slice(b"  ");
                        i += 2;
                        break;
                    }
                    (Some(&c), _) => {
                        out.push(blank(c));
                        i += 1;
                    }
                    (None, _) => return Err(PtxSearchError::UnterminatedBlockComment),
                }
            }
        } else if b == b'"' {
            out.push(b' ');
            i += 1;
            loop {
                match src.get(i) {
                    Some(b'"') => {
                        out.push(b' ');
                        i += 1;
                        break;
                    }
                    Some(b'\\') => {
                        out.push(b' ');
                        i += 1;
                        if let Some(&c) = src.get(i) {
                            out.push(blank(c));
                            i += 1;
                        }
                    }
                    Some(&c) => {
                        out.push(blank(c));
                        i += 1;
                    }
                    None => return Err(PtxSearchError::UnterminatedString),
                }
            }
        } else {
            out.push(b);
            i += 1;
        }
    }
    Ok(out)
}

/// `start` から数えて深さ 1 の `{` に対応する `}` の位置。
fn matching_close(text: &[u8], start: usize) -> Result<usize, PtxSearchError> {
    let mut depth: usize = 1;
    for (i, b) in text.get(start..).unwrap_or(&[]).iter().enumerate() {
        match b {
            b'{' => {
                depth = depth
                    .checked_add(1)
                    .ok_or(PtxSearchError::UnbalancedBraces)?
            }
            b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Ok(start + i);
                }
            }
            _ => {}
        }
    }
    Err(PtxSearchError::UnbalancedBraces)
}

fn is_word_sep(b: u8) -> bool {
    b.is_ascii_whitespace() || matches!(b, b'(' | b')' | b',')
}

/// 文の中の最初の `.entry`・`.func` の語。種類と語の終わりの位置を返す。
fn find_unit_word(stmt: &[u8]) -> Option<(UnitKind, usize)> {
    let mut i = 0;
    while i < stmt.len() {
        while stmt.get(i).is_some_and(|&b| is_word_sep(b)) {
            i += 1;
        }
        let start = i;
        while stmt.get(i).is_some_and(|&b| !is_word_sep(b)) {
            i += 1;
        }
        match stmt.get(start..i) {
            Some(b".entry") => return Some((UnitKind::Entry, i)),
            Some(b".func") => return Some((UnitKind::Func, i)),
            _ => {}
        }
    }
    None
}

/// 語の後ろから関数の名前を読む。`.func` の戻りの `( … )` は飛ばす。
fn read_name(stmt: &[u8], mut i: usize, kind: UnitKind) -> Result<String, PtxSearchError> {
    let skip_ws = |mut i: usize| {
        while stmt.get(i).is_some_and(|b| b.is_ascii_whitespace()) {
            i += 1;
        }
        i
    };
    i = skip_ws(i);
    if kind == UnitKind::Func && stmt.get(i) == Some(&b'(') {
        let mut depth: usize = 0;
        loop {
            match stmt.get(i) {
                Some(b'(') => depth += 1,
                Some(b')') => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        i += 1;
                        break;
                    }
                }
                Some(_) => {}
                None => return Err(PtxSearchError::MissingFunctionName),
            }
            i += 1;
        }
        i = skip_ws(i);
    }
    let start = i;
    while stmt
        .get(i)
        .is_some_and(|&b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'$' | b'%'))
    {
        i += 1;
    }
    if start == i {
        return Err(PtxSearchError::MissingFunctionName);
    }
    Ok(String::from_utf8_lossy(stmt.get(start..i).unwrap_or(&[])).into_owned())
}

/// 文の頭の空白・括弧・ラベル・述語・`.loc`/`.file` を外し、命令の opcode を返す（指示なら `None`）。
fn opcode_of(stmt: &[u8]) -> Option<String> {
    let mut i = 0;
    loop {
        while stmt
            .get(i)
            .is_some_and(|&b| b.is_ascii_whitespace() || b == b'{' || b == b'}')
        {
            i += 1;
        }
        let rest = stmt.get(i..).unwrap_or(&[]);
        if rest.first() == Some(&b'@') {
            i += rest.iter().take_while(|b| !b.is_ascii_whitespace()).count();
            continue;
        }
        let ident = rest
            .iter()
            .take_while(|&&b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'$'))
            .count();
        if ident > 0 && rest.get(ident) == Some(&b':') {
            i += ident + 1;
            continue;
        }
        let word = rest.iter().take_while(|b| !b.is_ascii_whitespace()).count();
        if matches!(rest.get(..word), Some(b".loc" | b".file")) {
            i += rest.iter().take_while(|&&b| b != b'\n').count();
            continue;
        }
        break;
    }
    let rest = stmt.get(i..).unwrap_or(&[]);
    let len = rest
        .iter()
        .take_while(|&&b| !(b.is_ascii_whitespace() || matches!(b, b',' | b'{' | b'}')))
        .count();
    let op = rest.get(..len).unwrap_or(&[]);
    if op.is_empty() || op.first() == Some(&b'.') {
        return None;
    }
    Some(String::from_utf8_lossy(op).into_owned())
}

/// 関数の本文を文に分け、命令ごとにパターンを当てて数える。
fn count_body(
    kind: UnitKind,
    name: String,
    body: &[u8],
    patterns: &[InstructionPattern],
) -> Result<UnitReport, PtxSearchError> {
    let mut instructions: u64 = 0;
    let mut counts: Vec<(u64, BTreeMap<String, u64>)> =
        patterns.iter().map(|_| (0, BTreeMap::new())).collect();
    for stmt in body.split(|&b| b == b';') {
        let Some(op) = opcode_of(stmt) else { continue };
        instructions = instructions
            .checked_add(1)
            .ok_or(PtxSearchError::CountOverflow)?;
        for (pat, slot) in patterns.iter().zip(counts.iter_mut()) {
            if pat.rules.iter().any(|r| r.matches(&op)) {
                slot.0 = slot.0.checked_add(1).ok_or(PtxSearchError::CountOverflow)?;
                let e = slot.1.entry(op.clone()).or_insert(0);
                *e = e.checked_add(1).ok_or(PtxSearchError::CountOverflow)?;
            }
        }
    }
    let counts = patterns
        .iter()
        .zip(counts)
        .map(|(pat, (count, map))| PatternCount {
            id: pat.id,
            count,
            opcodes: map.into_iter().collect(),
        })
        .collect();
    Ok(UnitReport {
        kind,
        name,
        instructions,
        counts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str = ".version 8.0\n.target sm_80\n.address_size 64\n";

    fn pat(p: &InstructionPattern, op: &str) -> bool {
        p.rules.iter().any(|r| r.matches(op))
    }

    #[test]
    fn rule_root_and_parts() {
        let r = OpcodeRule {
            root: Some("atom"),
            all_parts: &["add"],
            any_parts: &["f32", "f64"],
        };
        assert!(r.matches("atom.global.add.f32"));
        assert!(r.matches("atom.add.f64"));
        assert!(!r.matches("red.global.add.f32"));
        assert!(!r.matches("atom.global.f32"));
        assert!(!r.matches("atom.global.add.u32"));
        let any = OpcodeRule {
            root: None,
            all_parts: &[],
            any_parts: &[],
        };
        assert!(any.matches("anything.x"));
    }

    #[test]
    fn rule_whole_word_only() {
        assert!(!pat(&F16_INSTRUCTIONS, "cvt.rn.bf16.f32"));
        assert!(!pat(&F32_ATOMIC_ADD_INSTRUCTIONS, "redux.sync.add.f32"));
    }

    #[test]
    fn builtin_patterns() {
        for op in ["fma.rn.f16", "cvt.rn.f16.f32", "cvt.f32.f16", "add.f16x2"] {
            assert!(pat(&F16_INSTRUCTIONS, op), "{op}");
        }
        for op in [
            "shfl.sync.bfly.b32",
            "redux.sync.add.u32",
            "vote.sync.ballot.b32",
        ] {
            assert!(pat(&SUBGROUP_INSTRUCTIONS, op), "{op}");
        }
        assert!(!pat(&SUBGROUP_INSTRUCTIONS, "red.global.add.f32"));
        for op in ["atom.global.add.f32", "red.global.add.f32"] {
            assert!(pat(&F32_ATOMIC_ADD_INSTRUCTIONS, op), "{op}");
        }
        for op in ["atom.global.add.u32", "redux.sync.add.u32"] {
            assert!(!pat(&F32_ATOMIC_ADD_INSTRUCTIONS, op), "{op}");
        }
        for op in [
            "mma.sync.aligned.m16n8k16.row.col.f32.f16.f16.f32",
            "wmma.mma.sync.aligned.row.row.m16n16k16.f32.f32",
        ] {
            assert!(pat(&MATRIX_MMA_INSTRUCTIONS, op), "{op}");
        }
        for op in [
            "wmma.load.a.sync.aligned.row.m16n16k16.global.f16",
            "wmma.store.d.sync.aligned.row.m16n16k16.global.f32",
        ] {
            assert!(!pat(&MATRIX_MMA_INSTRUCTIONS, op), "{op}");
        }
    }

    #[test]
    fn error_messages_nonempty() {
        for e in [
            PtxSearchError::EmptyPatterns,
            PtxSearchError::DuplicatePatternId,
            PtxSearchError::UnterminatedBlockComment,
            PtxSearchError::UnterminatedString,
            PtxSearchError::UnbalancedBraces,
            PtxSearchError::MissingFunctionName,
            PtxSearchError::DuplicateFunctionName,
            PtxSearchError::NoEntry,
            PtxSearchError::CountOverflow,
        ] {
            assert!(!e.to_string().is_empty());
        }
    }

    fn unit(id: &'static str, n: u64) -> UnitReport {
        UnitReport {
            kind: UnitKind::Entry,
            name: "k".into(),
            instructions: n,
            counts: vec![PatternCount {
                id,
                count: n,
                opcodes: vec![],
            }],
        }
    }

    #[test]
    fn total_sums_and_none() {
        let r = SearchReport {
            units: vec![unit("a", 2), unit("a", 3)],
            declarations: vec![],
        };
        assert_eq!(r.total("a"), Some(5));
        assert_eq!(r.total("b"), None);
        let o = SearchReport {
            units: vec![unit("a", u64::MAX), unit("a", 1)],
            declarations: vec![],
        };
        assert_eq!(o.total("a"), None);
    }

    #[test]
    fn smoke_loc_then_instruction() {
        let ptx = format!(
            "{HEAD}.visible .entry k(.param .u64 p)\n{{\n.reg .f32 %f<4>;\n.loc 1 2 0\nfma.rn.f16 %h1, %h2, %h3, %h4;\n$L1:\n@%p1 shfl.sync.bfly.b32 %r1, %r2, 1, 31, -1;\nret;\n}}\n"
        );
        let r = search(&ptx, &BUILTIN_PATTERNS).unwrap();
        assert_eq!(r.units.len(), 1);
        assert_eq!(r.units[0].instructions, 3);
        assert_eq!(r.total("f16"), Some(1));
        assert_eq!(r.total("subgroup"), Some(1));
        assert_eq!(r.total("matrix-mma"), Some(0));
    }

    const FUNC: &str = concat!(
        ".func (.param .b32 r) helper(.param .b32 a)\n",
        "{ // atom.global.add.f32 x;\n mma.sync.x; /* red.add.f32 y; */\n }\n",
        ".entry main()\n",
        "{ .pragma \"red.add.f32;\"; ret; }"
    );

    #[test]
    fn comments_strings_declarations_and_func() {
        let ptx = format!(
            "{HEAD}.extern .func ext(.param .b32 a);\n.global .align 4 .b8 tbl[4] = {{1,2,3,4}};\n{FUNC}"
        );
        let r = search(&ptx, &BUILTIN_PATTERNS).unwrap();
        assert_eq!(r.declarations, vec!["ext".to_string()]);
        assert_eq!(r.units.len(), 2);
        assert_eq!(r.units[0].kind, UnitKind::Func);
        assert_eq!(r.units[0].name, "helper");
        assert_eq!(r.total("matrix-mma"), Some(1));
        assert_eq!(r.total("f32-atomic-add"), Some(0));
        assert_eq!(r.units[1].instructions, 1);
    }

    #[test]
    fn errors() {
        let ok = format!("{HEAD}.entry k()\n{{ ret; }}");
        assert_eq!(search(&ok, &[]), Err(PtxSearchError::EmptyPatterns));
        assert_eq!(
            search(&ok, &[F16_INSTRUCTIONS, F16_INSTRUCTIONS]),
            Err(PtxSearchError::DuplicatePatternId)
        );
        let p = &BUILTIN_PATTERNS;
        assert_eq!(search("", p), Err(PtxSearchError::NoEntry));
        assert_eq!(
            search(&format!("{ok} /* x"), p),
            Err(PtxSearchError::UnterminatedBlockComment)
        );
        assert_eq!(
            search(&format!("{ok} \"x"), p),
            Err(PtxSearchError::UnterminatedString)
        );
        assert_eq!(
            search(".entry k() { ret;", p),
            Err(PtxSearchError::UnbalancedBraces)
        );
        assert_eq!(
            search(&format!("{ok} }}"), p),
            Err(PtxSearchError::UnbalancedBraces)
        );
        assert_eq!(
            search(".entry k()", p),
            Err(PtxSearchError::UnbalancedBraces)
        );
        assert_eq!(
            search(".entry () { ret; }", p),
            Err(PtxSearchError::MissingFunctionName)
        );
        assert_eq!(
            search(".entry k() { ret; } .entry k() { ret; }", p),
            Err(PtxSearchError::DuplicateFunctionName)
        );
        assert_eq!(
            search(".entry k(); .entry k() { ret; }", p).map(|r| r.declarations),
            Ok(vec!["k".to_string()])
        );
        assert_eq!(
            search(".func f() { ret; }", p),
            Err(PtxSearchError::NoEntry)
        );
    }
}
