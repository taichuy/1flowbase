use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const DATA_MODEL_TEMPLATE_DESCRIPTOR_VERSION_V1: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DataModelTemplateIdentity {
    pub provider: String,
    pub code: String,
    pub version: String,
}

impl DataModelTemplateIdentity {
    pub fn canonical_name(&self) -> String {
        format!("{}/{}/{}", self.provider, self.code, self.version)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataModelSourceKind {
    MainSource,
    ExternalSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataModelTemplateSource {
    pub kind: DataModelSourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DataModelTemplateSourceSelector {
    Any,
    MainSource,
    ExternalSource,
    ExternalProvider { provider: String },
}

impl DataModelTemplateSourceSelector {
    pub fn matches(&self, source: &DataModelTemplateSource) -> bool {
        match (self, source.kind) {
            (Self::Any, _) => true,
            (Self::MainSource, DataModelSourceKind::MainSource) => true,
            (Self::ExternalSource, DataModelSourceKind::ExternalSource) => true,
            (Self::ExternalProvider { provider }, DataModelSourceKind::ExternalSource) => {
                source.provider.as_deref() == Some(provider.as_str())
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DataModelCapabilityRequirement {
    pub code: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataModelSystemFieldWritePolicy {
    RuntimeGenerated,
    RuntimeManaged,
    DatabaseGenerated,
    DatabaseManaged,
    ReadOnlyProjection,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataModelTemplateSystemField {
    pub code: String,
    pub value_schema: Value,
    pub required: bool,
    pub write_policy: DataModelSystemFieldWritePolicy,
    pub summary: String,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum DataModelOperationMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl DataModelOperationMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DataModelOperationHandlerRef {
    pub provider: String,
    pub code: String,
    pub version: String,
}

impl DataModelOperationHandlerRef {
    pub fn canonical_name(&self) -> String {
        format!("{}/{}/{}", self.provider, self.code, self.version)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataModelTemplateOperation {
    pub code: String,
    pub method: DataModelOperationMethod,
    pub path: String,
    pub input_schema: Value,
    pub output_schema: Value,
    pub permission_action: String,
    pub handler_ref: DataModelOperationHandlerRef,
    pub summary: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataModelTemplateDescriptor {
    pub descriptor_version: u32,
    pub identity: DataModelTemplateIdentity,
    pub source_selector: DataModelTemplateSourceSelector,
    #[serde(default)]
    pub required_capabilities: Vec<DataModelCapabilityRequirement>,
    #[serde(default)]
    pub system_fields: Vec<DataModelTemplateSystemField>,
    #[serde(default)]
    pub operations: Vec<DataModelTemplateOperation>,
    pub summary: String,
    pub description: String,
}

impl DataModelTemplateDescriptor {
    pub fn validate(&self) -> Result<(), DataModelTemplateContractError> {
        if self.descriptor_version != DATA_MODEL_TEMPLATE_DESCRIPTOR_VERSION_V1 {
            return Err(
                DataModelTemplateContractError::UnsupportedDescriptorVersion(
                    self.descriptor_version,
                ),
            );
        }

        validate_contract_token("identity.provider", &self.identity.provider)?;
        validate_contract_token("identity.code", &self.identity.code)?;
        validate_contract_token("identity.version", &self.identity.version)?;
        validate_text("summary", &self.summary)?;
        validate_text("description", &self.description)?;

        if let DataModelTemplateSourceSelector::ExternalProvider { provider } =
            &self.source_selector
        {
            validate_contract_token("source_selector.provider", provider)?;
        }

        let mut capabilities = BTreeSet::new();
        for capability in &self.required_capabilities {
            validate_contract_token("required_capabilities.code", &capability.code)?;
            if !capabilities.insert(capability.code.as_str()) {
                return Err(DataModelTemplateContractError::DuplicateCapability(
                    capability.code.clone(),
                ));
            }
        }

        if self.system_fields.is_empty() {
            return Err(DataModelTemplateContractError::MissingSystemFields);
        }

        let mut fields = BTreeSet::new();
        for field in &self.system_fields {
            validate_contract_token("system_fields.code", &field.code)?;
            validate_json_schema(
                &format!("system_fields.{}.value_schema", field.code),
                &field.value_schema,
            )?;
            validate_text("system_fields.summary", &field.summary)?;
            validate_text("system_fields.description", &field.description)?;
            if !fields.insert(field.code.as_str()) {
                return Err(DataModelTemplateContractError::DuplicateSystemField(
                    field.code.clone(),
                ));
            }
        }

        if self.operations.is_empty() {
            return Err(DataModelTemplateContractError::MissingOperations);
        }

        let mut operation_codes = BTreeSet::new();
        let mut routes = BTreeSet::new();
        for operation in &self.operations {
            validate_contract_token("operations.code", &operation.code)?;
            validate_operation_path(&operation.path)?;
            validate_json_schema(
                &format!("operations.{}.input_schema", operation.code),
                &operation.input_schema,
            )?;
            validate_json_schema(
                &format!("operations.{}.output_schema", operation.code),
                &operation.output_schema,
            )?;
            validate_contract_token("operations.permission_action", &operation.permission_action)?;
            validate_handler_ref(&operation.handler_ref)?;
            validate_text("operations.summary", &operation.summary)?;
            validate_text("operations.description", &operation.description)?;

            if !operation_codes.insert(operation.code.as_str()) {
                return Err(DataModelTemplateContractError::DuplicateOperationCode(
                    operation.code.clone(),
                ));
            }
            if !routes.insert((operation.method, operation.path.as_str())) {
                return Err(DataModelTemplateContractError::DuplicateOperationRoute {
                    method: operation.method,
                    path: operation.path.clone(),
                });
            }
        }

        Ok(())
    }
}

fn validate_handler_ref(
    handler_ref: &DataModelOperationHandlerRef,
) -> Result<(), DataModelTemplateContractError> {
    validate_contract_token("operations.handler_ref.provider", &handler_ref.provider)?;
    validate_contract_token("operations.handler_ref.code", &handler_ref.code)?;
    validate_contract_token("operations.handler_ref.version", &handler_ref.version)
}

fn validate_contract_token(
    field: &'static str,
    value: &str,
) -> Result<(), DataModelTemplateContractError> {
    if value.is_empty()
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.')
        })
    {
        return Err(DataModelTemplateContractError::InvalidToken {
            field,
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn validate_text(field: &'static str, value: &str) -> Result<(), DataModelTemplateContractError> {
    if value.trim().is_empty() {
        return Err(DataModelTemplateContractError::MissingText(field));
    }
    Ok(())
}

fn validate_json_schema(field: &str, schema: &Value) -> Result<(), DataModelTemplateContractError> {
    let present = match schema {
        Value::Bool(_) => true,
        Value::Object(object) => !object.is_empty(),
        _ => false,
    };
    if !present {
        return Err(DataModelTemplateContractError::MissingJsonSchema(
            field.to_owned(),
        ));
    }
    Ok(())
}

fn validate_operation_path(path: &str) -> Result<(), DataModelTemplateContractError> {
    if !path.starts_with("/api/runtime/")
        || path.contains(char::is_whitespace)
        || path.contains('?')
        || path.contains('#')
        || path.contains("//")
    {
        return Err(DataModelTemplateContractError::InvalidOperationPath(
            path.to_owned(),
        ));
    }

    for segment in path.split('/').filter(|segment| !segment.is_empty()) {
        let has_brace = segment.contains('{') || segment.contains('}');
        if has_brace {
            let parameter = segment
                .strip_prefix('{')
                .and_then(|value| value.strip_suffix('}'));
            if parameter.is_none_or(|value| {
                value.is_empty()
                    || !value.bytes().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'
                    })
            }) {
                return Err(DataModelTemplateContractError::InvalidOperationPath(
                    path.to_owned(),
                ));
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DataModelTemplateContractError {
    #[error("unsupported data model template descriptor version: {0}")]
    UnsupportedDescriptorVersion(u32),
    #[error("invalid data model template token in {field}: {value}")]
    InvalidToken { field: &'static str, value: String },
    #[error("missing data model template text: {0}")]
    MissingText(&'static str),
    #[error("missing JSON schema: {0}")]
    MissingJsonSchema(String),
    #[error("duplicate required capability: {0}")]
    DuplicateCapability(String),
    #[error("data model template must declare at least one system field")]
    MissingSystemFields,
    #[error("duplicate system field: {0}")]
    DuplicateSystemField(String),
    #[error("data model template must declare at least one operation")]
    MissingOperations,
    #[error("duplicate operation code: {0}")]
    DuplicateOperationCode(String),
    #[error("duplicate operation route: {method:?} {path}")]
    DuplicateOperationRoute {
        method: DataModelOperationMethod,
        path: String,
    },
    #[error("invalid data model template operation path: {0}")]
    InvalidOperationPath(String),
}
