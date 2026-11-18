use crate::{
    RecordPermissionGrant, RecordPermissionMode, RecordPermissionPolicy, ReplaceRecordPermissions,
};
use uuid::Uuid;

#[test]
fn missing_policy_inherits_but_restricted_empty_policy_denies() {
    assert!(RecordPermissionPolicy::default().allows("open", &[]));
    let restricted = RecordPermissionPolicy {
        mode: RecordPermissionMode::Restricted,
        grants: vec![],
        revision: 1,
    };
    assert!(!restricted.allows("open", &[]));
    assert!(!restricted.allows("view", &[Uuid::now_v7()]));
}

#[test]
fn role_uuid_and_action_must_both_match_without_implicit_action_hierarchy() {
    let role = Uuid::now_v7();
    let policy = RecordPermissionPolicy {
        mode: RecordPermissionMode::Restricted,
        revision: 1,
        grants: vec![RecordPermissionGrant {
            role_id: role,
            actions: vec!["view".into(), "open".into()],
        }],
    };
    assert!(policy.allows("open", &[Uuid::now_v7(), role]));
    assert!(!policy.allows("update", &[role]));
    assert!(!policy.allows("open", &[Uuid::now_v7()]));
    assert!(!policy.allows("open", &[]));
}

#[test]
fn public_policy_dto_rejects_identity_and_role_alias_injection() {
    let valid = serde_json::json!({"mode":"restricted","grants":[{"role_id":Uuid::now_v7(),"actions":["open"]}],"expected_revision":0});
    let parsed: ReplaceRecordPermissions = serde_json::from_value(valid.clone()).unwrap();
    assert_eq!(parsed.mode, RecordPermissionMode::Restricted);
    for field in ["workspace_id", "is_root", "publisher_namespace", "actor_id"] {
        let mut spoof = valid.clone();
        spoof[field] = serde_json::json!("attacker");
        assert!(serde_json::from_value::<ReplaceRecordPermissions>(spoof).is_err());
    }
    let mut spoof = valid;
    spoof["grants"][0]["role_code"] = serde_json::json!("root");
    assert!(serde_json::from_value::<ReplaceRecordPermissions>(spoof).is_err());
}
