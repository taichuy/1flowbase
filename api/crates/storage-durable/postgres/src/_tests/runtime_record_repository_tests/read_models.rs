use super::*;

#[tokio::test]
async fn runtime_record_repository_registers_builtin_runtime_read_models() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);

    let metadata = store.list_runtime_model_metadata().await.unwrap();
    let model_codes = metadata
        .iter()
        .map(|model| model.model_code.as_str())
        .collect::<Vec<_>>();
    for expected in [
        "application_run_log_summaries",
        "application_conversations",
        "application_conversation_messages",
        "node_runs",
        "flow_run_events",
        "flow_run_checkpoints",
        "flow_run_callback_tasks",
    ] {
        assert!(
            model_codes.contains(&expected),
            "missing builtin runtime read model {expected}"
        );
    }

    let run_logs = metadata
        .iter()
        .find(|model| model.model_code == "application_run_log_summaries")
        .unwrap();
    assert_eq!(
        run_logs.physical_table_name,
        "application_run_log_summaries"
    );
    assert_eq!(run_logs.scope_column_name, "scope_id");
    assert!(run_logs.fields.iter().any(|field| {
        field.code == "flow_run_id"
            && field.physical_column_name == "flow_run_id"
            && !field.is_writable
    }));
    assert!(run_logs.fields.iter().any(|field| {
        field.code == "scope_id" && field.physical_column_name == "scope_id" && !field.is_writable
    }));
    assert!(run_logs.fields.iter().any(|field| {
        field.code == "input_cache_hit_rate"
            && field.physical_column_name == "input_cache_hit_rate"
            && !field.is_writable
    }));
    assert!(run_logs.fields.iter().all(|field| !field.is_writable));

    let node_runs = metadata
        .iter()
        .find(|model| model.model_code == "node_runs")
        .unwrap();
    let node_field_codes = node_runs
        .fields
        .iter()
        .map(|field| field.code.as_str())
        .collect::<Vec<_>>();
    assert!(node_field_codes.contains(&"flow_run_id"));
    assert!(node_field_codes.contains(&"node_run_id"));
    assert!(!node_field_codes.contains(&"input_payload"));
    assert!(!node_field_codes.contains(&"output_payload"));
    assert!(!node_field_codes.contains(&"debug_payload"));
}

#[tokio::test]
async fn runtime_record_repository_lists_registered_system_tables_from_physical_tables() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let workspace_id = Uuid::now_v7();
    let actor_user_id = Uuid::now_v7();
    let attachment_id = Uuid::now_v7();
    let role_id = Uuid::now_v7();

    insert_workspace(&store, workspace_id).await;
    insert_user(&store, actor_user_id, "registered-system-table").await;
    let storage = FileManagementRepository::create_file_storage(
        &store,
        &CreateFileStorageInput {
            storage_id: Uuid::now_v7(),
            actor_user_id,
            code: format!("local_{}", Uuid::now_v7().simple()),
            title: "Local".into(),
            driver_type: "local".into(),
            enabled: true,
            is_default: true,
            config_json: json!({ "root_path": "/tmp/1flowbase-tests", "public_base_url": null }),
            rule_json: json!({}),
        },
    )
    .await
    .unwrap();

    FileManagementBootstrapService::new(store.clone())
        .ensure_builtin_attachments(actor_user_id, storage.id, "attachments")
        .await
        .unwrap();
    SystemMetadataBootstrapService::new(store.clone())
        .ensure_builtin_user_and_role_models(actor_user_id)
        .await
        .unwrap();

    sqlx::query(
        r#"
        insert into roles (
            id, scope_id, scope_kind, workspace_id, code, name, introduction,
            is_builtin, is_editable, auto_grant_new_permissions,
            is_default_member_role, system_kind, created_by, updated_by
        ) values (
            $1, $2, 'workspace', $2, 'qa_reviewer', 'QA Reviewer', '',
            true, false, false, false, null, $3, $3
        )
        "#,
    )
    .bind(role_id)
    .bind(workspace_id)
    .bind(actor_user_id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into attachments (
            id, scope_id, title, filename, extname, size, mimetype, path,
            meta, storage_id, created_by, updated_by
        ) values (
            $1, $2, 'Contract Evidence', 'contract.txt', 'txt', 12,
            'text/plain', 'workspace/attachments/contract.txt', '{"source":"test"}',
            $3, $4, $4
        )
        "#,
    )
    .bind(attachment_id)
    .bind(workspace_id)
    .bind(storage.id)
    .bind(actor_user_id)
    .execute(store.pool())
    .await
    .unwrap();

    let metadata = store.list_runtime_model_metadata().await.unwrap();
    let users = metadata
        .iter()
        .find(|model| model.model_code == "users")
        .expect("users metadata should be registered");
    let roles = metadata
        .iter()
        .find(|model| model.model_code == "roles")
        .expect("roles metadata should be registered");
    let attachments = metadata
        .iter()
        .find(|model| model.model_code == "attachments")
        .expect("attachments metadata should be registered");

    assert_eq!(users.physical_table_name, "users");
    assert_eq!(roles.physical_table_name, "roles");
    assert_eq!(attachments.physical_table_name, "attachments");
    assert!(users
        .fields
        .iter()
        .all(|field| field.code != "password_hash"));
    assert!(users
        .fields
        .iter()
        .all(|field| field.code != "session_version"));

    let user_page = RuntimeRecordRepository::list_records(
        &store,
        users,
        RuntimeListQuery {
            scope_id: None,
            owner_user_id: None,
            filter: domain::ResourceFilterExpr::Field {
                field: "account".into(),
                operator: domain::ResourceFilterOperator::Includes,
                value: json!("registered-system-table"),
            },
            sorts: vec![],
            expand_relations: vec![],
            page: 1,
            page_size: 20,
        },
    )
    .await
    .unwrap();
    assert_eq!(user_page.total, 1);
    assert_eq!(user_page.items[0]["id"], json!(actor_user_id.to_string()));
    assert!(user_page.items[0].get("password_hash").is_none());
    assert!(user_page.items[0].get("session_version").is_none());

    let role_page = RuntimeRecordRepository::list_records(
        &store,
        roles,
        RuntimeListQuery {
            scope_id: Some(workspace_id),
            owner_user_id: None,
            filter: domain::ResourceFilterExpr::Field {
                field: "code".into(),
                operator: domain::ResourceFilterOperator::Eq,
                value: json!("qa_reviewer"),
            },
            sorts: vec![],
            expand_relations: vec![],
            page: 1,
            page_size: 20,
        },
    )
    .await
    .unwrap();
    assert_eq!(role_page.total, 1);
    assert_eq!(role_page.items[0]["id"], json!(role_id.to_string()));
    assert_eq!(
        role_page.items[0]["scope_id"],
        json!(workspace_id.to_string())
    );

    let attachment_page = RuntimeRecordRepository::list_records(
        &store,
        attachments,
        RuntimeListQuery {
            scope_id: Some(workspace_id),
            owner_user_id: None,
            filter: domain::ResourceFilterExpr::Field {
                field: "filename".into(),
                operator: domain::ResourceFilterOperator::Eq,
                value: json!("contract.txt"),
            },
            sorts: vec![],
            expand_relations: vec![],
            page: 1,
            page_size: 20,
        },
    )
    .await
    .unwrap();
    assert_eq!(attachment_page.total, 1);
    assert_eq!(
        attachment_page.items[0]["id"],
        json!(attachment_id.to_string())
    );
    assert_eq!(
        attachment_page.items[0]["path"],
        json!("workspace/attachments/contract.txt")
    );
}

#[tokio::test]
async fn runtime_record_repository_lists_application_run_logs_as_scoped_read_model() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seed = seed_runtime_read_model_rows(&store).await;
    let metadata = store
        .list_runtime_model_metadata()
        .await
        .unwrap()
        .into_iter()
        .find(|model| model.model_code == "application_run_log_summaries")
        .unwrap();

    let page = RuntimeRecordRepository::list_records(
        &store,
        &metadata,
        RuntimeListQuery {
            scope_id: Some(seed.workspace_id),
            owner_user_id: None,
            filter: domain::ResourceFilterExpr::All(vec![
                domain::ResourceFilterExpr::Field {
                    field: "title".into(),
                    operator: domain::ResourceFilterOperator::Includes,
                    value: json!("refund"),
                },
                domain::ResourceFilterExpr::Field {
                    field: "application_id".into(),
                    operator: domain::ResourceFilterOperator::Eq,
                    value: json!(seed.application_id.to_string()),
                },
                domain::ResourceFilterExpr::Field {
                    field: "scope_id".into(),
                    operator: domain::ResourceFilterOperator::Eq,
                    value: json!(seed.workspace_id.to_string()),
                },
                domain::ResourceFilterExpr::Field {
                    field: "started_at".into(),
                    operator: domain::ResourceFilterOperator::Gte,
                    value: json!("2026-05-29T07:59:00Z"),
                },
            ]),
            sorts: vec![RuntimeSortInput {
                field_code: "created_at".into(),
                direction: "desc".into(),
            }],
            expand_relations: vec![],
            page: 1,
            page_size: 1,
        },
    )
    .await
    .unwrap();

    assert_eq!(page.total, 1);
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0]["id"], json!(seed.flow_run_id.to_string()));
    assert_eq!(
        page.items[0]["flow_run_id"],
        json!(seed.flow_run_id.to_string())
    );
    assert_eq!(
        page.items[0]["scope_id"],
        json!(seed.workspace_id.to_string())
    );
    assert_eq!(
        page.items[0]["application_id"],
        json!(seed.application_id.to_string())
    );
}

#[tokio::test]
async fn runtime_record_repository_lists_application_conversation_messages_by_declared_filters() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seed = seed_runtime_read_model_rows(&store).await;
    let conversation_id = Uuid::now_v7();
    let message_id = Uuid::now_v7();
    let created_at = datetime!(2026-05-29 08:01:00 UTC);

    sqlx::query(
        r#"
        insert into application_conversations (
            id, scope_id, application_id, external_conversation_id, external_user,
            created_at, updated_at
        ) values ($1, $2, $3, 'conversation-1', 'customer-1', $4, $4)
        "#,
    )
    .bind(conversation_id)
    .bind(seed.workspace_id)
    .bind(seed.application_id)
    .bind(created_at)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into application_conversation_messages (
            id, scope_id, conversation_id, application_id, flow_run_id, node_run_id,
            role, content, sequence, created_at, updated_at
        ) values (
            $1, $2, $3, $4, $5, $6, 'assistant', 'Refund policy answer', 1, $7, $7
        )
        "#,
    )
    .bind(message_id)
    .bind(seed.workspace_id)
    .bind(conversation_id)
    .bind(seed.application_id)
    .bind(seed.flow_run_id)
    .bind(seed.node_run_id)
    .bind(created_at)
    .execute(store.pool())
    .await
    .unwrap();

    let metadata = store
        .list_runtime_model_metadata()
        .await
        .unwrap()
        .into_iter()
        .find(|model| model.model_code == "application_conversation_messages")
        .unwrap();
    let page = RuntimeRecordRepository::list_records(
        &store,
        &metadata,
        RuntimeListQuery {
            scope_id: Some(seed.workspace_id),
            owner_user_id: None,
            filter: domain::ResourceFilterExpr::All(vec![
                domain::ResourceFilterExpr::Field {
                    field: "conversation_id".into(),
                    operator: domain::ResourceFilterOperator::Eq,
                    value: json!(conversation_id.to_string()),
                },
                domain::ResourceFilterExpr::Field {
                    field: "flow_run_id".into(),
                    operator: domain::ResourceFilterOperator::Eq,
                    value: json!(seed.flow_run_id.to_string()),
                },
                domain::ResourceFilterExpr::Field {
                    field: "role".into(),
                    operator: domain::ResourceFilterOperator::Eq,
                    value: json!("assistant"),
                },
                domain::ResourceFilterExpr::Field {
                    field: "content".into(),
                    operator: domain::ResourceFilterOperator::Includes,
                    value: json!("policy"),
                },
            ]),
            sorts: vec![RuntimeSortInput {
                field_code: "created_at".into(),
                direction: "asc".into(),
            }],
            expand_relations: vec![],
            page: 1,
            page_size: 20,
        },
    )
    .await
    .unwrap();

    assert_eq!(page.total, 1);
    assert_eq!(page.items[0]["id"], json!(message_id.to_string()));
    assert_eq!(page.items[0]["role"], json!("assistant"));
    assert_eq!(page.items[0]["content"], json!("Refund policy answer"));
}

#[tokio::test]
async fn runtime_record_repository_lists_run_detail_shards_without_large_payload_columns() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seed = seed_runtime_read_model_rows(&store).await;

    let metadata = store
        .list_runtime_model_metadata()
        .await
        .unwrap()
        .into_iter()
        .find(|model| model.model_code == "node_runs")
        .unwrap();
    let page = RuntimeRecordRepository::list_records(
        &store,
        &metadata,
        RuntimeListQuery {
            scope_id: Some(seed.workspace_id),
            owner_user_id: None,
            filter: domain::ResourceFilterExpr::All(vec![
                domain::ResourceFilterExpr::Field {
                    field: "flow_run_id".into(),
                    operator: domain::ResourceFilterOperator::Eq,
                    value: json!(seed.flow_run_id.to_string()),
                },
                domain::ResourceFilterExpr::Field {
                    field: "node_run_id".into(),
                    operator: domain::ResourceFilterOperator::Eq,
                    value: json!(seed.node_run_id.to_string()),
                },
            ]),
            sorts: vec![],
            expand_relations: vec![],
            page: 1,
            page_size: 20,
        },
    )
    .await
    .unwrap();

    assert_eq!(page.total, 1);
    assert_eq!(page.items[0]["id"], json!(seed.node_run_id.to_string()));
    assert_eq!(
        page.items[0]["node_run_id"],
        json!(seed.node_run_id.to_string())
    );
    assert!(page.items[0].get("input_payload").is_none());
    assert!(page.items[0].get("output_payload").is_none());
    assert!(page.items[0].get("debug_payload").is_none());
}
