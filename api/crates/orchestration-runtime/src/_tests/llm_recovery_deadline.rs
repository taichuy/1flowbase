use super::*;

#[test]
fn inherited_expired_deadline_is_never_refreshed() {
    let now = OffsetDateTime::now_utc();
    let expired = unix_millis(now) - 1;
    let mut input = ProviderInvocationInput::default();
    input
        .run_context
        .insert("task_deadline_unix_ms".into(), json!(expired));
    assert_eq!(recovery_absolute_deadline(&input, now), expired);
}

#[test]
fn absent_deadline_gets_one_initial_window() {
    let now = OffsetDateTime::now_utc();
    assert_eq!(
        recovery_absolute_deadline(&ProviderInvocationInput::default(), now),
        unix_millis(now + time::Duration::minutes(30))
    );
}
