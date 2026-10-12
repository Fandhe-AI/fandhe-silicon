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
