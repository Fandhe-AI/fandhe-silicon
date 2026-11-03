//! `GraphicsOps` の rustdoc にある `compile_fail,E0046` の例の形を、ソースの文字列から確かめる統合テスト。
//!
//! rustdoc の `compile_fail,E0046` は「E0046 が出ること」しか見ない。例の局所の実装から別の操作が
//! 抜けても通ってしまうので、各例が「trait の全操作から狙った 1 操作だけを除いた集合」であることをここで押さえる。
//! 同じ集合に除いた 1 操作を足した実装がコンパイルに通ることは、rustdoc 側の通常の例が確かめる。

use std::collections::BTreeSet;

const SOURCE: &str = include_str!("../src/graphics.rs");

/// `fn name(` の行から操作名を取り出す。
fn fn_name(line: &str) -> Option<&str> {
    let rest = line.trim_start().strip_prefix("fn ")?;
    Some(rest.split('(').next()?.trim())
}

/// `pub trait GraphicsOps` の本体にある操作名の集合。
fn trait_ops() -> BTreeSet<String> {
    let body = SOURCE
        .split("pub trait GraphicsOps {")
        .nth(1)
        .expect("trait GraphicsOps");
    let mut ops = BTreeSet::new();
    for line in body.lines() {
        if line.starts_with('}') {
            break;
        }
        // trait 直下の宣言だけ（4 スペースの字下げ・doc コメントでない行）を拾う。
        if line.starts_with("    fn ") {
            ops.insert(fn_name(line).expect("fn name").to_string());
        }
    }
    ops
}

/// `compile_fail,E0046` の例ごとの、局所の実装が持つ操作名の集合。
fn compile_fail_impls() -> Vec<BTreeSet<String>> {
    let mut blocks = Vec::new();
    let mut current: Option<BTreeSet<String>> = None;
    for line in SOURCE.lines() {
        let Some(doc) = line.strip_prefix("///") else {
            continue;
        };
        match (&mut current, doc.trim()) {
            (None, "```compile_fail,E0046") => current = Some(BTreeSet::new()),
            (Some(_), "```") => blocks.push(current.take().expect("open block")),
            (Some(set), _) if doc.starts_with("     fn ") => {
                set.insert(fn_name(doc).expect("fn name").to_string());
            }
            _ => {}
        }
    }
    blocks
}

#[test]
fn each_e0046_example_lacks_exactly_one_operation() {
    let all = trait_ops();
    let blocks = compile_fail_impls();
    assert_eq!(all.len(), 23, "trait operations");
    assert_eq!(blocks.len(), 12, "E0046 examples with a local impl");

    let mut missing_all = BTreeSet::new();
    for block in &blocks {
        let missing: Vec<&String> = all.difference(block).collect();
        assert_eq!(missing.len(), 1, "exactly one missing: {missing:?}");
        assert!(block.is_subset(&all), "no unknown operation in the impl");
        assert!(missing_all.insert(missing[0].clone()), "distinct targets");
    }
    for name in [
        "draw_mesh_tasks",
        "draw_mesh_tasks_indirect",
        "multi_draw_mesh_tasks_indirect",
        "multi_draw_mesh_tasks_indirect_count",
    ] {
        assert!(missing_all.contains(name), "{name} has an E0046 example");
    }
}
