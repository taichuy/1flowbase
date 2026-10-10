use control_plane::ports::{ManagedSchemaRepository, ModelDefinitionRepository};
use extension_contracts::{
    PluginDataFieldType, PluginDataModelContribution, PluginExtensionField, PluginOwnedCollection,
    PluginOwnedField, PluginStorageBinding,
};

use super::*;

fn declaration(target_table: String) -> ManagedSchemaDeclaration {
    ManagedSchemaDeclaration {
        owner: PluginSchemaOwner {
            publisher_namespace: "fixture".to_string(),
            plugin_code: "managed_schema".to_string(),
            plugin_version: "1.0.0".to_string(),
        },
        contributions: vec![PluginDataModelContribution {
            contribution_version: "1flowbase.plugin-data-model/v1".to_string(),
            storage_binding: PluginStorageBinding::Main,
            owned_collections: vec![PluginOwnedCollection {
                collection_code: "affinity".to_string(),
                fields: vec![PluginOwnedField {
                    field_code: "conversation_id".to_string(),
                    field_type: PluginDataFieldType::Uuid,
                    nullable: false,
                }],
            }],
            extension_fields: vec![PluginExtensionField {
                target_table,
                field_code: "affinity_hint".to_string(),
                field_type: PluginDataFieldType::String,
                nullable: true,
            }],
        }],
    }
}

#[tokio::test]
async fn pdm_003_009_composition_previews_applies_and_retains_the_compiled_plan() {
    let (state, _database_url) = crate::_tests::support::test_api_state_with_database_url().await;
    let models = ModelDefinitionRepository::list_model_definitions(
        &state.store,
        state.bootstrap_workspace_id,
    )
    .await
    .unwrap();
    let mut target = None;
    for model in models {
        let exists = sqlx::query_scalar::<_, Option<String>>("select to_regclass($1)::text")
            .bind(&model.physical_table_name)
            .fetch_one(state.store.pool())
            .await
            .unwrap()
            .is_some();
        if exists {
            target = Some(model.physical_table_name);
            break;
        }
    }
    let target = target.expect("a registered physical business-table fixture must exist");
    let declaration = declaration(target);
    let dependencies = super::super::ExtensionCenterDependencies {
        store: state.store.clone(),
        provider_runtime: state.provider_runtime.clone(),
        official_plugin_source: state.official_plugin_source.clone(),
        official_mcp_bundle_source: state.official_mcp_bundle_source.clone(),
        official_extension_catalog_source: state.official_extension_catalog_source.clone(),
        cache_store: state.infrastructure.cache_store(),
        provider_install_root: state.provider_install_root.clone(),
        api_node_id: state.api_node_id.clone(),
        allow_uploaded_host_extensions: state.allow_uploaded_host_extensions,
    };

    let prepared = prepare_managed_schema(
        &dependencies,
        state.bootstrap_workspace_id,
        Some(&declaration),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(prepared.preview.entries.len(), 3);
    assert!(prepared
        .preview
        .entries
        .iter()
        .all(|entry| entry.action == "create"));
    let original_plan = prepared.plan.clone();
    let applied = prepared.apply(&dependencies).await.unwrap();
    assert_eq!(applied.created_objects, 3);

    let replay = prepare_managed_schema(
        &dependencies,
        state.bootstrap_workspace_id,
        Some(&declaration),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(replay
        .preview
        .entries
        .iter()
        .all(|entry| entry.action == "already_present"));
    let replay_receipt = replay.apply(&dependencies).await.unwrap();
    assert_eq!(replay_receipt.receipt_id, applied.receipt_id);

    let identity = domain::ExtensionInstallationIdentity {
        category: domain::ExtensionCategory::RuntimeExtensions,
        organization: "fixture".to_string(),
        artifact_id: "managed_schema".to_string(),
        version: "1.0.0".to_string(),
    };
    let (table, column) = original_plan
        .operations
        .iter()
        .find_map(|op| match op {
            ManagedSchemaOperation::EnsureOwnedField {
                physical_table,
                physical_column,
                ..
            } => Some((physical_table.clone(), physical_column.clone())),
            _ => None,
        })
        .unwrap();
    assert!(table
        .chars()
        .chain(column.chars())
        .all(|c| c.is_ascii_alphanumeric() || c == '_'));
    let host_id = uuid::Uuid::now_v7();
    let preserved_value = uuid::Uuid::now_v7();
    sqlx::query(&format!(
        "insert into \"{table}\" (id, scope_id, \"{column}\") values ($1,$2,$3)"
    ))
    .bind(host_id)
    .bind(domain::SYSTEM_SCOPE_ID)
    .bind(preserved_value)
    .execute(state.store.pool())
    .await
    .unwrap();
    let retained = retain_managed_schema(&dependencies, state.bootstrap_workspace_id, &identity)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained.retained_objects, 3);
    let ownership = ManagedSchemaRepository::list_managed_schema_ownership(&state.store)
        .await
        .unwrap();
    assert_eq!(ownership.len(), 3);
    assert!(ownership.iter().all(|record| !record.active));
    // Receipt reuse must follow current-state reconciliation, including the second enable cycle.
    for _ in 0..2 {
        let restored = ManagedSchemaRepository::apply_managed_schema(&state.store, &original_plan)
            .await
            .unwrap();
        assert_eq!(restored.receipt_id.to_string(), applied.receipt_id);
        let ownership = ManagedSchemaRepository::list_managed_schema_ownership(&state.store)
            .await
            .unwrap();
        assert!(ownership
            .iter()
            .all(|record| record.active && record.owner_version == "1.0.0"));
        let stored: uuid::Uuid =
            sqlx::query_scalar(&format!("select \"{column}\" from \"{table}\" where id=$1"))
                .bind(host_id)
                .fetch_one(state.store.pool())
                .await
                .unwrap();
        assert_eq!(stored, preserved_value);
        retain_managed_schema(&dependencies, state.bootstrap_workspace_id, &identity)
            .await
            .unwrap();
        let ownership = ManagedSchemaRepository::list_managed_schema_ownership(&state.store)
            .await
            .unwrap();
        assert!(ownership.iter().all(|record| !record.active));
    }
}

#[test]
fn pdm_008_malformed_ownership_inventory_fails_closed() {
    let record = ManagedSchemaOwnershipRecord {
        ownership_key: "column:fixture.field".to_string(),
        owner_id: "fixture/managed_schema".to_string(),
        owner_version: "1.0.0".to_string(),
        object_kind: ManagedSchemaObjectKind::OwnedField,
        logical_name: "missing_collection_separator".to_string(),
        physical_table: "fixture".to_string(),
        physical_column: Some("field".to_string()),
        field_type: Some(ManagedSchemaFieldType::String),
        nullable: Some(true),
        active: true,
        plan_fingerprint: "fixture".to_string(),
    };
    assert!(existing_ownership(&record).is_err());
}
