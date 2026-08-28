use crate::{
    PluginDataFieldType, PluginDataModelContractError, PluginDataModelContribution,
    PluginExtensionField, PluginOwnedCollection, PluginOwnedField, PluginStorageBinding,
};

fn contribution() -> PluginDataModelContribution {
    PluginDataModelContribution {
        contribution_version: "1flowbase.plugin-data-model/v1".to_string(),
        storage_binding: PluginStorageBinding::Main,
        owned_collections: vec![PluginOwnedCollection {
            collection_code: "affinity_bindings".to_string(),
            fields: vec![PluginOwnedField {
                field_code: "conversation_id".to_string(),
                field_type: PluginDataFieldType::Uuid,
                nullable: false,
            }],
        }],
        extension_fields: vec![PluginExtensionField {
            target_table: "model_provider_instances".to_string(),
            field_code: "affinity_hint".to_string(),
            field_type: PluginDataFieldType::String,
            nullable: true,
        }],
    }
}

#[test]
fn pdm_001_pdm_002_accepts_one_additive_desired_state_without_target_opt_in() {
    contribution().validate_additive_v1().unwrap();
}

#[test]
fn pdm_007_rejects_non_nullable_or_duplicate_extension_fields() {
    let mut non_nullable = contribution();
    non_nullable.extension_fields[0].nullable = false;
    assert!(matches!(
        non_nullable.validate_additive_v1(),
        Err(PluginDataModelContractError::ExtensionFieldMustBeNullable { .. })
    ));

    let mut duplicate = contribution();
    duplicate
        .extension_fields
        .push(duplicate.extension_fields[0].clone());
    assert!(matches!(
        duplicate.validate_additive_v1(),
        Err(PluginDataModelContractError::DuplicateExtensionField { .. })
    ));
}
