use super::*;
use serde_json::json;

#[test]
fn draft_exception_only_accepts_missing_identity_and_rejects_forged_selectors() {
    assert_eq!(
        selector_value(&json!({"body":{}}), "/body/host_id", true).unwrap(),
        None
    );
    assert_eq!(
        selector_value(&json!({"body":{"host_id":"real"}}), "/body/host_id", true).unwrap(),
        Some("real")
    );
    for payload in [
        json!({"body":{"host_id":""}}),
        json!({"body":{"host_id":42}}),
        json!({"body":{"host_id":{"actor":"root"}}}),
    ] {
        assert!(selector_value(&payload, "/body/host_id", true).is_err());
    }
    assert!(selector_value(&json!({"path":{}}), "/path/session_id", false).is_err());
}
fn handle() -> HandleKey {
    HandleKey {
        installation: uuid::Uuid::now_v7(),
        generation: "g1".into(),
        actor: uuid::Uuid::now_v7(),
        workspace: uuid::Uuid::now_v7(),
        resource: "hosts".into(),
        handle: "session-1".into(),
    }
}
#[test]
fn host_handle_cannot_be_rebound_to_another_record_or_replayed_by_other_identity() {
    let original = handle();
    bind_handle(original.clone(), "host-1").unwrap();
    assert_eq!(resolve_handle(&original).unwrap(), "host-1");
    assert!(bind_handle(original.clone(), "host-2").is_err());
    assert_eq!(resolve_handle(&original).unwrap(), "host-1");
    for changed in [
        HandleKey {
            actor: uuid::Uuid::now_v7(),
            ..original.clone()
        },
        HandleKey {
            workspace: uuid::Uuid::now_v7(),
            ..original.clone()
        },
        HandleKey {
            installation: uuid::Uuid::now_v7(),
            ..original.clone()
        },
        HandleKey {
            resource: "other".into(),
            ..original.clone()
        },
        HandleKey {
            generation: "g2".into(),
            ..original.clone()
        },
    ] {
        assert!(resolve_handle(&changed).is_err());
    }
}
#[test]
fn policy_identity_tracks_real_caller_workspace_and_stable_plugin_publisher() {
    let registration = crate::managed_services::tests::fixture();
    let actor = ActorContext::root(uuid::Uuid::now_v7(), uuid::Uuid::now_v7(), "root");
    let resource = ManagedServiceRecordResource {
        resource_code: "hosts".into(),
        collection_code: "hosts".into(),
        id_field: "id".into(),
        label: "Hosts".into(),
        description: "Hosts".into(),
        actions: vec![],
    };
    let record = uuid::Uuid::now_v7().to_string();
    let policy = key(&registration, &actor, &resource, &record).unwrap();
    assert_eq!(policy.workspace_id, actor.current_workspace_id);
    assert_ne!(policy.workspace_id, domain::SYSTEM_SCOPE_ID);
    assert_eq!(
        policy.resource.publisher_namespace,
        registration.publisher_namespace
    );
    let mut upgraded = registration.clone();
    upgraded.plugin_version = "99".into();
    assert_eq!(policy, key(&upgraded, &actor, &resource, &record).unwrap());
    upgraded.publisher_namespace = "other".into();
    assert_ne!(policy, key(&upgraded, &actor, &resource, &record).unwrap());
}

#[test]
fn uuid_spelling_cannot_escape_the_same_record_policy_header() {
    let registration = crate::managed_services::tests::fixture();
    let resource = ManagedServiceRecordResource {
        resource_code: "hosts".into(),
        collection_code: "hosts".into(),
        id_field: "id".into(),
        label: "Hosts".into(),
        description: "Hosts".into(),
        actions: vec![],
    };
    let id = uuid::Uuid::now_v7();
    for raw in [
        id.to_string().to_uppercase(),
        id.simple().to_string(),
        id.to_string(),
    ] {
        assert_eq!(
            canonical_record_id(&registration, &resource, &raw).unwrap(),
            id.to_string()
        );
    }
    assert!(canonical_record_id(&registration, &resource, "not-a-uuid").is_err());
}

#[test]
fn sql_context_uses_real_caller_scope_and_unmapped_operations_fail_closed() {
    let mut registration = crate::managed_services::tests::fixture();
    registration.declaration.record_resources = vec![ManagedServiceRecordResource {
        resource_code: "hosts".into(),
        collection_code: "hosts".into(),
        id_field: "id".into(),
        label: "Hosts".into(),
        description: "Hosts".into(),
        actions: vec![],
    }];
    let mut actor = ActorContext::root(uuid::Uuid::now_v7(), uuid::Uuid::now_v7(), "operator");
    actor.is_root = false;
    let mut operation = registration.declaration.operations[0].clone();
    let context = data_context(&registration, &actor, &operation, None);
    assert_eq!(context.actor_user_id, actor.user_id.to_string());
    assert_eq!(
        context.caller_workspace_id,
        actor.current_workspace_id.to_string()
    );
    assert_ne!(
        context.caller_workspace_id,
        domain::SYSTEM_SCOPE_ID.to_string()
    );
    assert_eq!(context.active_role_code, "operator");
    assert!(!context.is_root);
    assert_eq!(
        context.resources.len(),
        1,
        "unmapped must retain the protected collection"
    );
    assert_eq!(context.resources[0].action_code, "");
    operation.record_permission = Some(plugin_framework::ManagedServiceRecordPermission {
        resource_code: "hosts".into(),
        action_code: "open".into(),
        selector: ManagedServiceRecordSelector::Record {
            pointer: "/body/host_id".into(),
            allow_missing: false,
        },
        response_handle_pointer: None,
    });
    let record = uuid::Uuid::now_v7().to_string();
    let context = data_context(&registration, &actor, &operation, Some(record.clone()));
    assert_eq!(context.resources[0].action_code, "open");
    assert_eq!(
        context.resources[0].record_id.as_deref(),
        Some(record.as_str())
    );
    actor.is_root = true;
    assert!(data_context(&registration, &actor, &operation, None).is_root);
}
