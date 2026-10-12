//! `common` の数える道具を、見本の文字列で確かめるテスト（実物の `unsafe` はコミットしない）。
//!
//! 呼び出し先は `common` の `strip_comments_and_literals`・`count_unsafe`・`compare_counts`・
//! `check_backend`。背後実装ごとの個数テスト（`backend_<名前>_unsafe_count.rs`）が「期待値の表どおりか」を
//! 見るのに対し、ここは「道具が増減を正しく数え、食い違いで失敗するか」を見る。
//! 入口は `docs/unsafe-policy.md`、spec TASK-9 のポインタ。
//!
//! 見本の識別子（`g`・`sample_ffi` など）は実在しないダミー。
//! 限界: 「実際が期待より多い」向きは、実物の `unsafe` を置かない限り `check_backend` では作れない。
//! その向きは `compare_counts` への見本で示す。

mod common;

use common::{
    BACKENDS, ExpectedFile, UnsafeCounts, check_backend, compare_counts, count_unsafe,
    strip_comments_and_literals,
};

const fn c(
    block: usize,
    impl_: usize,
    func: usize,
    extern_block: usize,
    other: usize,
) -> UnsafeCounts {
    UnsafeCounts {
        block,
        impl_,
        func,
        extern_block,
        other,
    }
}

/// 見本の表を全行回し、食い違いを名前つきで集めて 1 回で失敗させる。
fn check_samples(samples: &[(&str, &str, UnsafeCounts)]) {
    let mut errors = Vec::new();
    for (name, src, expected) in samples {
        let actual = count_unsafe(src);
        if actual != *expected {
            errors.push(format!("{name}: expected {expected:?}, actual {actual:?}"));
        }
    }
    assert!(
        errors.is_empty(),
        "sample mismatches:\n{}",
        errors.join("\n")
    );
}

#[test]
fn unsafe_inside_comments_is_not_counted() {
    check_samples(&[
        (
            "line comment",
            "// unsafe { g() }\nfn f() {}",
            c(0, 0, 0, 0, 0),
        ),
        (
            "doc comment",
            "/// unsafe { g() }\nfn f() {}",
            c(0, 0, 0, 0, 0),
        ),
        (
            "inner doc comment",
            "//! unsafe { g() }\nfn f() {}",
            c(0, 0, 0, 0, 0),
        ),
        (
            "block comment",
            "/* unsafe { g() } */ fn f() {}",
            c(0, 0, 0, 0, 0),
        ),
        (
            "nested block comment",
            "/* a /* unsafe { g() } */ unsafe { g() } */ fn f() {}",
            c(0, 0, 0, 0, 0),
        ),
        (
            "multi-line block comment",
            "/*\n unsafe { g() }\n unsafe impl X {}\n*/\nfn f() {}",
            c(0, 0, 0, 0, 0),
        ),
    ]);
}

#[test]
fn real_block_after_comment_is_still_counted() {
    check_samples(&[
        (
            "same line",
            "/* unsafe { g() } */ fn f() { unsafe { g() } }",
            c(1, 0, 0, 0, 0),
        ),
        (
            "next line",
            "// unsafe { g() }\nfn f() { unsafe { g() } }",
            c(1, 0, 0, 0, 0),
        ),
        (
            "trailing comment",
            "fn f() { unsafe { g() } } // unsafe { g() }",
            c(1, 0, 0, 0, 0),
        ),
    ]);
}

#[test]
fn unsafe_inside_string_literals_is_not_counted() {
    check_samples(&[
        (
            "plain",
            r###"fn f() { let _ = "unsafe { g() }"; }"###,
            c(0, 0, 0, 0, 0),
        ),
        (
            "escaped quote",
            r###"fn f() { let _ = "a\" unsafe { g() }"; }"###,
            c(0, 0, 0, 0, 0),
        ),
        (
            "raw 0 hash",
            r###"fn f() { let _ = r"unsafe { g() }"; }"###,
            c(0, 0, 0, 0, 0),
        ),
        (
            "raw 1 hash",
            r###"fn f() { let _ = r#"unsafe { " g() }"#; }"###,
            c(0, 0, 0, 0, 0),
        ),
        (
            "raw 2 hashes",
            r###"fn f() { let _ = r##"unsafe { "# g() }"##; }"###,
            c(0, 0, 0, 0, 0),
        ),
        (
            "byte string",
            r###"fn f() { let _ = b"unsafe { g() }"; }"###,
            c(0, 0, 0, 0, 0),
        ),
        (
            "raw byte string",
            r###"fn f() { let _ = br#"unsafe { g() }"#; }"###,
            c(0, 0, 0, 0, 0),
        ),
        (
            "c string",
            r###"fn f() { let _ = c"unsafe { g() }"; }"###,
            c(0, 0, 0, 0, 0),
        ),
        (
            "raw c string",
            r###"fn f() { let _ = cr#"unsafe { g() }"#; }"###,
            c(0, 0, 0, 0, 0),
        ),
        (
            "multi-line string",
            "fn f() { let _ = \"a\n unsafe { g() }\n b\"; }",
            c(0, 0, 0, 0, 0),
        ),
        (
            "line continuation",
            "fn f() { let _ = \"a \\\n unsafe { g() }\"; }",
            c(0, 0, 0, 0, 0),
        ),
    ]);
}

#[test]
fn real_block_after_literal_is_still_counted() {
    check_samples(&[
        (
            "after string",
            r###"fn f() { let _ = "unsafe { g() }"; unsafe { g() } }"###,
            c(1, 0, 0, 0, 0),
        ),
        (
            "after comment-like strings",
            r###"fn f() { let _ = "//"; let _ = "/*"; unsafe { g() } }"###,
            c(1, 0, 0, 0, 0),
        ),
        (
            "quote inside comment",
            "fn f() { /* \" */ unsafe { g() } }",
            c(1, 0, 0, 0, 0),
        ),
        (
            "lifetime",
            "fn f<'a>(x: &'a u8) -> &'a u8 { unsafe { g(x) } }",
            c(1, 0, 0, 0, 0),
        ),
    ]);
}

#[test]
fn identifiers_containing_unsafe_are_not_counted() {
    check_samples(&[
        (
            "forbid",
            "#![forbid(unsafe_code)]\nfn f() {}",
            c(0, 0, 0, 0, 0),
        ),
        ("deny", "#![deny(unsafe_code)]\nfn f() {}", c(0, 0, 0, 0, 0)),
        ("allow", "#[allow(unsafe_code)]\nmod m {}", c(0, 0, 0, 0, 0)),
        (
            "lint name",
            "#![warn(unsafe_op_in_unsafe_fn)]\nfn f() {}",
            c(0, 0, 0, 0, 0),
        ),
        ("prefix", "fn not_unsafe() {}", c(0, 0, 0, 0, 0)),
        ("suffix", "fn unsafe_helper() {}", c(0, 0, 0, 0, 0)),
        ("digit suffix", "fn unsafe2() {}", c(0, 0, 0, 0, 0)),
        ("upper case", "const UNSAFE: u8 = 0;", c(0, 0, 0, 0, 0)),
        ("capitalized", "struct Unsafe;", c(0, 0, 0, 0, 0)),
        (
            "raw identifier",
            "fn f() { let r#unsafe = 0; }",
            c(0, 0, 0, 0, 0),
        ),
    ]);
}

#[test]
fn unsafe_extern_block_and_fn_are_distinguished() {
    check_samples(&[
        (
            "extern block with abi",
            "unsafe extern \"C\" { fn sample_ffi(); }",
            c(0, 0, 0, 1, 0),
        ),
        (
            "extern block without abi",
            "unsafe extern { fn sample_ffi(); }",
            c(0, 0, 0, 1, 0),
        ),
        (
            "extern fn with abi",
            "unsafe extern \"C\" fn f() {}",
            c(0, 0, 1, 0, 0),
        ),
        (
            "pub extern fn",
            "pub unsafe extern \"C\" fn f() {}",
            c(0, 0, 1, 0, 0),
        ),
        (
            "extern fn without abi",
            "unsafe extern fn f() {}",
            c(0, 0, 1, 0, 0),
        ),
        (
            "extern block with inner fns",
            "unsafe extern \"C\" { pub unsafe fn f(); pub safe fn g(); }",
            c(0, 0, 1, 1, 0),
        ),
    ]);
}

#[test]
fn unsafe_fn_and_impl_forms_are_counted() {
    check_samples(&[
        ("pub unsafe fn", "pub unsafe fn f() {}", c(0, 0, 1, 0, 0)),
        (
            "pub(crate) unsafe fn",
            "pub(crate) unsafe fn f() {}",
            c(0, 0, 1, 0, 0),
        ),
        (
            "indented in impl",
            "impl X {\n    pub unsafe fn f(&self) {}\n}",
            c(0, 0, 1, 0, 0),
        ),
        (
            "pub const unsafe fn",
            "pub const unsafe fn f() {}",
            c(0, 0, 1, 0, 0),
        ),
        ("unsafe impl", "unsafe impl Send for X {}", c(0, 1, 0, 0, 0)),
        (
            "generic unsafe impl",
            "unsafe impl<T> Send for X<T> {}",
            c(0, 1, 0, 0, 0),
        ),
        (
            "fn pointer type",
            "type F = unsafe fn(u8) -> u8;",
            c(0, 0, 1, 0, 0),
        ),
        (
            "extern fn pointer type",
            "type F = unsafe extern \"C\" fn(u8) -> u8;",
            c(0, 0, 1, 0, 0),
        ),
    ]);
}

#[test]
fn block_forms_and_other_kinds_are_counted() {
    check_samples(&[
        ("block", "fn f() { unsafe { g() } }", c(1, 0, 0, 0, 0)),
        ("no space", "fn f() { unsafe{ g() } }", c(1, 0, 0, 0, 0)),
        (
            "brace on next line",
            "fn f() { unsafe\n{ g() } }",
            c(1, 0, 0, 0, 0),
        ),
        (
            "comment before brace",
            "fn f() { unsafe /* x */ { g() } }",
            c(1, 0, 0, 0, 0),
        ),
        (
            "two blocks",
            "fn f() { unsafe { g() } unsafe { g() } }",
            c(2, 0, 0, 0, 0),
        ),
        (
            "fn with inner block",
            "unsafe fn f() { unsafe { g() } }",
            c(1, 0, 1, 0, 0),
        ),
        ("unsafe trait", "unsafe trait T {}", c(0, 0, 0, 0, 1)),
        (
            "unsafe attribute",
            "#[unsafe(no_mangle)]\nfn f() {}",
            c(0, 0, 0, 0, 1),
        ),
    ]);
}

#[test]
fn mixed_kinds_are_counted_per_kind() {
    let src = r###"
//! deny(unsafe_code) の説明 unsafe { g() }
/// unsafe impl の説明
unsafe extern "C" {
    pub unsafe fn sample_ffi();
}
unsafe trait T {}
unsafe impl T for X {}
pub unsafe fn h() {}
fn f() {
    let _ = "unsafe { g() }";
    unsafe { sample_ffi() }
}
"###;
    let actual = count_unsafe(src);
    assert_eq!(actual, c(1, 1, 2, 1, 1));
    assert_eq!(actual.total(), 6);
}

#[test]
fn strip_keeps_line_count_and_removes_comment_and_literal_bodies() {
    let src = "/* a\n unsafe { x }\n*/\nlet a = \"b\n unsafe\n\";\nlet c = r#\"d\nunsafe\n\"#;\n// unsafe\nlet e = 1;\n";
    let out = strip_comments_and_literals(src);
    assert_eq!(out.matches('\n').count(), src.matches('\n').count());
    assert!(!out.contains("unsafe"), "stripped: {out:?}");
    assert!(out.contains("let a") && out.contains("let c") && out.contains("let e"));
}

fn one_block() -> UnsafeCounts {
    count_unsafe("fn f() { unsafe { g() } }")
}

#[test]
fn compare_counts_passes_when_equal() {
    assert_eq!(
        compare_counts("p.rs", &c(1, 0, 0, 0, 0), &one_block()),
        Ok(())
    );
}

#[test]
fn compare_counts_fails_when_actual_exceeds_expected() {
    let err = compare_counts("p.rs", &UnsafeCounts::ZERO, &one_block()).unwrap_err();
    assert_eq!(err, "p.rs: block: expected 0, actual 1");
}

#[test]
fn compare_counts_fails_when_actual_is_below_expected() {
    let err = compare_counts("p.rs", &c(2, 0, 0, 0, 0), &one_block()).unwrap_err();
    assert_eq!(err, "p.rs: block: expected 2, actual 1");
}

#[test]
fn compare_counts_fails_when_only_kinds_differ() {
    let err = compare_counts("p.rs", &c(0, 1, 0, 0, 0), &one_block()).unwrap_err();
    assert!(err.contains("p.rs: impl: expected 1, actual 0"), "{err}");
    assert!(err.contains("p.rs: block: expected 0, actual 1"), "{err}");
}

/// 同じ背後実装の持ち場にある、ずらさない行（実ファイルの個数のまま）。
const VULKAN_EXTRA: ExpectedFile = ExpectedFile {
    path: "src/backend_vulkan/spirv_operands.rs",
    source: include_str!("../src/backend_vulkan/spirv_operands.rs"),
    expected: UnsafeCounts::ZERO,
};

/// 実ファイルの実際の個数を期待値にした表。先頭の行の block だけ `delta` ずらせる。
/// Vulkan は持ち場に子モジュールの行もあるので、表に足す（足さないと「表に無い」で失敗する）。
fn table_for(path: &'static str, source: &'static str, delta: usize) -> Vec<ExpectedFile> {
    let mut expected = count_unsafe(source);
    expected.block += delta;
    let mut table = vec![ExpectedFile {
        path,
        source,
        expected,
    }];
    if path == "src/backend_vulkan.rs" {
        table.push(VULKAN_EXTRA);
    }
    table
}

#[test]
fn check_backend_fails_when_expected_is_off_by_one() {
    let rows: [(&str, &'static str, &'static str); 3] = [
        (
            "metal",
            "src/backend_metal.rs",
            include_str!("../src/backend_metal.rs"),
        ),
        (
            "vulkan",
            "src/backend_vulkan.rs",
            include_str!("../src/backend_vulkan.rs"),
        ),
        (
            "cuda",
            "src/backend_cuda.rs",
            include_str!("../src/backend_cuda.rs"),
        ),
    ];
    let names: Vec<&str> = rows.iter().map(|r| r.0).collect();
    assert_eq!(names, BACKENDS, "update this test when BACKENDS changes");

    for (name, path, source) in rows {
        assert_eq!(
            check_backend(name, &table_for(path, source, 0)),
            Ok(()),
            "{name}: exact table must pass"
        );
        let actual = count_unsafe(source).block;
        let err = check_backend(name, &table_for(path, source, 1)).unwrap_err();
        let want = format!("{path}: block: expected {}, actual {actual}", actual + 1);
        assert!(err.contains(&want), "{name}: {err}");
    }
}
