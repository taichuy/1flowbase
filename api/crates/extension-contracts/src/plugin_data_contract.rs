use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    pin::Pin,
};

use serde::{Deserialize, Serialize};

pub const RUNTIME_HOST_CALL_PROTOCOL_V1: &str = "runtime_host_call/v1";
pub const PLUGIN_DATA_SERVICE_V1: &str = "plugin_data/v1";
pub const RUNTIME_HOST_CALL_CAPABILITY_V1: &str = "runtime_host_call/v1";
const MAX_BATCH_OPERATIONS: usize = 64;
const MAX_FIELDS: usize = 128;
const MAX_FILTERS: usize = 64;
const MAX_PAGE_SIZE: u32 = 100;
const MAX_STRING_BYTES: usize = 64 * 1024;
const MAX_JSON_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginDataPermission {
    Read,
    Write,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginDataBinding {
    pub publisher_namespace: String,
    pub plugin_code: String,
    pub plugin_version: String,
    pub storage_binding: String,
    pub workspace_id: String,
    pub actor_id: Option<String>,
    pub provider_instance_id: String,
    pub permissions: BTreeSet<PluginDataPermission>,
    pub deadline_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum PluginDataValue {
    Null,
    String(String),
    Number(String),
    Boolean(bool),
    Datetime(String),
    Json(serde_json::Value),
    Uuid(String),
}

impl PluginDataValue {
    pub fn validate(&self) -> Result<(), PluginDataError> {
        match self {
            Self::String(value)
            | Self::Number(value)
            | Self::Datetime(value)
            | Self::Uuid(value)
                if value.is_empty() || value.len() > MAX_STRING_BYTES =>
            {
                Err(PluginDataError::invalid("plugin_data_value"))
            }
            Self::Number(value) if value.parse::<f64>().is_err() => {
                Err(PluginDataError::invalid("plugin_data_number"))
            }
            Self::Json(value)
                if serde_json::to_vec(value)
                    .map(|bytes| bytes.len() > MAX_JSON_BYTES)
                    .unwrap_or(true) =>
            {
                Err(PluginDataError::invalid("plugin_data_json"))
            }
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PluginDataTarget {
    OwnedCollection { collection_code: String },
    ExtensionProjection { target_table: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginDataFilterOperator {
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    IsNull,
    IsNotNull,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginDataFilter {
    pub field: String,
    pub operator: PluginDataFilterOperator,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<PluginDataValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginDataOrderDirection {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginDataOrder {
    pub field: String,
    pub direction: PluginDataOrderDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginDataPage {
    pub limit: u32,
    pub offset: u64,
}

impl Default for PluginDataPage {
    fn default() -> Self {
        Self {
            limit: 50,
            offset: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum PluginDataOperation {
    Find {
        target: PluginDataTarget,
        fields: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        filters: Vec<PluginDataFilter>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        order: Vec<PluginDataOrder>,
        #[serde(default)]
        page: PluginDataPage,
    },
    FindOne {
        target: PluginDataTarget,
        fields: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        filters: Vec<PluginDataFilter>,
    },
    Count {
        target: PluginDataTarget,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        filters: Vec<PluginDataFilter>,
    },
    Insert {
        target: PluginDataTarget,
        values: BTreeMap<String, PluginDataValue>,
    },
    Update {
        target: PluginDataTarget,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        filters: Vec<PluginDataFilter>,
        values: BTreeMap<String, PluginDataValue>,
    },
    Delete {
        target: PluginDataTarget,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        filters: Vec<PluginDataFilter>,
    },
    Upsert {
        target: PluginDataTarget,
        identity: BTreeMap<String, PluginDataValue>,
        values: BTreeMap<String, PluginDataValue>,
    },
}

impl PluginDataOperation {
    pub fn permission(&self) -> PluginDataPermission {
        match self {
            Self::Find { .. } | Self::FindOne { .. } | Self::Count { .. } => {
                PluginDataPermission::Read
            }
            Self::Insert { .. }
            | Self::Update { .. }
            | Self::Delete { .. }
            | Self::Upsert { .. } => PluginDataPermission::Write,
        }
    }

    pub fn validate(&self) -> Result<(), PluginDataError> {
        let (target, fields, filters, values) = match self {
            Self::Find {
                target,
                fields,
                filters,
                order,
                page,
            } => {
                if page.limit == 0 || page.limit > MAX_PAGE_SIZE || order.len() > MAX_FIELDS {
                    return Err(PluginDataError::invalid("plugin_data_page"));
                }
                for item in order {
                    validate_identifier(&item.field)?;
                }
                (target, Some(fields), filters.as_slice(), None)
            }
            Self::FindOne {
                target,
                fields,
                filters,
            } => (target, Some(fields), filters.as_slice(), None),
            Self::Count { target, filters } | Self::Delete { target, filters } => {
                (target, None, filters.as_slice(), None)
            }
            Self::Insert { target, values } => (target, None, &[][..], Some(values)),
            Self::Update {
                target,
                filters,
                values,
            } => (target, None, filters.as_slice(), Some(values)),
            Self::Upsert {
                target,
                identity,
                values,
            } => {
                validate_values(identity)?;
                if identity.is_empty() {
                    return Err(PluginDataError::invalid("plugin_data_identity"));
                }
                (target, None, &[][..], Some(values))
            }
        };
        validate_target(target)?;
        if let Some(fields) = fields {
            if fields.is_empty() || fields.len() > MAX_FIELDS {
                return Err(PluginDataError::invalid("plugin_data_fields"));
            }
            for field in fields {
                validate_identifier(field)?;
            }
        }
        if filters.len() > MAX_FILTERS {
            return Err(PluginDataError::invalid("plugin_data_filters"));
        }
        for filter in filters {
            validate_identifier(&filter.field)?;
            let requires_value = !matches!(
                filter.operator,
                PluginDataFilterOperator::IsNull | PluginDataFilterOperator::IsNotNull
            );
            if requires_value != filter.value.is_some() {
                return Err(PluginDataError::invalid("plugin_data_filter_value"));
            }
            if let Some(value) = &filter.value {
                value.validate()?;
            }
        }
        if let Some(values) = values {
            validate_values(values)?;
            if values.is_empty() {
                return Err(PluginDataError::invalid("plugin_data_values"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginDataRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    pub operations: Vec<PluginDataOperation>,
}

impl PluginDataRequest {
    pub fn validate(&self) -> Result<(), PluginDataError> {
        if self.operations.is_empty() || self.operations.len() > MAX_BATCH_OPERATIONS {
            return Err(PluginDataError::invalid("plugin_data_operations"));
        }
        if self
            .idempotency_key
            .as_ref()
            .is_some_and(|value| value.is_empty() || value.len() > 256 || !value.is_ascii())
        {
            return Err(PluginDataError::invalid("plugin_data_idempotency_key"));
        }
        for operation in &self.operations {
            operation.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginDataRow {
    pub values: BTreeMap<String, PluginDataValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case", deny_unknown_fields)]
pub enum PluginDataOperationResult {
    Rows { rows: Vec<PluginDataRow> },
    OptionalRow { row: Option<PluginDataRow> },
    Count { count: u64 },
    Mutation { affected: u64 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginDataResponse {
    pub results: Vec<PluginDataOperationResult>,
    pub replayed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginDataErrorKind {
    InvalidRequest,
    PermissionDenied,
    OwnershipDenied,
    Conflict,
    NotFound,
    DeadlineExceeded,
    Cancelled,
    StorageUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{kind:?}: {code}")]
#[serde(deny_unknown_fields)]
pub struct PluginDataError {
    pub kind: PluginDataErrorKind,
    pub code: String,
    pub retryable: bool,
}

impl PluginDataError {
    pub fn invalid(code: impl Into<String>) -> Self {
        Self {
            kind: PluginDataErrorKind::InvalidRequest,
            code: code.into(),
            retryable: false,
        }
    }
}

pub type PluginDataFuture<'a> =
    Pin<Box<dyn Future<Output = Result<PluginDataResponse, PluginDataError>> + Send + 'a>>;

pub trait PluginDataPort: Send + Sync {
    fn execute<'a>(
        &'a self,
        binding: &'a PluginDataBinding,
        request: &'a PluginDataRequest,
    ) -> PluginDataFuture<'a>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "frame", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeHostWorkerFrame {
    HostCall {
        protocol: String,
        call_id: String,
        service: String,
        request: PluginDataRequest,
    },
    HostCancel {
        protocol: String,
        call_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "frame", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeHostFrame {
    HostResult {
        protocol: String,
        call_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        result: Option<PluginDataResponse>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<PluginDataError>,
    },
}

fn validate_target(target: &PluginDataTarget) -> Result<(), PluginDataError> {
    match target {
        PluginDataTarget::OwnedCollection { collection_code } => {
            validate_identifier(collection_code)
        }
        PluginDataTarget::ExtensionProjection { target_table } => validate_identifier(target_table),
    }
}

fn validate_values(values: &BTreeMap<String, PluginDataValue>) -> Result<(), PluginDataError> {
    if values.len() > MAX_FIELDS {
        return Err(PluginDataError::invalid("plugin_data_values"));
    }
    for (field, value) in values {
        validate_identifier(field)?;
        value.validate()?;
    }
    Ok(())
}

fn validate_identifier(value: &str) -> Result<(), PluginDataError> {
    if value.is_empty()
        || value.len() > 63
        || !value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
    {
        return Err(PluginDataError::invalid("plugin_data_identifier"));
    }
    Ok(())
}
