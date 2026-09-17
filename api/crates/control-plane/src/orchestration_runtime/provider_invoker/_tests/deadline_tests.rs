use super::*;

#[test]
fn provider_execution_inherits_available_task_deadline() {
    let now = OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap();
    let expected = now.unix_timestamp() * 1_000 + 42_000;
    let mut input = ProviderInvocationInput::default();
    input
        .run_context
        .insert("task_deadline_unix_ms".to_string(), Value::from(expected));

    assert_eq!(provider_execution_deadline_unix_ms(&input, now), expected);
}

#[test]
fn provider_execution_defaults_to_thirty_minutes() {
    let now = OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap();
    let input = ProviderInvocationInput::default();

    assert_eq!(
        provider_execution_deadline_unix_ms(&input, now),
        now.unix_timestamp() * 1_000 + 30 * 60 * 1_000
    );
}
