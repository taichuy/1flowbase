use super::*;

#[tokio::test]
async fn workflow_extension_legacy_parameters_migration_normalizes_current_and_published_mappings()
{
    let pool = isolated_database().await.connect().await.unwrap();
    before_workflow_extension_legacy_parameters_migrator()
        .run(&pool)
        .await
        .unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let workspace_id = seed_workspace(&store, "Legacy Workflow Extension Mapping").await;
    let actor_user_id = seed_user(&store, workspace_id, "legacy-extension-owner").await;
    let application_id =
        seed_application(&store, workspace_id, actor_user_id, "Legacy Extension").await;
    sqlx::query(
        "update applications set application_type = 'workflow', workflow_trigger_type = 'extension' where id = $1",
    )
    .bind(application_id)
    .execute(&pool)
    .await
    .unwrap();
    let (flow_id, flow_version_id, compiled_plan_id, document) =
        seed_flow_version_and_compiled_plan(&store, application_id, actor_user_id).await;
    let mut legacy_mapping =
        serde_json::to_value(workflow_extension_mapping("legacy-openai")).unwrap();
    legacy_mapping["extension"]["parameters"] = serde_json::json!([]);
    assert_eq!(
        legacy_mapping["extension"]["parameters"],
        serde_json::json!([])
    );
    let publication_id = Uuid::now_v7();

    sqlx::query(
        r#"
        insert into application_api_mappings (
            id, application_id, scope_id, extension_slug, mapping_config, operation_bindings,
            created_by, updated_by
        ) values ($1, $2, (select scope_id from applications where id = $2), $3, $4, '{}'::jsonb, $5, $5)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(application_id)
    .bind("legacy-openai")
    .bind(&legacy_mapping)
    .bind(actor_user_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into application_publication_versions (
            id, application_id, scope_id, flow_id, flow_version_id, compiled_plan_id,
            extension_slug, version_sequence, active, api_enabled, flow_schema_version,
            document_hash, document_snapshot, mapping_snapshot, operation_bindings,
            runtime_profile_snapshot, output_selector, dependency_snapshot, created_by, updated_by
        ) values (
            $1, $2, (select scope_id from applications where id = $2), $3, $4, $5,
            $6, 1, true, true, $7, $8, $9, $10, '{}'::jsonb,
            '{}'::jsonb, '{}'::jsonb, '[]'::jsonb, $11, $11
        )
        "#,
    )
    .bind(publication_id)
    .bind(application_id)
    .bind(flow_id)
    .bind(flow_version_id)
    .bind(compiled_plan_id)
    .bind("legacy-openai")
    .bind(domain::FLOW_SCHEMA_VERSION)
    .bind("sha256:legacy-extension")
    .bind(document)
    .bind(&legacy_mapping)
    .bind(actor_user_id)
    .execute(&pool)
    .await
    .unwrap();

    run_migrations(&pool).await.unwrap();

    let current =
        ApplicationApiMappingRepository::get_application_api_mapping(&store, application_id)
            .await
            .unwrap()
            .unwrap();
    let publications =
        ApplicationPublicationRepository::list_enabled_extension_publications(&store)
            .await
            .unwrap();
    let current_json: serde_json::Value = sqlx::query_scalar(
        "select mapping_config from application_api_mappings where application_id = $1",
    )
    .bind(application_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let published_json: serde_json::Value = sqlx::query_scalar(
        "select mapping_snapshot from application_publication_versions where id = $1",
    )
    .bind(publication_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(current.mapping, workflow_extension_mapping("legacy-openai"));
    assert_eq!(publications.len(), 1);
    assert_eq!(publications[0].id, publication_id);
    assert!(current_json["extension"]["parameters"].is_null());
    assert!(published_json["extension"]["parameters"].is_null());
}

/// Root #1366 AC-003 / AC-006: old snapshots backfill Generate only when identity is unique,
/// and the reversible migration restores the old readable snapshot shape.
#[tokio::test]
async fn operation_binding_migration_backfills_unique_generate_and_rolls_back() {
    let pool = isolated_database().await.connect().await.unwrap();
    before_application_operation_bindings_migrator()
        .run(&pool)
        .await
        .unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let workspace_id = seed_workspace(&store, "Operation Binding Migration").await;
    let actor_user_id = seed_user(&store, workspace_id, "binding-migration-owner").await;
    let target_inventory = [0_usize, 1, 2];
    let mut publications = Vec::new();

    for target_count in target_inventory {
        let application_id = seed_application(
            &store,
            workspace_id,
            actor_user_id,
            &format!("Binding Migration {target_count}"),
        )
        .await;
        let nodes = (0..target_count)
            .map(|index| {
                let node_id = format!("node-llm-{index}");
                (
                    node_id.clone(),
                    serde_json::json!({
                        "node_id": node_id,
                        "node_type": "llm",
                        "llm_runtime": {
                            "provider_instance_id": Uuid::now_v7().to_string(),
                            "provider_code": "fixture_provider",
                            "protocol": "fixture",
                            "model": "fixture-model"
                        }
                    }),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        let (flow_id, flow_version_id, compiled_plan_id, document) =
            seed_flow_version_and_compiled_plan_with_plan(
                &store,
                application_id,
                actor_user_id,
                serde_json::json!({"nodes": nodes}),
            )
            .await;
        let publication_id = Uuid::now_v7();
        sqlx::query(
            r#"
            insert into application_publication_versions (
                id, application_id, scope_id, flow_id, flow_version_id, compiled_plan_id,
                extension_slug, version_sequence, active, api_enabled, flow_schema_version,
                document_hash, document_snapshot, mapping_snapshot, runtime_profile_snapshot,
                output_selector, dependency_snapshot, created_by, updated_by
            ) values (
                $1, $2, (select scope_id from applications where id = $2), $3, $4, $5,
                null, 1, true, true, $6, $7, $8, $9, '{}'::jsonb, '{}'::jsonb,
                '[]'::jsonb, $10, $10
            )
            "#,
        )
        .bind(publication_id)
        .bind(application_id)
        .bind(flow_id)
        .bind(flow_version_id)
        .bind(compiled_plan_id)
        .bind(domain::FLOW_SCHEMA_VERSION)
        .bind(format!("sha256:binding-{target_count}"))
        .bind(document)
        .bind(serde_json::to_value(ApplicationApiMappingConfig::default_native()).unwrap())
        .bind(actor_user_id)
        .execute(&pool)
        .await
        .unwrap();
        publications.push((target_count, publication_id));
    }

    application_operation_bindings_migrator()
        .run(&pool)
        .await
        .unwrap();

    for (target_count, publication_id) in &publications {
        let bindings: serde_json::Value = sqlx::query_scalar(
            "select operation_bindings from application_publication_versions where id = $1",
        )
        .bind(publication_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(bindings["count_tokens"].is_null());
        assert!(bindings["compact"]["responses_compact"].is_null());
        assert!(bindings["compact"]["responses_compaction_v2"].is_null());
        if *target_count == 1 {
            assert_eq!(
                bindings["generate"]["target_node_id"],
                serde_json::json!("node-llm-0")
            );
        } else {
            assert!(bindings["generate"].is_null());
        }
    }

    application_operation_bindings_migrator()
        .undo(&pool, APPLICATION_OPERATION_BINDINGS_MIGRATION_VERSION - 1)
        .await
        .unwrap();
    let binding_columns: i64 = sqlx::query_scalar(
        r#"
        select count(*)::bigint
        from information_schema.columns
        where table_schema = current_schema()
          and table_name in ('application_api_mappings', 'application_publication_versions')
          and column_name = 'operation_bindings'
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let old_mapping_snapshot: serde_json::Value = sqlx::query_scalar(
        "select mapping_snapshot from application_publication_versions where id = $1",
    )
    .bind(publications[1].1)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(binding_columns, 0);
    assert_eq!(
        serde_json::from_value::<ApplicationApiMappingConfig>(old_mapping_snapshot).unwrap(),
        ApplicationApiMappingConfig::default_native()
    );
}

#[tokio::test]
async fn application_public_api_repository_migration_creates_publication_core_tables_and_indexes() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let schema = current_schema(&pool).await;

    let tables: Vec<String> = sqlx::query_scalar(
        r#"
        select table_name
        from information_schema.tables
        where table_schema = $1
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    let schedule_columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_schema = $1
          and table_name = 'workflow_schedule_triggers'
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    let application_columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_schema = $1
          and table_name = 'applications'
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    let mapping_columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_schema = $1
          and table_name = 'application_api_mappings'
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    let publication_columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_schema = $1
          and table_name = 'application_publication_versions'
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    let extension_slug_indexes: Vec<String> = sqlx::query_scalar(
        r#"
        select indexname
        from pg_indexes
        where schemaname = $1
          and indexname in (
              'application_api_mappings_extension_slug_uidx',
              'application_publication_versions_extension_slug_uidx'
          )
        order by indexname
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    let partial_active_publication_indexes: Vec<String> = sqlx::query_scalar(
        r#"
        select indexdef
        from pg_indexes
        where schemaname = $1
          and tablename = 'application_publication_versions'
          and indexdef ilike '%where active%'
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    let current_publication_indexes: Vec<String> = sqlx::query_scalar(
        r#"
        select indexdef
        from pg_indexes
        where schemaname = $1
          and tablename = 'application_publication_versions'
          and indexname = 'application_publication_versions_application_id_idx'
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    let schedule_indexes: Vec<String> = sqlx::query_scalar(
        r#"
        select indexname
        from pg_indexes
        where schemaname = $1
          and tablename = 'workflow_schedule_triggers'
        order by indexname
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert!(tables.contains(&"application_api_mappings".to_string()));
    assert!(tables.contains(&"application_publication_versions".to_string()));
    assert!(tables.contains(&"workflow_schedule_triggers".to_string()));
    assert!(application_columns.contains(&"api_enabled".to_string()));
    assert!(mapping_columns.contains(&"extension_slug".to_string()));
    assert!(!mapping_columns.contains(&"operation_bindings".to_string()));
    assert!(!publication_columns.contains(&"operation_bindings".to_string()));
    for expected_column in [
        "application_id",
        "flow_id",
        "flow_version_id",
        "compiled_plan_id",
        "version_sequence",
        "active",
        "api_enabled",
        "document_snapshot",
        "mapping_snapshot",
        "runtime_profile_snapshot",
        "output_selector",
        "dependency_snapshot",
        "extension_slug",
    ] {
        assert!(
            publication_columns.contains(&expected_column.to_string()),
            "missing application_publication_versions.{expected_column}"
        );
    }
    assert!(
        partial_active_publication_indexes.is_empty(),
        "current publication is unique per application and should not rely on a partial active index"
    );
    assert!(
        current_publication_indexes.len() == 1 && current_publication_indexes[0].contains("UNIQUE"),
        "a unique application_id index should enforce one current publication per application"
    );
    assert_eq!(
        extension_slug_indexes,
        vec![
            "application_api_mappings_extension_slug_uidx".to_string(),
            "application_publication_versions_extension_slug_uidx".to_string(),
        ]
    );
    for expected_column in [
        "application_id",
        "scope_id",
        "enabled",
        "cron",
        "timezone",
        "input_payload",
    ] {
        assert!(
            schedule_columns.contains(&expected_column.to_string()),
            "missing workflow_schedule_triggers.{expected_column}"
        );
    }
    assert!(
        schedule_indexes.contains(&"workflow_schedule_triggers_application_id_uidx".to_string())
    );
}
