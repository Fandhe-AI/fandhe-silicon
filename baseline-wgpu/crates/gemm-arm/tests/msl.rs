//! `msl` の言語版と束縛の番号の読み取りを、合成のテキストで確かめる（spec TASK-1010 のポインタ）。
//! device には触れない。

use baseline_wgpu_gemm_arm::msl::{MslError, Slots, buffer_slots, language_version};

const ENTRY: &str = "test_entry";
const SIZES_OK: &str = "    uint size0;\n    uint size1;\n    uint size2;\n";

/// 引数の並び `args`・大きさの表の本体 `sizes`・入口の宣言の本体 `kernels` から合成のテキストを作る。
fn build(args: &[(&str, &str)], sizes: &str, kernels: &str) -> String {
    let mut text = String::from("// language: metal2.4\n#include <metal_stdlib>\n");
    text.push_str(&format!("struct _mslBufferSizes {{\n{sizes}}};\n"));
    text.push_str(kernels);
    text.push_str("(\n");
    for (name, number) in args {
        text.push_str(&format!("    device float* {name} [[buffer({number})]],\n"));
    }
    text.push_str("    uint3 gid [[thread_position_in_grid]]) {\n}\n");
    text
}

fn kernel(entry: &str) -> String {
    format!("kernel void {entry}")
}

const GOOD_ARGS: [(&str, &str); 5] = [
    ("a", "2"),
    ("b", "0"),
    ("c", "1"),
    ("dims", "4"),
    ("_buffer_sizes", "3"),
];

fn good() -> String {
    build(&GOOD_ARGS, SIZES_OK, &kernel(ENTRY))
}

fn with_args(args: &[(&str, &str)]) -> Result<Slots, MslError> {
    buffer_slots(&build(args, SIZES_OK, &kernel(ENTRY)), ENTRY)
}

#[test]
fn language_version_reads_the_first_line() {
    assert_eq!(language_version("// language: metal2.4\nrest"), Ok((2, 4)));
    assert_eq!(language_version("// language: metal3.1\n"), Ok((3, 1)));
    assert_eq!(language_version("// language: metal10.12\nx"), Ok((10, 12)));
    assert_eq!(language_version("// language: metal2.4"), Ok((2, 4)));
}

#[test]
fn language_line_shape_is_checked() {
    for text in [
        "",
        "\n// language: metal2.4\n",
        "// Language: metal2.4\n",
        "//language: metal2.4\n",
        "// language: metal2\n",
        "// language: metal.4\n",
        "// language: metal2.\n",
        "// language: metal100.1\n",
        "// language: metal2.100\n",
        "// language: metal2.4 \n",
        "// language: metal2.4\r\n",
        "// language: metal2.4x\n",
        " // language: metal2.4\n",
        "// language: metal2.4.1\n",
        "// language: metal+2.4\n",
        "#include <x>\n// language: metal2.4\n",
    ] {
        assert_eq!(
            language_version(text),
            Err(MslError::LanguageLine),
            "{text:?}"
        );
    }
}

#[test]
fn good_text_returns_the_slots() {
    let slots = buffer_slots(&good(), ENTRY).expect("good text");
    assert_eq!(
        (slots.a(), slots.b(), slots.c(), slots.dims(), slots.sizes()),
        (2, 0, 1, 4, 3)
    );
    let loose = good()
        .replace(
            "device float* a [[buffer(2)]]",
            "device float* a   [[buffer(2)]]",
        )
        .replace("    uint size1;", "\t\tuint size1;");
    assert!(buffer_slots(&loose, ENTRY).is_ok());
    let low = with_args(&[
        ("a", "30"),
        ("b", "29"),
        ("c", "28"),
        ("dims", "27"),
        ("_buffer_sizes", "26"),
    ])
    .expect("30 is accepted");
    assert_eq!(low.a(), 30);
}

#[test]
fn entry_point_shape_is_checked() {
    let none = build(&GOOD_ARGS, SIZES_OK, &kernel("other_entry"));
    assert_eq!(buffer_slots(&none, ENTRY), Err(MslError::EntryPoint));
    let prefix = build(&GOOD_ARGS, SIZES_OK, &kernel("test_entry_x"));
    assert_eq!(buffer_slots(&prefix, ENTRY), Err(MslError::EntryPoint));
    let twice = format!("{}{}", good(), good());
    assert_eq!(buffer_slots(&twice, ENTRY), Err(MslError::EntryPoint));
    let extra = format!("{}kernel void helper(uint x) {{}}\n", good());
    assert_eq!(buffer_slots(&extra, ENTRY), Err(MslError::EntryPoint));
    for entry in ["", "a b", "x(", "x-y"] {
        assert_eq!(
            buffer_slots(&good(), entry),
            Err(MslError::EntryPoint),
            "{entry:?}"
        );
    }
}

#[test]
fn slot_numbers_are_checked() {
    for number in ["31", "99", "100", "", "x", "1 "] {
        let args = [
            ("a", number),
            ("b", "1"),
            ("c", "2"),
            ("dims", "3"),
            ("_buffer_sizes", "4"),
        ];
        assert_eq!(with_args(&args), Err(MslError::SlotNumber), "{number:?}");
    }
    let unclosed = good().replace("[[buffer(2)]]", "[[buffer(2]]");
    // 閉じない括弧は引数リストの対応も崩すので、入口の形の失敗になる。
    assert_eq!(buffer_slots(&unclosed, ENTRY), Err(MslError::EntryPoint));
    let at_end = good().replace("uint3 gid", "[[buffer( uint3 gid");
    assert_eq!(buffer_slots(&at_end, ENTRY), Err(MslError::EntryPoint));
}

#[test]
fn slot_names_are_checked() {
    for skip in 0..5 {
        let args: Vec<_> = GOOD_ARGS
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != skip)
            .map(|(_, arg)| *arg)
            .collect();
        assert_eq!(with_args(&args), Err(MslError::MissingSlot), "skip {skip}");
    }
    let mut dup = GOOD_ARGS.to_vec();
    dup.push(("a", "5"));
    assert_eq!(with_args(&dup), Err(MslError::DuplicateSlot));
    let mut other = GOOD_ARGS.to_vec();
    other.push(("d", "5"));
    assert_eq!(with_args(&other), Err(MslError::UnknownSlot));
    let nameless = good().replace("uint3 gid", "[[buffer(6)]] uint3 gid");
    assert_eq!(buffer_slots(&nameless, ENTRY), Err(MslError::UnknownSlot));
}

#[test]
fn slot_collisions_are_rejected() {
    for (x, y) in [(0, 1), (0, 4), (2, 3)] {
        let mut numbers = ["0", "1", "2", "3", "4"];
        numbers[y] = numbers[x];
        let args: Vec<_> = ["a", "b", "c", "dims", "_buffer_sizes"]
            .into_iter()
            .zip(numbers)
            .collect();
        assert_eq!(with_args(&args), Err(MslError::SlotCollision), "{x} {y}");
    }
}

#[test]
fn sizes_struct_is_checked() {
    let build_with = |sizes: &str| buffer_slots(&build(&GOOD_ARGS, sizes, &kernel(ENTRY)), ENTRY);
    for sizes in [
        "    uint size0;\n    uint size1;\n",
        "    uint size0;\n    uint size1;\n    uint size2;\n    uint size3;\n",
        "    uint size0;\n    uint size2;\n    uint size1;\n",
        "    int size0;\n    uint size1;\n    uint size2;\n",
        "",
    ] {
        assert_eq!(build_with(sizes), Err(MslError::SizesStruct), "{sizes:?}");
    }
    let without = good().replace("struct _mslBufferSizes {", "struct other {");
    assert_eq!(buffer_slots(&without, ENTRY), Err(MslError::SizesStruct));
    let twice = format!("struct _mslBufferSizes {{\n{SIZES_OK}}};\n{}", good());
    assert_eq!(buffer_slots(&twice, ENTRY), Err(MslError::SizesStruct));
}

#[test]
fn checks_run_in_a_fixed_order() {
    let bad_sizes = "    uint size0;\n";
    // 入口の誤りと番号の誤り。
    let args = [
        ("a", "99"),
        ("b", "1"),
        ("c", "2"),
        ("dims", "3"),
        ("_buffer_sizes", "4"),
    ];
    let text = build(&args, SIZES_OK, &kernel("other_entry"));
    assert_eq!(buffer_slots(&text, ENTRY), Err(MslError::EntryPoint));
    // 番号の誤りと名前の欠け。
    let args = [("a", "99"), ("b", "1"), ("c", "2"), ("dims", "3")];
    let text = build(&args, SIZES_OK, &kernel(ENTRY));
    assert_eq!(buffer_slots(&text, ENTRY), Err(MslError::SlotNumber));
    // 欠けと重なりと大きさの表の誤り。
    let args = [("a", "1"), ("b", "1"), ("c", "2"), ("dims", "3")];
    let text = build(&args, bad_sizes, &kernel(ENTRY));
    assert_eq!(buffer_slots(&text, ENTRY), Err(MslError::MissingSlot));
    // 重なりと大きさの表の誤り。
    let args = [
        ("a", "1"),
        ("b", "1"),
        ("c", "2"),
        ("dims", "3"),
        ("_buffer_sizes", "4"),
    ];
    let text = build(&args, bad_sizes, &kernel(ENTRY));
    assert_eq!(buffer_slots(&text, ENTRY), Err(MslError::SlotCollision));
    // 名前の誤りは、重複・欠けより先に当たる。
    let args = [("a", "1"), ("d", "1"), ("a", "2")];
    let text = build(&args, SIZES_OK, &kernel(ENTRY));
    assert_eq!(buffer_slots(&text, ENTRY), Err(MslError::UnknownSlot));
}

#[test]
fn errors_do_not_print_values() {
    let all = [
        MslError::LanguageLine,
        MslError::EntryPoint,
        MslError::SlotNumber,
        MslError::UnknownSlot,
        MslError::DuplicateSlot,
        MslError::MissingSlot,
        MslError::SlotCollision,
        MslError::SizesStruct,
    ];
    for error in all {
        let shown = format!("{error} {error:?}");
        assert!(!shown.is_empty());
        for fragment in [ENTRY, "99", "[[", "kernel", "uint"] {
            assert!(!shown.contains(fragment), "{shown}");
        }
    }
}

#[test]
fn slots_of_other_functions_are_not_read() {
    // 指定した kernel の引数が空で、別の通常関数の引数にだけ属性がある。
    let mut text = String::from("// language: metal2.4\n#include <metal_stdlib>\n");
    text.push_str(&format!("struct _mslBufferSizes {{\n{SIZES_OK}}};\n"));
    text.push_str(&format!("{}() {{\n}}\n", kernel(ENTRY)));
    text.push_str("void helper(\n");
    for (name, number) in GOOD_ARGS {
        text.push_str(&format!("    device float* {name} [[buffer({number})]],\n"));
    }
    text.push_str("    uint x) {\n}\n");
    assert_eq!(buffer_slots(&text, ENTRY), Err(MslError::MissingSlot));
}

#[test]
fn unclosed_argument_list_is_rejected() {
    let text = format!(
        "// language: metal2.4\nstruct _mslBufferSizes {{\n{SIZES_OK}}};\n{}(\n device float* a [[buffer(0)]]\n",
        kernel(ENTRY)
    );
    assert_eq!(buffer_slots(&text, ENTRY), Err(MslError::EntryPoint));
}

#[test]
fn entry_name_must_start_with_letter_or_underscore() {
    for entry in ["123", "1entry"] {
        let text = build(&GOOD_ARGS, SIZES_OK, &kernel(entry));
        assert_eq!(buffer_slots(&text, entry), Err(MslError::EntryPoint));
    }
}
