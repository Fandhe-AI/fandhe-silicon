//! 手で書いた PTX の入力に対する検索の関数の振る舞いの確認（spec TASK-100）。
//! 公開のパス `fandhe_silicon_core::ptx_search` だけを使い、入力は試験の中に組んだ最小の PTX のテキスト。
//! 置き場と範囲の説明は docs/ptx-instruction-search.md。

use fandhe_silicon_core::ptx_search::{
    BUILTIN_PATTERNS, F16_INSTRUCTIONS, PatternCount, PtxSearchError, SearchReport, UnitKind,
    UnitReport, search,
};

const HEAD: &str = ".version 8.0\n.target sm_80\n.address_size 64\n";

fn unit<'a>(r: &'a SearchReport, name: &str) -> &'a UnitReport {
    r.units.iter().find(|u| u.name == name).unwrap()
}

fn count<'a>(u: &'a UnitReport, id: &str) -> &'a PatternCount {
    u.counts.iter().find(|c| c.id == id).unwrap()
}

fn ops(pairs: &[(&str, u64)]) -> Vec<(String, u64)> {
    pairs.iter().map(|(s, n)| (s.to_string(), *n)).collect()
}

#[test]
fn counts_each_pattern_per_entry() {
    let ptx = format!(
        "{HEAD}\
.visible .entry hit(.param .u64 p)\n{{\n\
.reg .f32 %f<4>;\n\
cvt.rn.f16.f32 %h1, %f1;\n\
fma.rn.f16 %h2, %h1, %h1, %h1;\n\
shfl.sync.bfly.b32 %r1, %r2, 1, 31, -1;\n\
atom.global.add.f32 %f2, [%rd1], %f1;\n\
red.global.add.f32 [%rd1], %f1;\n\
mma.sync.aligned.m16n8k16.row.col.f32.f16.f16.f32 {{%f1, %f2, %f3, %f4}}, {{%r1, %r2, %r3, %r4}}, {{%r5, %r6}}, {{%f1, %f2, %f3, %f4}};\n\
ret;\n}}\n\
.visible .entry miss(.param .u64 p)\n{{\n\
add.s32 %r1, %r2, %r3;\n\
ld.global.u32 %r4, [%rd1];\n\
st.global.u32 [%rd1], %r4;\n\
ret;\n}}\n"
    );
    let r = search(&ptx, &BUILTIN_PATTERNS).unwrap();
    assert_eq!(r.units.len(), 2);
    assert!(r.declarations.is_empty());
    let hit = &r.units[0];
    assert_eq!((hit.kind, hit.name.as_str()), (UnitKind::Entry, "hit"));
    assert_eq!(hit.instructions, 7);
    let ids: Vec<&str> = hit.counts.iter().map(|c| c.id).collect();
    assert_eq!(ids, ["f16", "subgroup", "f32-atomic-add", "matrix-mma"]);
    assert_eq!(count(hit, "f16").count, 3);
    assert_eq!(
        count(hit, "f16").opcodes,
        ops(&[
            ("cvt.rn.f16.f32", 1),
            ("fma.rn.f16", 1),
            ("mma.sync.aligned.m16n8k16.row.col.f32.f16.f16.f32", 1)
        ])
    );
    assert_eq!(
        count(hit, "subgroup").opcodes,
        ops(&[("shfl.sync.bfly.b32", 1)])
    );
    assert_eq!(count(hit, "f32-atomic-add").count, 2);
    assert_eq!(
        count(hit, "f32-atomic-add").opcodes,
        ops(&[("atom.global.add.f32", 1), ("red.global.add.f32", 1)])
    );
    assert_eq!(count(hit, "matrix-mma").count, 1);
    let miss = &r.units[1];
    assert_eq!(miss.name, "miss");
    assert_eq!(miss.instructions, 4);
    for c in &miss.counts {
        assert_eq!((c.count, c.opcodes.len()), (0, 0), "{}", c.id);
    }
    assert_eq!(r.total("f16"), Some(3));
}

#[test]
fn directives_comments_and_strings_are_not_counted() {
    let ptx = format!(
        "{HEAD}.file 1 \"a .entry b // c.cu\"\n\
.visible .entry k(.param .u64 p)\n{{\n\
.reg .f16 %h<4>;\n\
.shared .align 4 .b8 buf[16];\n\
.pragma \"nounroll\";\n\
.loc 1 5 3\n\
// fma.rn.f16 %h1, %h2, %h3, %h4;\n\
/* shfl.sync.bfly.b32 %r1, %r2, 1, 31, -1;\n vote.sync.ballot.b32 %r1, %p1, -1; */\n\
add.s32 %r1, %r2, %r3;\n\
ret;\n}}\n"
    );
    let r = search(&ptx, &BUILTIN_PATTERNS).unwrap();
    assert_eq!(r.units.len(), 1);
    assert_eq!(r.units[0].name, "k");
    assert_eq!(r.units[0].instructions, 2);
    for c in &r.units[0].counts {
        assert_eq!(c.count, 0, "{}", c.id);
    }
}

#[test]
fn labels_predicates_and_scopes() {
    let ptx = format!(
        "{HEAD}.visible .entry k(.param .u64 p)\n{{\n\
$L__BB0_1:\n\
@%p1 shfl.sync.bfly.b32 %r1, %r2, 1, 31, -1;\n\
@!%p1 bra $L__BB0_1;\n\
{{ .reg .b16 t; cvt.rn.f16.f32 t, %f1; }}\n\
$L__BB0_2:\n\
ret;\n}}\n"
    );
    let r = search(&ptx, &BUILTIN_PATTERNS).unwrap();
    let u = unit(&r, "k");
    assert_eq!(u.instructions, 4);
    assert_eq!(
        count(u, "subgroup").opcodes,
        ops(&[("shfl.sync.bfly.b32", 1)])
    );
    assert_eq!(count(u, "f16").opcodes, ops(&[("cvt.rn.f16.f32", 1)]));
}

#[test]
fn vector_operand_braces_are_part_of_the_instruction() {
    let base = format!("{HEAD}.visible .entry k()\n{{\n ret;\n}}\n");
    let with = format!(
        "{HEAD}.visible .entry k()\n{{\n\
wmma.mma.sync.aligned.row.row.m16n16k16.f32.f32 {{%f1, %f2}}, {{%r1, %r2}}, {{%r3, %r4}}, {{%f3, %f4}};\n\
ret;\n}}\n"
    );
    let b = search(&base, &BUILTIN_PATTERNS).unwrap();
    let w = search(&with, &BUILTIN_PATTERNS).unwrap();
    assert_eq!(w.units[0].instructions, b.units[0].instructions + 1);
    assert_eq!(
        count(&w.units[0], "matrix-mma").opcodes,
        ops(&[("wmma.mma.sync.aligned.row.row.m16n16k16.f32.f32", 1)])
    );
    assert_eq!(w.total("f16"), Some(0));
    let load = format!(
        "{HEAD}.visible .entry k()\n{{\n\
wmma.load.a.sync.aligned.row.m16n16k16.global.f16 {{%r1, %r2}}, [%rd1];\n\
ret;\n}}\n"
    );
    let l = search(&load, &BUILTIN_PATTERNS).unwrap();
    assert_eq!(l.total("matrix-mma"), Some(0));
    assert_eq!(l.total("f16"), Some(1));
}

#[test]
fn func_bodies_and_declarations() {
    let ptx = format!(
        "{HEAD}.extern .func (.param .b32 r) ext(.param .b32 a);\n\
.visible .func (.param .b32 r) helper(.param .b32 a)\n{{\n\
red.global.add.f32 [%rd1], %f1;\n\
ret;\n}}\n\
.visible .entry k()\n{{\n\
atom.global.add.f32 %f2, [%rd1], %f1;\n\
ret;\n}}\n"
    );
    let r = search(&ptx, &BUILTIN_PATTERNS).unwrap();
    assert_eq!(r.declarations, vec!["ext".to_string()]);
    let names: Vec<(UnitKind, &str)> = r.units.iter().map(|u| (u.kind, u.name.as_str())).collect();
    assert_eq!(names, [(UnitKind::Func, "helper"), (UnitKind::Entry, "k")]);
    assert_eq!(count(&r.units[0], "f32-atomic-add").count, 1);
    assert_eq!(count(&r.units[1], "f32-atomic-add").count, 1);
    assert_eq!(r.total("f32-atomic-add"), Some(2));
}

#[test]
fn top_level_initializers_are_skipped() {
    let ptx = format!(
        "{HEAD}.global .align 1 .b8 s[3] = {{104, 105, 0}};\n\
.visible .entry k()\n{{\n fma.rn.f16 %h1, %h2, %h3, %h4;\n ret;\n}}\n"
    );
    let r = search(&ptx, &BUILTIN_PATTERNS).unwrap();
    assert_eq!(r.units.len(), 1);
    assert_eq!(r.units[0].name, "k");
    assert_eq!(r.units[0].instructions, 2);
    assert_eq!(r.total("f16"), Some(1));
    assert!(r.declarations.is_empty());
}

#[test]
fn malformed_inputs_are_errors() {
    let ok = format!("{HEAD}.visible .entry k()\n{{\n ret;\n}}\n");
    let p = &BUILTIN_PATTERNS;
    assert!(matches!(search("", p), Err(PtxSearchError::NoEntry)));
    assert!(matches!(search(HEAD, p), Err(PtxSearchError::NoEntry)));
    let open = format!("{HEAD}.visible .entry k()\n{{\n ret;\n");
    assert!(matches!(
        search(&open, p),
        Err(PtxSearchError::UnbalancedBraces)
    ));
    let extra = format!("{ok}}}\n");
    assert!(matches!(
        search(&extra, p),
        Err(PtxSearchError::UnbalancedBraces)
    ));
    let comment = format!("{ok}/* x\n");
    assert!(matches!(
        search(&comment, p),
        Err(PtxSearchError::UnterminatedBlockComment)
    ));
    let string = format!("{ok}.file 1 \"x.cu\n");
    assert!(matches!(
        search(&string, p),
        Err(PtxSearchError::UnterminatedString)
    ));
    let nameless = format!("{HEAD}.visible .entry (.param .u64 p)\n{{\n ret;\n}}\n");
    assert!(matches!(
        search(&nameless, p),
        Err(PtxSearchError::MissingFunctionName)
    ));
    let dup = format!("{ok}.visible .entry k()\n{{\n ret;\n}}\n");
    assert!(matches!(
        search(&dup, p),
        Err(PtxSearchError::DuplicateFunctionName)
    ));
    assert!(matches!(
        search(&ok, &[]),
        Err(PtxSearchError::EmptyPatterns)
    ));
    let twice = [F16_INSTRUCTIONS, F16_INSTRUCTIONS];
    assert!(matches!(
        search(&ok, &twice),
        Err(PtxSearchError::DuplicatePatternId)
    ));
}

#[test]
fn builtin_patterns_are_the_four() {
    let ids: Vec<&str> = BUILTIN_PATTERNS.iter().map(|p| p.id).collect();
    assert_eq!(ids, ["f16", "subgroup", "f32-atomic-add", "matrix-mma"]);
    for (i, id) in ids.iter().enumerate() {
        assert!(!ids[..i].contains(id));
    }
}
