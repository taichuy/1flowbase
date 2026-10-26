use super::from_env;
use std::{collections::BTreeMap, time::Duration};

fn parse(
    values: &[(&str, &str)],
) -> anyhow::Result<Option<storage_ephemeral::ReadThroughCachePolicy>> {
    from_env(
        &values
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect::<BTreeMap<_, _>>(),
    )
}

#[test]
fn docs_cache_policy_is_optional_and_requires_explicit_complete_resources() {
    assert!(parse(&[]).unwrap().is_none());
    assert!(parse(&[("API_DOCS_CACHE_MAX_WEIGHT_BYTES", "2048")]).is_err());
    assert!(parse(&[("API_DOCS_CACHE_IDLE_SECONDS", "30")]).is_err());
    let policy = parse(&[
        ("API_DOCS_CACHE_MAX_WEIGHT_BYTES", " 2048 "),
        ("API_DOCS_CACHE_IDLE_SECONDS", "30"),
    ])
    .unwrap()
    .unwrap();
    assert_eq!(policy.max_weight_bytes(), 2048);
    assert_eq!(policy.idle_timeout(), Duration::from_secs(30));
}

#[test]
fn docs_cache_policy_rejects_invalid_zero_and_overflow_values() {
    for (budget, idle) in [
        ("0", "1"),
        ("1", "0"),
        ("-1", "1"),
        ("1", "x"),
        ("18446744073709551616", "1"),
        ("1", "31536000000"),
    ] {
        assert!(
            parse(&[
                ("API_DOCS_CACHE_MAX_WEIGHT_BYTES", budget),
                ("API_DOCS_CACHE_IDLE_SECONDS", idle)
            ])
            .is_err(),
            "{budget}/{idle}"
        );
    }
}
