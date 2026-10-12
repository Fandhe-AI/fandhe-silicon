//! 試験の枠の型（組み合わせ・許可・宣言・エラー）の振る舞いの確認（spec TASK-23 の 3/8）。
//! 置き場と範囲の説明は docs/test-frame.md の「現状の形」。

use fandhe_silicon_core::declaration;
use fandhe_silicon_core::test_frame::declaration::{
    MAX_COMBINATIONS, MAX_PERMISSION_LEN, MAX_PERMISSIONS,
};
use fandhe_silicon_core::test_frame::{
    Backend, Declaration, DeclarationError, Machine, MachineCombination, Permission,
    PermissionError,
};

#[test]
fn all_combinations_are_distinct() {
    let all = MachineCombination::ALL;
    for (i, a) in all.iter().enumerate() {
        for b in all.iter().skip(i + 1) {
            assert_ne!(a, b);
            assert_ne!(a.name(), b.name());
        }
    }
}

#[test]
fn name_round_trips_for_every_combination() {
    for c in MachineCombination::ALL {
        assert_eq!(MachineCombination::from_name(c.name()), Some(c));
    }
}

#[test]
fn names_use_only_lowercase_digits_and_hyphen() {
    for c in MachineCombination::ALL {
        assert!(
            c.name()
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        );
        assert!(!c.name().is_empty());
    }
}

#[test]
fn backend_and_machine_are_fixed_per_combination() {
    use MachineCombination::*;
    assert_eq!(MetalMacMini.backend(), Backend::Metal);
    assert_eq!(MetalMacMini.machine(), Machine::MacMini);
    assert_eq!(VulkanNodeB.backend(), Backend::Vulkan);
    assert_eq!(VulkanNodeB.machine(), Machine::NodeB);
    assert_eq!(VulkanNodeA.machine(), Machine::NodeA);
    assert_eq!(VulkanAmd.machine(), Machine::Amd);
    assert_eq!(VulkanIntel.machine(), Machine::Intel);
    assert_eq!(CudaNodeB.backend(), Backend::Cuda);
    assert_eq!(CudaNodeB.machine(), Machine::NodeB);
    assert_eq!(CudaNodeA.backend(), Backend::Cuda);
    assert_eq!(CudaNodeA.machine(), Machine::NodeA);
}

#[test]
fn from_name_is_exact_match_only() {
    assert_eq!(MachineCombination::from_name("unknown"), None);
    assert_eq!(MachineCombination::from_name(""), None);
    assert_eq!(MachineCombination::from_name("Metal-Mac-Mini"), None);
    assert_eq!(MachineCombination::from_name(" metal-mac-mini"), None);
    assert_eq!(MachineCombination::from_name("metal-mac-mini "), None);
}

#[test]
fn permission_accepts_valid_names() {
    assert_eq!(
        Permission::new("fault-injection").unwrap().as_str(),
        "fault-injection"
    );
    assert!(Permission::new("a1").is_ok());
    assert!(Permission::new(&"a".repeat(MAX_PERMISSION_LEN)).is_ok());
}

#[test]
fn permission_rejects_invalid_names() {
    assert_eq!(Permission::new(""), Err(PermissionError::Empty));
    assert_eq!(
        Permission::new(&"a".repeat(MAX_PERMISSION_LEN + 1)),
        Err(PermissionError::TooLong)
    );
    for bad in ["Upper", "has space", "a,b", "caf\u{e9}", "under_score"] {
        assert_eq!(Permission::new(bad), Err(PermissionError::InvalidChar));
    }
}

#[test]
fn declaration_requires_a_combination() {
    assert_eq!(
        Declaration::new(&[], &[]),
        Err(DeclarationError::NoCombination)
    );
}

#[test]
fn declaration_limits_counts() {
    let many = vec![MachineCombination::MetalMacMini; MAX_COMBINATIONS + 1];
    assert_eq!(
        Declaration::new(&many, &[]),
        Err(DeclarationError::TooManyCombinations)
    );
    let names: Vec<String> = (0..=MAX_PERMISSIONS).map(|i| format!("p{i}")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    assert_eq!(
        Declaration::from_parts(&[MachineCombination::MetalMacMini], &refs),
        Err(DeclarationError::TooManyPermissions)
    );
}

#[test]
fn declaration_drops_duplicates() {
    let a = Declaration::from_parts(
        &[MachineCombination::CudaNodeA, MachineCombination::CudaNodeA],
        &["x", "x"],
    )
    .unwrap();
    let b = Declaration::from_parts(&[MachineCombination::CudaNodeA], &["x"]).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.combinations().len(), 1);
    assert_eq!(a.permissions().len(), 1);
}

#[test]
fn from_parts_reports_permission_errors() {
    assert_eq!(
        Declaration::from_parts(&[MachineCombination::CudaNodeA], &["Bad"]),
        Err(DeclarationError::Permission(PermissionError::InvalidChar))
    );
}

#[test]
fn macro_matches_function_api() {
    let by_macro = declaration!([MachineCombination::VulkanAmd], ["perm-a"]).unwrap();
    let by_fn = Declaration::from_parts(&[MachineCombination::VulkanAmd], &["perm-a"]).unwrap();
    assert_eq!(by_macro, by_fn);
    let no_perm = declaration!([MachineCombination::VulkanAmd]).unwrap();
    assert!(no_perm.permissions().is_empty());
    assert_eq!(declaration!([]), Err(DeclarationError::NoCombination));
    assert_eq!(
        declaration!([MachineCombination::VulkanAmd], ["BAD"]),
        Err(DeclarationError::Permission(PermissionError::InvalidChar))
    );
}

#[test]
fn error_messages_are_ascii_and_do_not_echo_input() {
    let input = "Secret Value";
    let e = Permission::new(input).unwrap_err();
    for s in [e.to_string(), DeclarationError::from(e).to_string()] {
        assert!(!s.is_empty());
        assert!(s.is_ascii());
        assert!(!s.contains("Secret"));
    }
    for e in [
        DeclarationError::NoCombination,
        DeclarationError::TooManyCombinations,
        DeclarationError::TooManyPermissions,
    ] {
        assert!(!e.to_string().is_empty());
        assert!(e.to_string().is_ascii());
    }
}

// ---- 7/8: 綴り・対応・境界・検査の順（置き場と範囲は docs/test-frame.md の「テストの現状（7/8）」） ----

#[test]
fn combination_and_machine_names_are_fixed_spellings() {
    let names: Vec<&str> = MachineCombination::ALL.iter().map(|c| c.name()).collect();
    assert_eq!(
        names,
        [
            "metal-mac-mini",
            "vulkan-node-b",
            "vulkan-node-a",
            "vulkan-amd",
            "vulkan-intel",
            "cuda-node-b",
            "cuda-node-a"
        ]
    );
    let machines = [
        (Machine::MacMini, "mac-mini"),
        (Machine::NodeA, "node-a"),
        (Machine::NodeB, "node-b"),
        (Machine::Amd, "amd"),
        (Machine::Intel, "intel"),
    ];
    for (i, (m, n)) in machines.iter().enumerate() {
        assert_eq!(m.name(), *n);
        assert!(
            n.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        );
        assert!(machines[..i].iter().all(|(_, o)| o != n));
    }
}

#[test]
fn every_combination_has_a_fixed_backend_and_machine() {
    use MachineCombination::*;
    let expected = [
        (MetalMacMini, Backend::Metal, Machine::MacMini),
        (VulkanNodeB, Backend::Vulkan, Machine::NodeB),
        (VulkanNodeA, Backend::Vulkan, Machine::NodeA),
        (VulkanAmd, Backend::Vulkan, Machine::Amd),
        (VulkanIntel, Backend::Vulkan, Machine::Intel),
        (CudaNodeB, Backend::Cuda, Machine::NodeB),
        (CudaNodeA, Backend::Cuda, Machine::NodeA),
    ];
    assert_eq!(expected.map(|(c, _, _)| c), MachineCombination::ALL);
    for (c, b, m) in expected {
        assert_eq!(c.backend(), b);
        assert_eq!(c.machine(), m);
    }
    let count = |b: Backend| expected.iter().filter(|(_, x, _)| *x == b).count();
    assert_eq!(
        [
            count(Backend::Metal),
            count(Backend::Vulkan),
            count(Backend::Cuda)
        ],
        [1, 4, 2]
    );
    for (i, (_, b, m)) in expected.iter().enumerate() {
        assert!(expected[..i].iter().all(|(_, b2, m2)| (b2, m2) != (b, m)));
    }
    for m in [
        Machine::MacMini,
        Machine::NodeA,
        Machine::NodeB,
        Machine::Amd,
        Machine::Intel,
    ] {
        assert!(expected.iter().any(|(_, _, x)| *x == m));
    }
}

#[test]
fn combination_name_ends_with_its_machine_name() {
    for c in MachineCombination::ALL {
        assert!(c.name().ends_with(&format!("-{}", c.machine().name())));
    }
}

#[test]
fn declaration_accepts_exact_limits_and_rejects_one_more() {
    let all = MachineCombination::ALL;
    let many: Vec<MachineCombination> = (0..MAX_COMBINATIONS).map(|i| all[i % 7]).collect();
    let d = Declaration::new(&many, &[]).unwrap();
    assert_eq!(d.combinations().len(), 7);

    let perms = |n: usize| -> Vec<Permission> {
        (0..n)
            .map(|i| Permission::new(&format!("p{i}")).unwrap())
            .collect()
    };
    let d = Declaration::new(&all, &perms(MAX_PERMISSIONS)).unwrap();
    assert_eq!(d.permissions().len(), MAX_PERMISSIONS);
    assert_eq!(
        Declaration::new(&all, &perms(MAX_PERMISSIONS + 1)),
        Err(DeclarationError::TooManyPermissions)
    );
}

#[test]
fn count_limit_is_checked_before_permission_names() {
    let mut names: Vec<String> = (0..MAX_PERMISSIONS).map(|i| format!("p{i}")).collect();
    names.push("BAD NAME".to_string());
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    assert_eq!(
        Declaration::from_parts(&[MachineCombination::MetalMacMini], &refs),
        Err(DeclarationError::TooManyPermissions)
    );
}

#[test]
fn macro_accepts_several_items_and_trailing_commas() {
    use MachineCombination::{CudaNodeA, VulkanAmd};
    assert_eq!(
        declaration!([VulkanAmd, CudaNodeA,], ["x", "y",]),
        Declaration::from_parts(&[VulkanAmd, CudaNodeA], &["x", "y"])
    );
    assert_eq!(
        declaration!([VulkanAmd,]),
        Declaration::from_parts(&[VulkanAmd], &[])
    );
}
