use std::collections::BTreeSet;

use rand::{rngs::StdRng, SeedableRng};
use storage_durable_postgres::RuntimeTableNamePolicy;

#[test]
fn naming_policy_samples_literals_classes_captures_and_alternations() {
    let policy = RuntimeTableNamePolicy::from_config(true, "(app|svc)_([a-c0-2_]{4})").unwrap();
    assert!(policy.auto_prefix_enabled());

    let mut rng = StdRng::seed_from_u64(0x1f10_ba5e);
    let mut seen_branches = BTreeSet::new();
    let mut seen_chars = BTreeSet::new();
    for _ in 0..128 {
        let table = policy
            .build_table_name_with_rng("orders", &mut rng)
            .unwrap();
        let prefix = table.strip_suffix("_orders").expect("table code suffix");
        let (branch, body) = prefix.split_once('_').expect("branch separator");
        assert!(matches!(branch, "app" | "svc"));
        assert_eq!(body.len(), 4);
        assert!(body
            .chars()
            .all(|ch| matches!(ch, 'a'..='c' | '0'..='2' | '_')));
        seen_branches.insert(branch.to_string());
        seen_chars.extend(body.chars());
    }
    assert_eq!(seen_branches.len(), 2);
    assert_eq!(seen_chars.len(), 7);
}

#[test]
fn naming_policy_samples_bounded_and_unbounded_repetitions_within_limits() {
    let bounded = RuntimeTableNamePolicy::from_config(true, "t_[a-z]{2,5}").unwrap();
    let unbounded = RuntimeTableNamePolicy::from_config(true, "u_[0-9]+").unwrap();
    let optional = RuntimeTableNamePolicy::from_config(true, "v_[a-z][0-9]*").unwrap();

    let mut rng = StdRng::seed_from_u64(42);
    let mut bounded_lengths = BTreeSet::new();
    let mut unbounded_lengths = BTreeSet::new();
    for _ in 0..96 {
        let name = bounded
            .build_table_name_with_rng("items", &mut rng)
            .unwrap();
        let prefix = name.strip_suffix("_items").unwrap();
        assert!(prefix.starts_with("t_"));
        assert!((4..=7).contains(&prefix.len()));
        bounded_lengths.insert(prefix.len());

        let uname = unbounded
            .build_table_name_with_rng("items", &mut rng)
            .unwrap();
        let uprefix = uname.strip_suffix("_items").unwrap();
        assert!(uprefix.starts_with("u_"));
        assert!((3..=19).contains(&uprefix.len()));
        assert!(uprefix[2..].chars().all(|ch| ch.is_ascii_digit()));
        unbounded_lengths.insert(uprefix.len());

        let oname = optional
            .build_table_name_with_rng("items", &mut rng)
            .unwrap();
        let oprefix = oname.strip_suffix("_items").unwrap();
        assert!(oprefix.starts_with("v_"));
        assert!((3..=19).contains(&oprefix.len()));
    }
    assert!(bounded_lengths.len() >= 3);
    assert!(unbounded_lengths.len() >= 5);
}

#[test]
fn naming_policy_samples_unicode_hir_classes_and_rejects_invalid_patterns() {
    let unicode_hir = regex_syntax::ParserBuilder::new()
        .unicode(true)
        .build()
        .parse("[a-b9_]{3}")
        .unwrap();
    let mut rng = StdRng::seed_from_u64(7);
    let mut out = String::new();
    crate::model_definition_repository::sample_hir_for_test(&unicode_hir, &mut rng, &mut out);
    assert_eq!(out.len(), 3);
    assert!(out.chars().all(|ch| matches!(ch, 'a' | 'b' | '9' | '_')));

    for invalid in [
        "",
        "[a-z]*",
        "[A-Z]{3}",
        "foo-bar",
        "^abc",
        "[a-z]{62}",
        "(",
    ] {
        assert!(
            RuntimeTableNamePolicy::from_config(true, invalid).is_err(),
            "expected invalid pattern {invalid:?} to fail"
        );
    }

    let disabled = RuntimeTableNamePolicy::from_config(false, "ignored-UPPERCASE").unwrap();
    assert!(!disabled.auto_prefix_enabled());
    let mut rng = StdRng::seed_from_u64(1);
    assert_eq!(
        disabled
            .build_table_name_with_rng("plain_table", &mut rng)
            .unwrap(),
        "plain_table"
    );
}
