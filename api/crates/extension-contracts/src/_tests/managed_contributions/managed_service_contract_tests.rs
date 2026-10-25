use crate::ManagedServiceCallContext;
use serde_json::json;

#[test]
fn managed_service_context_is_a_closed_minimal_wire_projection() {
    let wire =
        json!({"actor_id":"actor", "workspace_id":"source-workspace", "deadline_unix_ms":123});
    let context: ManagedServiceCallContext = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(context).unwrap(), wire);
    let mut credentials = wire;
    credentials["session_identity"] = json!("credential");
    assert!(serde_json::from_value::<ManagedServiceCallContext>(credentials).is_err());
    assert!(
        serde_json::from_value::<ManagedServiceCallContext>(json!({"actor_id":"actor"})).is_err()
    );
}
