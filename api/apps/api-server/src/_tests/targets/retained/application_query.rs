use serde_json::json;
use uuid::Uuid;

#[test]
fn agent_logs_main_conversation_query_rejects_removed_children_scope() {
    assert!(serde_json::from_value::<
        crate::routes::application_runtime::provider_trajectory::RecordConversationQuery,
    >(json!({"scope":"children"}))
    .is_err());
    assert!(serde_json::from_value::<
        crate::routes::application_runtime::provider_trajectory::RecordConversationQuery,
    >(json!({"before":Uuid::nil(),"limit":5}))
    .is_ok());
}
