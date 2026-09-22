use crate::ports::{WorkflowEventCategory, WorkflowTrajectoryQuery};
use serde_json::json;

#[test]
fn workflow_filters_are_explicit_and_cursor_is_opaque() {
    let defaults: WorkflowTrajectoryQuery = serde_json::from_value(json!({})).unwrap();
    assert_eq!(defaults.category, WorkflowEventCategory::All);
    for category in ["all", "nodes", "requests", "tools", "rounds", "agents"] {
        let query: WorkflowTrajectoryQuery =
            serde_json::from_value(json!({"category":category,"cursor":"opaque-keyset"})).unwrap();
        assert_eq!(query.category.as_str(), category);
        assert_eq!(query.cursor.as_deref(), Some("opaque-keyset"));
    }
    assert!(
        serde_json::from_value::<WorkflowTrajectoryQuery>(json!({"category":"arbitrary"})).is_err()
    );
    assert!(serde_json::from_value::<WorkflowTrajectoryQuery>(
        json!({"node_run_id":"guess-from-name"})
    )
    .is_err());
    assert!(serde_json::from_value::<WorkflowTrajectoryQuery>(json!({"cursor":42})).is_err());
}
