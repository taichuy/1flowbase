use super::*;

#[tokio::test]
async fn external_source_runtime_crud_dispatches_to_data_source_backend_after_acl_scope_resolution()
{
    let backend = Arc::new(CapturingDataSourceBackend::default());
    let model_id = Uuid::now_v7();
    let workspace_id = Uuid::now_v7();
    let actor_user_id = Uuid::now_v7();
    let data_source_instance_id = Uuid::now_v7();
    let engine = RuntimeEngine::for_tests_with_models_and_data_source_backend(
        vec![external_model_metadata(
            model_id,
            workspace_id,
            data_source_instance_id,
        )],
        backend.clone(),
    );
    let actor = ActorContext::scoped(actor_user_id, workspace_id, "member", Vec::<String>::new());
    let grant = RuntimeScopeGrant {
        data_model_id: model_id,
        scope_kind: domain::DataModelScopeKind::Workspace,
        scope_id: workspace_id,
        enabled: true,
        permission_profile: domain::ScopeDataModelPermissionProfile::Owner,
    };

    let listed = engine
        .list_records(RuntimeListInput {
            actor: actor.clone(),
            model_code: "external_contacts".into(),
            filter: domain::ResourceFilterExpr::Field {
                field: "email".into(),
                operator: domain::ResourceFilterOperator::Eq,
                value: json!("ada@example.test"),
            },
            sorts: vec![RuntimeSortInput {
                field_code: "created_at".into(),
                direction: "desc".into(),
            }],
            expand_relations: vec!["company".into()],
            page: 3,
            page_size: 25,
            scope_grant: Some(grant.clone()),
        })
        .await
        .unwrap();
    assert_eq!(
        listed.items,
        vec![json!({
            "id": "external-1",
            "email": "ada@example.test",
            "name": "Ada",
            "created_by": actor_user_id.to_string(),
        })]
    );
    assert_eq!(listed.total, 41);

    let fetched = engine
        .get_record(RuntimeGetInput {
            actor: actor.clone(),
            model_code: "external_contacts".into(),
            record_id: "external-1".into(),
            scope_grant: Some(grant.clone()),
        })
        .await
        .unwrap();
    assert_eq!(
        fetched,
        Some(json!({
            "id": "external-1",
            "email": "ada@example.test",
            "name": "Ada",
            "created_by": actor_user_id.to_string(),
        }))
    );

    let created = engine
        .create_record(RuntimeCreateInput {
            actor: actor.clone(),
            model_code: "external_contacts".into(),
            payload: json!({ "email": "ada@example.test", "name": "Ada" }),
            scope_grant: Some(grant.clone()),
        })
        .await
        .unwrap();
    assert_eq!(created["id"], json!("created-external"));
    assert_eq!(created["email"], json!("ada@example.test"));
    assert_eq!(created["name"], json!("Ada"));
    assert!(created.get("external_only").is_none());

    let updated = engine
        .update_record(RuntimeUpdateInput {
            actor: actor.clone(),
            model_code: "external_contacts".into(),
            record_id: "external-1".into(),
            payload: json!({ "name": "Ada Lovelace" }),
            scope_grant: Some(grant.clone()),
        })
        .await
        .unwrap();
    assert_eq!(updated["name"], json!("Ada Lovelace"));

    let deleted = engine
        .delete_record(RuntimeDeleteInput {
            actor,
            model_code: "external_contacts".into(),
            record_id: "external-1".into(),
            scope_grant: Some(grant),
        })
        .await
        .unwrap();
    assert_eq!(deleted, json!({ "deleted": true }));

    let calls = backend.calls.lock().unwrap();
    assert_eq!(calls.len(), 5);
    assert!(calls
        .iter()
        .all(|call| call.instance_id == data_source_instance_id));
    assert_eq!(calls[0].method, "list");
    assert_eq!(calls[0].payload["resource_key"], json!("crm.contacts"));
    assert_eq!(
        calls[0].payload["context"]["scope_id"],
        json!(workspace_id.to_string())
    );
    assert_eq!(
        calls[0].payload["context"]["owner_id"],
        json!(actor_user_id.to_string())
    );
    assert_eq!(
        calls[0].payload["filters"][0]["field_key"],
        json!("contact_email")
    );
    assert_eq!(
        calls[0].payload["sort"][0]["field_key"],
        json!("created_at_utc")
    );
    assert_eq!(calls[0].payload["sort"][0]["descending"], json!(true));
    assert_eq!(calls[0].payload["page"]["limit"], json!(25));
    assert_eq!(calls[0].payload["page"]["offset"], json!(50));
    assert_eq!(
        calls[0].payload["options_json"]["expand_relations"],
        json!(["company"])
    );
    assert_eq!(calls[1].payload["record_id"], json!("external-1"));
    assert_eq!(
        calls[2].payload["record"],
        json!({ "contact_email": "ada@example.test", "display_name": "Ada" })
    );
    assert_eq!(
        calls[2].payload["context"]["owner_id"],
        json!(actor_user_id.to_string())
    );
    assert_eq!(
        calls[2].payload["context"]["scope_id"],
        json!(workspace_id.to_string())
    );
    assert_eq!(calls[2].payload["transaction_id"], json!(null));
    assert_eq!(
        calls[3].payload["patch"],
        json!({ "display_name": "Ada Lovelace" })
    );
    assert_eq!(calls[3].payload["transaction_id"], json!(null));
    assert_eq!(calls[4].payload["record_id"], json!("external-1"));
    assert_eq!(calls[4].payload["transaction_id"], json!(null));
}

#[tokio::test]
async fn external_source_runtime_rejects_unknown_fields_and_invalid_sort_direction() {
    let backend = Arc::new(CapturingDataSourceBackend::default());
    let model_id = Uuid::now_v7();
    let workspace_id = Uuid::now_v7();
    let engine = RuntimeEngine::for_tests_with_models_and_data_source_backend(
        vec![external_model_metadata(
            model_id,
            workspace_id,
            Uuid::now_v7(),
        )],
        backend.clone(),
    );
    let actor = ActorContext::scoped(Uuid::now_v7(), workspace_id, "member", Vec::<String>::new());
    let grant = RuntimeScopeGrant {
        data_model_id: model_id,
        scope_kind: domain::DataModelScopeKind::Workspace,
        scope_id: workspace_id,
        enabled: true,
        permission_profile: domain::ScopeDataModelPermissionProfile::Owner,
    };

    assert!(engine
        .create_record(RuntimeCreateInput {
            actor: actor.clone(),
            model_code: "external_contacts".into(),
            payload: json!({ "unknown": "value" }),
            scope_grant: Some(grant.clone()),
        })
        .await
        .unwrap_err()
        .to_string()
        .contains("unknown runtime field"));

    assert!(engine
        .update_record(RuntimeUpdateInput {
            actor: actor.clone(),
            model_code: "external_contacts".into(),
            record_id: "external-1".into(),
            payload: json!({ "contact_email": "external-key-should-not-enter" }),
            scope_grant: Some(grant.clone()),
        })
        .await
        .unwrap_err()
        .to_string()
        .contains("unknown runtime field"));

    assert!(engine
        .list_records(RuntimeListInput {
            actor: actor.clone(),
            model_code: "external_contacts".into(),
            filter: domain::ResourceFilterExpr::Field {
                field: "unknown".into(),
                operator: domain::ResourceFilterOperator::Eq,
                value: json!("value"),
            },
            sorts: vec![],
            expand_relations: vec![],
            page: 1,
            page_size: 20,
            scope_grant: Some(grant.clone()),
        })
        .await
        .unwrap_err()
        .to_string()
        .contains("unknown runtime field"));

    assert!(engine
        .list_records(RuntimeListInput {
            actor,
            model_code: "external_contacts".into(),
            filter: domain::ResourceFilterExpr::All(vec![]),
            sorts: vec![RuntimeSortInput {
                field_code: "email".into(),
                direction: "sideways".into(),
            }],
            expand_relations: vec![],
            page: 1,
            page_size: 20,
            scope_grant: Some(grant),
        })
        .await
        .unwrap_err()
        .to_string()
        .contains("unsupported sort direction"));

    assert!(backend.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn ac_013_external_custom_operation_dispatches_canonical_handler_context() {
    let backend = Arc::new(CapturingDataSourceBackend::default());
    let model_id = Uuid::now_v7();
    let workspace_id = Uuid::now_v7();
    let data_source_instance_id = Uuid::now_v7();
    let mut metadata = external_model_metadata(model_id, workspace_id, data_source_instance_id);
    metadata.template_provider = "acme_source".into();
    metadata.template_code = "contact_archive".into();
    metadata.template_version = "v1".into();
    let catalog = runtime_core::data_model_template_registry::DataModelTemplateCatalog::core();
    catalog
        .replace_provider(
            "acme_source@1.0.0",
            "acme_source",
            vec![serde_json::from_value(json!({
                "descriptor_version": 1,
                "identity": { "provider": "acme_source", "code": "contact_archive", "version": "v1" },
                "source_selector": { "kind": "external_provider", "provider": "acme_source" },
                "required_capabilities": [{ "code": "update_record" }],
                "system_fields": [{
                    "code": "id",
                    "value_schema": { "type": "string" },
                    "required": true,
                    "write_policy": "read_only_projection",
                    "summary": "ID",
                    "description": "External identifier."
                }],
                "operations": [{
                    "code": "archive_contact",
                    "method": "POST",
                    "path": "/api/runtime/models/{model_code}/archive/{id}",
                    "input_schema": { "type": "object" },
                    "output_schema": { "type": "object" },
                    "permission_action": "update",
                    "handler_ref": { "provider": "acme_source", "code": "archive_contact", "version": "v1" },
                    "summary": "Archive",
                    "description": "Archive one contact."
                }],
                "summary": "Contact archive",
                "description": "External archive operations."
            }))
            .unwrap()],
        )
        .unwrap();
    let engine = RuntimeEngine::for_tests_with_models_data_source_backend_and_templates(
        vec![metadata],
        backend.clone(),
        catalog,
    );
    let actor = ActorContext::scoped(Uuid::now_v7(), workspace_id, "member", Vec::<String>::new());

    let output = engine
        .execute_model_operation(RuntimeModelOperationInput {
            actor: actor.clone(),
            model_code: "external_contacts".into(),
            operation_code: "archive_contact".into(),
            payload: json!({ "reason": "duplicate" }),
            path: json!({ "id": "external-1" }),
            query: json!({ "notify": false }),
            scope_grant: Some(scope_grant(model_id, workspace_id)),
        })
        .await
        .unwrap();

    assert_eq!(output["archived"], true);
    let calls = backend.calls.lock().unwrap();
    let call = calls.last().unwrap();
    assert_eq!(call.method, "model_operation");
    assert_eq!(call.payload["handler_ref"]["provider"], "acme_source");
    assert_eq!(call.payload["resource_key"], "crm.contacts");
    assert_eq!(call.payload["actor"]["actor_id"], actor.user_id.to_string());
    assert_eq!(call.payload["scope"]["scope_id"], workspace_id.to_string());
}
