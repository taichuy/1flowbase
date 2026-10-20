use super::*;

#[tokio::test]
async fn migration_reconciles_files_grants_and_preserves_remaining_six_visibilities() {
    let pool = files_settings_historical_pool().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace_id = domain::DEFAULT_SCOPE_ID;
    postgres_test_support::insert_historical_workspace(
        store.pool(),
        workspace_id,
        tenant.id,
        "Files settings migration",
    )
    .await
    .unwrap();
    seed_files_settings_historical_permissions(&pool).await;

    let files_role = insert_role(&pool, workspace_id, "legacy_files").await;
    let untouched_role = insert_role(&pool, workspace_id, "legacy_remaining_settings").await;
    grant(&pool, files_role, workspace_id, FILES_VISIBILITY).await;
    grant(&pool, files_role, workspace_id, "file_table.view.all").await;
    for code in REMAINING_VISIBILITIES_AFTER_FILES {
        grant(&pool, untouched_role, workspace_id, code).await;
    }

    before_data_models_settings_feature_migrator()
        .run(&pool)
        .await
        .unwrap();

    assert_eq!(
        permission_codes(&pool, files_role).await,
        BTreeSet::from([
            FILES_FEATURE.to_string(),
            "file_storage.view.all".to_string(),
            "file_storage.manage.all".to_string(),
            "file_table.view.all".to_string(),
            "file_table.view.own".to_string(),
            "file_table.create.all".to_string(),
            "file_table.delete.all".to_string(),
            "file_table.delete.own".to_string(),
            "file_table.bind.all".to_string(),
        ])
    );
    assert_eq!(
        permission_codes(&pool, untouched_role).await,
        REMAINING_VISIBILITIES_AFTER_FILES
            .iter()
            .map(|code| (*code).to_string())
            .collect()
    );

    let removed_legacy_definitions: i64 =
        sqlx::query_scalar("select count(*) from permission_definitions where code = $1")
            .bind(FILES_VISIBILITY)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(removed_legacy_definitions, 0);

    let new_feature_only = insert_role(&pool, workspace_id, "new_files_feature_only").await;
    grant(&pool, new_feature_only, workspace_id, FILES_FEATURE).await;
    assert_eq!(
        permission_codes(&pool, new_feature_only).await,
        BTreeSet::from([FILES_FEATURE.to_string()])
    );
}

#[tokio::test]
async fn files_settings_migration_rolls_back_when_legacy_cleanup_fails() {
    let pool = files_settings_historical_pool().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace_id = domain::DEFAULT_SCOPE_ID;
    postgres_test_support::insert_historical_workspace(
        store.pool(),
        workspace_id,
        tenant.id,
        "Files settings rollback",
    )
    .await
    .unwrap();
    seed_files_settings_historical_permissions(&pool).await;
    let role_id = insert_role(&pool, workspace_id, "legacy_files").await;
    grant(&pool, role_id, workspace_id, FILES_VISIBILITY).await;

    sqlx::raw_sql(
        r#"
        create function reject_files_settings_visibility_delete() returns trigger language plpgsql as $$
        begin
            if old.code = 'settings_route.visible.settings.files' then
                raise exception 'forced files settings cleanup failure';
            end if;
            return old;
        end;
        $$;
        create trigger reject_files_settings_visibility_delete
        before delete on permission_definitions
        for each row execute function reject_files_settings_visibility_delete();
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let error = sqlx::migrate!("./migrations").run(&pool).await.unwrap_err();
    assert!(error
        .to_string()
        .contains("forced files settings cleanup failure"));
    assert_eq!(
        permission_codes(&pool, role_id).await,
        BTreeSet::from([FILES_VISIBILITY.to_string()])
    );
    let new_definition_count: i64 =
        sqlx::query_scalar("select count(*) from permission_definitions where code = $1")
            .bind(FILES_FEATURE)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(new_definition_count, 0);
}

#[tokio::test]
async fn migration_reconciles_data_models_grants_and_preserves_remaining_five_visibilities() {
    let pool = data_models_settings_historical_pool().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace_id = domain::DEFAULT_SCOPE_ID;
    postgres_test_support::insert_historical_workspace(
        store.pool(),
        workspace_id,
        tenant.id,
        "Data models settings migration",
    )
    .await
    .unwrap();
    seed_data_models_settings_historical_permissions(&pool).await;

    let data_models_role = insert_role(&pool, workspace_id, "legacy_data_models").await;
    let untouched_role = insert_role(&pool, workspace_id, "legacy_remaining_five").await;
    grant(
        &pool,
        data_models_role,
        workspace_id,
        DATA_MODELS_VISIBILITY,
    )
    .await;
    grant(
        &pool,
        data_models_role,
        workspace_id,
        "state_model.view.all",
    )
    .await;
    for code in REMAINING_VISIBILITIES_AFTER_DATA_MODELS {
        grant(&pool, untouched_role, workspace_id, code).await;
    }

    before_model_providers_settings_feature_migrator()
        .run(&pool)
        .await
        .unwrap();

    let mut expected = DATA_MODEL_ALL_PERMISSIONS
        .iter()
        .map(|code| (*code).to_string())
        .collect::<BTreeSet<_>>();
    expected.insert(DATA_MODELS_FEATURE.to_string());
    assert_eq!(permission_codes(&pool, data_models_role).await, expected);
    assert_eq!(
        permission_codes(&pool, untouched_role).await,
        REMAINING_VISIBILITIES_AFTER_DATA_MODELS
            .iter()
            .map(|code| (*code).to_string())
            .collect()
    );
    let removed_legacy_definition: i64 =
        sqlx::query_scalar("select count(*) from permission_definitions where code = $1")
            .bind(DATA_MODELS_VISIBILITY)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(removed_legacy_definition, 0);

    let feature_only = insert_role(&pool, workspace_id, "new_data_models_feature_only").await;
    grant(&pool, feature_only, workspace_id, DATA_MODELS_FEATURE).await;
    assert_eq!(
        permission_codes(&pool, feature_only).await,
        BTreeSet::from([DATA_MODELS_FEATURE.to_string()])
    );
}

#[tokio::test]
async fn data_models_settings_migration_rolls_back_when_legacy_cleanup_fails() {
    let pool = data_models_settings_historical_pool().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace_id = domain::DEFAULT_SCOPE_ID;
    postgres_test_support::insert_historical_workspace(
        store.pool(),
        workspace_id,
        tenant.id,
        "Data models settings rollback",
    )
    .await
    .unwrap();
    seed_data_models_settings_historical_permissions(&pool).await;
    let role_id = insert_role(&pool, workspace_id, "legacy_data_models").await;
    grant(&pool, role_id, workspace_id, DATA_MODELS_VISIBILITY).await;

    sqlx::raw_sql(
        r#"
        create function reject_data_models_settings_visibility_delete() returns trigger language plpgsql as $$
        begin
            if old.code = 'settings_route.visible.settings.data-models' then
                raise exception 'forced data models settings cleanup failure';
            end if;
            return old;
        end;
        $$;
        create trigger reject_data_models_settings_visibility_delete
        before delete on permission_definitions
        for each row execute function reject_data_models_settings_visibility_delete();
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let error = sqlx::migrate!("./migrations").run(&pool).await.unwrap_err();
    assert!(error
        .to_string()
        .contains("forced data models settings cleanup failure"));
    assert_eq!(
        permission_codes(&pool, role_id).await,
        BTreeSet::from([DATA_MODELS_VISIBILITY.to_string()])
    );
    let new_definition_count: i64 =
        sqlx::query_scalar("select count(*) from permission_definitions where code = $1")
            .bind(DATA_MODELS_FEATURE)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(new_definition_count, 0);
}

#[tokio::test]
async fn migration_reconciles_model_providers_grants_and_preserves_remaining_four_visibilities() {
    let pool = model_providers_settings_historical_pool().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace_id = domain::DEFAULT_SCOPE_ID;
    postgres_test_support::insert_historical_workspace(
        store.pool(),
        workspace_id,
        tenant.id,
        "Model providers settings migration",
    )
    .await
    .unwrap();
    seed_model_providers_settings_historical_permissions(&pool).await;

    let legacy_role = insert_role(&pool, workspace_id, "legacy_model_providers").await;
    let untouched_role = insert_role(&pool, workspace_id, "legacy_remaining_four").await;
    grant(&pool, legacy_role, workspace_id, MODEL_PROVIDERS_VISIBILITY).await;
    grant(&pool, legacy_role, workspace_id, "state_model.view.all").await;
    for code in REMAINING_VISIBILITIES_AFTER_MODEL_PROVIDERS {
        grant(&pool, untouched_role, workspace_id, code).await;
    }

    before_final_settings_feature_migrator()
        .run(&pool)
        .await
        .unwrap();

    let mut expected = MODEL_PROVIDER_ALL_PERMISSIONS
        .iter()
        .map(|code| (*code).to_string())
        .collect::<BTreeSet<_>>();
    expected.insert(MODEL_PROVIDERS_FEATURE.to_string());
    assert_eq!(permission_codes(&pool, legacy_role).await, expected);
    assert_eq!(
        permission_codes(&pool, untouched_role).await,
        REMAINING_VISIBILITIES_AFTER_MODEL_PROVIDERS
            .iter()
            .map(|code| (*code).to_string())
            .collect()
    );
    let removed_legacy_definition: i64 =
        sqlx::query_scalar("select count(*) from permission_definitions where code = $1")
            .bind(MODEL_PROVIDERS_VISIBILITY)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(removed_legacy_definition, 0);

    let feature_only = insert_role(&pool, workspace_id, "new_model_providers_feature_only").await;
    grant(&pool, feature_only, workspace_id, MODEL_PROVIDERS_FEATURE).await;
    assert_eq!(
        permission_codes(&pool, feature_only).await,
        BTreeSet::from([MODEL_PROVIDERS_FEATURE.to_string()])
    );
}

#[tokio::test]
async fn model_providers_settings_migration_rolls_back_when_legacy_cleanup_fails() {
    let pool = model_providers_settings_historical_pool().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace_id = domain::DEFAULT_SCOPE_ID;
    postgres_test_support::insert_historical_workspace(
        store.pool(),
        workspace_id,
        tenant.id,
        "Model providers settings rollback",
    )
    .await
    .unwrap();
    seed_model_providers_settings_historical_permissions(&pool).await;
    let role_id = insert_role(&pool, workspace_id, "legacy_model_providers").await;
    grant(&pool, role_id, workspace_id, MODEL_PROVIDERS_VISIBILITY).await;

    sqlx::raw_sql(
        r#"
        create function reject_model_providers_settings_visibility_delete() returns trigger language plpgsql as $$
        begin
            if old.code = 'settings_route.visible.settings.model-providers' then
                raise exception 'forced model providers settings cleanup failure';
            end if;
            return old;
        end;
        $$;
        create trigger reject_model_providers_settings_visibility_delete
        before delete on permission_definitions
        for each row execute function reject_model_providers_settings_visibility_delete();
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let error = sqlx::migrate!("./migrations").run(&pool).await.unwrap_err();
    assert!(error
        .to_string()
        .contains("forced model providers settings cleanup failure"));
    assert_eq!(
        permission_codes(&pool, role_id).await,
        BTreeSet::from([MODEL_PROVIDERS_VISIBILITY.to_string()])
    );
    let new_definition_count: i64 =
        sqlx::query_scalar("select count(*) from permission_definitions where code = $1")
            .bind(MODEL_PROVIDERS_FEATURE)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(new_definition_count, 0);
}

#[tokio::test]
async fn final_settings_migration_reconciles_all_remaining_legacy_grants() {
    let pool = final_settings_historical_pool().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace_id = domain::DEFAULT_SCOPE_ID;
    postgres_test_support::insert_historical_workspace(
        store.pool(),
        workspace_id,
        tenant.id,
        "Final settings migration",
    )
    .await
    .unwrap();
    seed_final_settings_historical_permissions(&pool).await;

    let docs_role = insert_role(&pool, workspace_id, "legacy_docs").await;
    let api_key_role = insert_role(&pool, workspace_id, "legacy_api_key").await;
    let system_runtime_role = insert_role(&pool, workspace_id, "legacy_system_runtime").await;
    let mcp_role = insert_role(&pool, workspace_id, "legacy_mcp").await;
    grant(&pool, docs_role, workspace_id, DOCS_VISIBILITY).await;
    grant(
        &pool,
        api_key_role,
        workspace_id,
        API_KEY_AUTHENTICATION_VISIBILITY,
    )
    .await;
    grant(
        &pool,
        system_runtime_role,
        workspace_id,
        SYSTEM_RUNTIME_VISIBILITY,
    )
    .await;
    grant(&pool, mcp_role, workspace_id, MCP_MANAGEMENT_VISIBILITY).await;

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    assert_eq!(
        permission_codes(&pool, docs_role).await,
        BTreeSet::from([
            DOCS_FEATURE.to_string(),
            "api_reference.view.all".to_string(),
        ])
    );
    assert_eq!(
        permission_codes(&pool, api_key_role).await,
        BTreeSet::from([API_KEY_AUTHENTICATION_FEATURE.to_string()])
    );
    assert_eq!(
        permission_codes(&pool, system_runtime_role).await,
        BTreeSet::from([
            SYSTEM_RUNTIME_FEATURE.to_string(),
            "system_runtime.view.all".to_string(),
        ])
    );
    assert_eq!(
        permission_codes(&pool, mcp_role).await,
        BTreeSet::from([
            MCP_MANAGEMENT_FEATURE.to_string(),
            "mcp_management.manage.all".to_string(),
            "mcp_management.view.all".to_string(),
        ])
    );

    let legacy_definition_count: i64 = sqlx::query_scalar(
        "select count(*) from permission_definitions where code like 'settings_route.visible.%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(legacy_definition_count, 0);
}
