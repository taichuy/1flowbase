use super::*;

// #2032 AC-002/006/008: explicit identity, never text/time/cursor grouping.
#[test]
fn explicit_identity_agrees_across_sources_and_conflicts_fail_closed() {
    assert_eq!(
        resolve_client_log_identity(&[Some("turn-1"), Some("turn-1")]),
        Ok(Some("turn-1".into()))
    );
    assert_eq!(
        resolve_client_log_identity(&[Some("turn-1"), Some("turn-2")]),
        Err("conflicting_identity")
    );
    assert_eq!(resolve_client_log_identity(&[None, Some("")]), Ok(None));
    assert_eq!(
        resolve_client_log_identity(&[Some("\nunsafe")]),
        Err("invalid_identity")
    );
    let oversized = "x".repeat(257);
    assert_eq!(
        resolve_client_log_identity(&[Some(&oversized)]),
        Err("invalid_identity")
    );
}
