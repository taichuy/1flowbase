#[test]
fn model_provider_plugin_cannot_register_routes_or_execute_tools() {
    let policy = control_plane::plugin_isolation::policy_for("model_provider_plugin").unwrap();

    assert!(!policy.can_register_route);
    assert!(!policy.can_execute_host_tool);
    assert_eq!(policy.db_write, "none");
    assert_eq!(policy.host_callback, "provider_event_only");
}

#[test]
fn capability_plugin_must_use_host_api_for_state_changes() {
    let policy = control_plane::plugin_isolation::policy_for("capability_plugin").unwrap();

    assert_eq!(policy.db_write, "host_api_only");
    assert_eq!(policy.host_callback, "capability_result");
    assert!(policy.rate_limit_required);
}

#[test]
fn external_agent_bridge_is_ingress_only() {
    let policy = control_plane::plugin_isolation::policy_for("external_agent_bridge").unwrap();

    assert_eq!(policy.process_model, "ingress_only");
    assert_eq!(policy.host_callback, "telemetry_ingest");
    assert!(!policy.can_register_route);
    assert!(!policy.can_execute_host_tool);
}
