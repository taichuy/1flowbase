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

    let prepared = prepare_managed_schema(&state, state.bootstrap_workspace_id, Some(&declaration))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(prepared.preview.entries.len(), 3);
    assert!(prepared
        .preview
        .entries
        .iter()
        .all(|entry| entry.action == "create"));
    let applied = prepared.apply(&state).await.unwrap();
    assert_eq!(applied.created_objects, 3);

    let replay = prepare_managed_schema(&state, state.bootstrap_workspace_id, Some(&declaration))
        .await
        .unwrap()
        .unwrap();
    assert!(replay
        .preview
        .entries
        .iter()
        .all(|entry| entry.action == "already_present"));
    let replay_receipt = replay.apply(&state).await.unwrap();
    assert_eq!(replay_receipt.receipt_id, applied.receipt_id);

    let identity = domain::ExtensionInstallationIdentity {
        category: domain::ExtensionCategory::RuntimeExtensions,
        organization: "fixture".to_string(),
        artifact_id: "managed_schema".to_string(),
        version: "1.0.0".to_string(),
    };
    let retained = retain_managed_schema(&state, state.bootstrap_workspace_id, &identity)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained.retained_objects, 3);
    let ownership = ManagedSchemaRepository::list_managed_schema_ownership(&state.store)
        .await
        .unwrap();
    assert_eq!(ownership.len(), 3);
    assert!(ownership.iter().all(|record| !record.active));
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
