use crate::ports::{McpManagementRepository, ReplaceMcpInstanceDirectoryInput};
use domain::mcp_management::{McpDirectoryBinding, McpDirectoryGroup, McpInstanceDirectory};
use serde_json::json;
use uuid::Uuid;

#[test]
fn mcp_directory_contract_preserves_snapshot_fields_and_optional_new_ids() {
    let instance = Uuid::now_v7();
    let group_id = Uuid::now_v7();
    let binding_id = Uuid::now_v7();
    let tool_id = Uuid::now_v7();
    let snapshot = json!({"instance_record_id":instance,"groups":[{
        "id":group_id,"instance_record_id":instance,"path":"/ops","display_name":"Operations",
        "description_short":null,"enabled":false,"sort_order":-3
    }],"bindings":[{"id":binding_id,"instance_record_id":instance,"tool_record_id":tool_id,
        "tool_id":"read","group_path":"/ops","display_alias":null,"visible":true,"sort_order":5}]});
    let decoded: McpInstanceDirectory = serde_json::from_value(snapshot.clone()).unwrap();
    assert_eq!(serde_json::to_value(&decoded).unwrap(), snapshot);
    let group: McpDirectoryGroup = serde_json::from_value(json!({"path":"/new","display_name":"New","description_short":null,"enabled":true,"sort_order":0})).unwrap();
    assert_eq!(group.id, None);
    let binding: McpDirectoryBinding = serde_json::from_value(json!({"tool_id":"read","group_path":"/","display_alias":null,"visible":true,"sort_order":0})).unwrap();
    assert_eq!(binding.id, None);
    let input = ReplaceMcpInstanceDirectoryInput {
        actor_user_id: Uuid::now_v7(),
        workspace_id: Uuid::now_v7(),
        instance_id: "instance".into(),
        expected_snapshot: decoded,
        groups: vec![group],
        bindings: vec![binding],
    };
    assert_eq!(input.expected_snapshot.instance_record_id, instance);
    fn accepts_repository(_: Option<&dyn McpManagementRepository>) {}
    accepts_repository(None);
}
