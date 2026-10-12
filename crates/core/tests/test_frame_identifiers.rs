//! 試験が宣言する識別子と、閾値の ID・承認の有無の表の振る舞いの確認（spec TASK-23 の 5/8）。
//! 置き場と範囲の説明は docs/test-frame.md の「現状の形（5/8）」。

use fandhe_silicon_core::test_frame::{
    APPROVED_THRESHOLDS, BehaviorId, IdentifierError, MAX_BEHAVIOR_ID_LEN, MAX_TEST_ID_LEN, TestId,
    ThresholdId, ThresholdIdError,
};

#[test]
fn length_limits_are_fixed() {
    assert_eq!(MAX_TEST_ID_LEN, 128);
    assert_eq!(MAX_BEHAVIOR_ID_LEN, 32);
}

#[test]
fn test_id_accepts_valid_names() {
    for n in ["a", "contract_alloc-1", "0", "a_b-c"] {
        let id = TestId::new(n).unwrap();
        assert_eq!(id.as_str(), n);
    }
    assert!(TestId::new(&"a".repeat(MAX_TEST_ID_LEN)).is_ok());
}

#[test]
fn test_id_rejects_invalid_names() {
    assert_eq!(TestId::new(""), Err(IdentifierError::Empty));
    assert_eq!(
        TestId::new(&"a".repeat(MAX_TEST_ID_LEN + 1)),
        Err(IdentifierError::TooLong)
    );
    for n in [
        "A", "a b", " a", "a ", "a/b", "a.b", "a\"b", "a\\b", "a\nb", "a\u{0}b", "あ", "a:b",
        "a;b", "a$b",
    ] {
        assert_eq!(TestId::new(n), Err(IdentifierError::InvalidChar), "{n:?}");
    }
}

#[test]
fn behavior_id_accepts_valid_names() {
    for n in ["BUILD-54", "A", "DEV-17", "X1-2"] {
        assert_eq!(BehaviorId::new(n).unwrap().as_str(), n);
    }
    assert!(BehaviorId::new(&"A".repeat(MAX_BEHAVIOR_ID_LEN)).is_ok());
}

#[test]
fn behavior_id_rejects_invalid_names() {
    assert_eq!(BehaviorId::new(""), Err(IdentifierError::Empty));
    assert_eq!(
        BehaviorId::new(&"A".repeat(MAX_BEHAVIOR_ID_LEN + 1)),
        Err(IdentifierError::TooLong)
    );
    for n in [
        "build-54", "A_B", "A B", "A/B", "A.B", "A\"B", "A\\B", "A\tB", "Ａ",
    ] {
        assert_eq!(
            BehaviorId::new(n),
            Err(IdentifierError::InvalidChar),
            "{n:?}"
        );
    }
}

#[test]
fn identifier_errors_are_ascii_and_do_not_echo_input() {
    let secret = "SECRET/../value";
    let e = BehaviorId::new(secret).unwrap_err();
    for text in [e.to_string(), format!("{e:?}")] {
        assert!(text.is_ascii());
        assert!(!text.contains("SECRET"));
    }
    let e = TestId::new(secret).unwrap_err();
    assert!(e.to_string().is_ascii());
    assert!(!e.to_string().contains("SECRET"));
    let e = ThresholdId::new("t-0x").unwrap_err();
    assert!(e.to_string().is_ascii());
    assert!(!e.to_string().contains("0x"));
}

#[test]
fn threshold_id_accepts_only_the_canonical_spelling() {
    for n in ["t-1", "t-9", "t-10", "t-99", "t-100", "t-999"] {
        let id = ThresholdId::new(n).unwrap();
        assert_eq!(id.as_str(), n);
    }
    for n in [
        "", "t-", "t-0", "t-01", "t-001", "t-1000", "T-1", "t_1", "t1", " t-1", "t-1 ", "t-1\n",
        "t--1", "t-+1", "t-1a", "t-１", "x-1",
    ] {
        assert_eq!(ThresholdId::new(n), Err(ThresholdIdError::Invalid), "{n:?}");
    }
}

/// 7/8: 検査の順（長さが文字より先）と、エラーの表示が固定の文であること（置き場と範囲は docs/test-frame.md の「テストの現状（7/8）」）。
#[test]
fn length_is_checked_before_characters_and_errors_display_fixed_text() {
    assert_eq!(
        TestId::new(&"A".repeat(MAX_TEST_ID_LEN + 1)),
        Err(IdentifierError::TooLong)
    );
    assert_eq!(
        BehaviorId::new(&"a".repeat(MAX_BEHAVIOR_ID_LEN + 1)),
        Err(IdentifierError::TooLong)
    );
    for e in [
        IdentifierError::Empty,
        IdentifierError::TooLong,
        IdentifierError::InvalidChar,
    ] {
        assert_eq!(e.to_string(), e.as_str());
    }
    let t = ThresholdIdError::Invalid;
    assert_eq!(t.to_string(), t.as_str());
}

#[test]
fn approval_table_is_empty_in_public_code() {
    assert!(APPROVED_THRESHOLDS.is_empty());
}

#[test]
fn only_ids_in_the_table_are_approved() {
    for n in ["t-1", "t-12", "t-13", "t-100", "t-999"] {
        assert!(!ThresholdId::new(n).unwrap().is_approved(), "{n}");
    }
}
