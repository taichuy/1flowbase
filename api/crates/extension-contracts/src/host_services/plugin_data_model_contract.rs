use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginStorageBinding {
    Main,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginDataFieldType {
    String,
    Text,
    Number,
    Boolean,
    Datetime,
    Json,
    Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginOwnedField {
    pub field_code: String,
    pub field_type: PluginDataFieldType,
    #[serde(default)]
    pub nullable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginOwnedCollection {
    pub collection_code: String,
    pub fields: Vec<PluginOwnedField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginExtensionField {
    pub target_table: String,
    pub field_code: String,
    pub field_type: PluginDataFieldType,
    pub nullable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginDataModelContribution {
    pub contribution_version: String,
    pub storage_binding: PluginStorageBinding,
    #[serde(default)]
    pub owned_collections: Vec<PluginOwnedCollection>,
    #[serde(default)]
    pub extension_fields: Vec<PluginExtensionField>,
}

impl PluginDataModelContribution {
    pub fn validate_additive_v1(&self) -> Result<(), PluginDataModelContractError> {
        if self.contribution_version != "1flowbase.plugin-data-model/v1" {
            return Err(PluginDataModelContractError::UnsupportedVersion);
        }
        if self.owned_collections.is_empty() && self.extension_fields.is_empty() {
            return Err(PluginDataModelContractError::EmptyContribution);
        }

        let mut collections = BTreeSet::new();
        for collection in &self.owned_collections {
            validate_identifier(&collection.collection_code, "collection_code")?;
            if !collections.insert(collection.collection_code.clone()) {
                return Err(PluginDataModelContractError::DuplicateCollection(
                    collection.collection_code.clone(),
                ));
            }
            if collection.fields.is_empty() {
                return Err(PluginDataModelContractError::EmptyCollection(
                    collection.collection_code.clone(),
                ));
            }
            let mut fields = BTreeSet::new();
            for field in &collection.fields {
                validate_identifier(&field.field_code, "field_code")?;
                if !fields.insert(field.field_code.clone()) {
                    return Err(PluginDataModelContractError::DuplicateOwnedField {
                        collection: collection.collection_code.clone(),
                        field: field.field_code.clone(),
                    });
                }
            }
        }

        let mut extension_fields = BTreeSet::new();
        for field in &self.extension_fields {
            validate_identifier(&field.target_table, "target_table")?;
            validate_identifier(&field.field_code, "field_code")?;
            if !field.nullable {
                return Err(PluginDataModelContractError::ExtensionFieldMustBeNullable {
                    table: field.target_table.clone(),
                    field: field.field_code.clone(),
                });
            }
            if !extension_fields.insert((field.target_table.clone(), field.field_code.clone())) {
                return Err(PluginDataModelContractError::DuplicateExtensionField {
                    table: field.target_table.clone(),
                    field: field.field_code.clone(),
                });
            }
        }
        Ok(())
    }
}

fn validate_identifier(
    value: &str,
    kind: &'static str,
) -> Result<(), PluginDataModelContractError> {
    let mut chars = value.chars();
    if !chars
        .next()
        .is_some_and(|character| character.is_ascii_lowercase())
        || !chars.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
    {
        return Err(PluginDataModelContractError::InvalidIdentifier {
            kind,
            value: value.to_string(),
        });
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PluginDataModelContractError {
    #[error("plugin data model contribution version is unsupported")]
    UnsupportedVersion,
    #[error("plugin data model contribution must declare at least one object")]
    EmptyContribution,
    #[error("{kind} has invalid identifier {value}")]
    InvalidIdentifier { kind: &'static str, value: String },
    #[error("duplicate owned collection {0}")]
    DuplicateCollection(String),
    #[error("owned collection {0} has no fields")]
    EmptyCollection(String),
    #[error("duplicate field {field} in owned collection {collection}")]
    DuplicateOwnedField { collection: String, field: String },
    #[error("extension field {table}.{field} must be nullable")]
    ExtensionFieldMustBeNullable { table: String, field: String },
    #[error("duplicate extension field {table}.{field}")]
    DuplicateExtensionField { table: String, field: String },
}
