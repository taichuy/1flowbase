use super::*;
use crate::routes::_tests::time_response_cases::assert_timestamp_mapping;

#[test]
fn flow_draft_response_timestamp_contract() {
    assert_timestamp_mapping(|at| {
        let response = to_flow_draft_response(domain::FlowDraftRecord {
            id: Uuid::nil(),
            flow_id: Uuid::nil(),
            schema_version: "1".into(),
            document: serde_json::json!({}),
            updated_at: at,
        })?;
        Ok(vec![response.updated_at])
    });
}

#[test]
fn flow_version_response_timestamp_contract() {
    assert_timestamp_mapping(|at| {
        let response = to_flow_version_response(domain::FlowVersionRecord {
            id: Uuid::nil(),
            flow_id: Uuid::nil(),
            sequence: 1,
            trigger: domain::FlowVersionTrigger::Autosave,
            change_kind: domain::FlowChangeKind::Layout,
            summary: "version".into(),
            summary_is_custom: false,
            is_user_protected: false,
            is_current_publication: false,
            document: serde_json::json!({}),
            created_at: at,
        })?;
        Ok(vec![response.created_at])
    });
}
