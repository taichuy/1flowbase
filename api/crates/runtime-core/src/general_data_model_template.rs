use std::{collections::BTreeSet, sync::OnceLock};

use plugin_framework::{
    DataModelCapabilityRequirement, DataModelOperationHandlerRef, DataModelOperationMethod,
    DataModelSystemFieldWritePolicy, DataModelTemplateDescriptor, DataModelTemplateIdentity,
    DataModelTemplateOperation, DataModelTemplateSource, DataModelTemplateSourceSelector,
    DataModelTemplateSystemField, DATA_MODEL_TEMPLATE_DESCRIPTOR_VERSION_V1,
};
use serde_json::{json, Value};

use crate::{
    data_model_template_registry::{
        DataModelTemplateRegistry, DataModelTemplateRegistryError,
        DataModelTemplateResolutionContract,
    },
    runtime_acl::RuntimeDataAction,
};

pub const GENERAL_RECORDS_READ_CAPABILITY: &str = "records.read";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreGeneralOperationHandler {
    ListRecords,
    GetRecord,
    CreateRecord,
    UpdateRecord,
    DeleteRecord,
}

impl CoreGeneralOperationHandler {
    pub fn from_ref(handler_ref: &DataModelOperationHandlerRef) -> Option<Self> {
        if handler_ref.provider != "core" || handler_ref.version != "v1" {
            return None;
        }
        match handler_ref.code.as_str() {
            "list_records" => Some(Self::ListRecords),
            "get_record" => Some(Self::GetRecord),
            "create_record" => Some(Self::CreateRecord),
            "update_record" => Some(Self::UpdateRecord),
            "delete_record" => Some(Self::DeleteRecord),
            _ => None,
        }
    }

    pub fn audit_action(self) -> &'static str {
        match self {
            Self::ListRecords => "list",
            Self::GetRecord => "get",
            Self::CreateRecord => "create",
            Self::UpdateRecord => "update",
            Self::DeleteRecord => "delete",
        }
    }
}

pub fn runtime_data_action(permission_action: &str) -> Option<RuntimeDataAction> {
    match permission_action {
        "view" => Some(RuntimeDataAction::View),
        "create" => Some(RuntimeDataAction::Create),
        "update" => Some(RuntimeDataAction::Update),
        "delete" => Some(RuntimeDataAction::Delete),
        _ => None,
    }
}

pub fn general_template_identity() -> DataModelTemplateIdentity {
    DataModelTemplateIdentity {
        provider: domain::CORE_DATA_MODEL_TEMPLATE_PROVIDER.to_owned(),
        code: domain::GENERAL_DATA_MODEL_TEMPLATE_CODE.to_owned(),
        version: domain::GENERAL_DATA_MODEL_TEMPLATE_VERSION.to_owned(),
    }
}

pub fn core_data_model_template_registry(
) -> Result<&'static DataModelTemplateRegistry, DataModelTemplateRegistryError> {
    static REGISTRY: OnceLock<Result<DataModelTemplateRegistry, DataModelTemplateRegistryError>> =
        OnceLock::new();
    REGISTRY
        .get_or_init(|| {
            DataModelTemplateRegistry::compile(
                [
                    general_template_descriptor(),
                    crate::ordered_tree_template::ordered_tree_template_descriptor(),
                ],
                &CoreResolution,
            )
        })
        .as_ref()
        .map_err(Clone::clone)
}

pub fn source_capabilities(
    source: &DataModelTemplateSource,
    external_capabilities: Option<&plugin_framework::DataSourceCrudCapabilities>,
) -> BTreeSet<String> {
    let mut capabilities = BTreeSet::new();
    match source.kind {
        plugin_framework::DataModelSourceKind::MainSource => {
            capabilities.insert(GENERAL_RECORDS_READ_CAPABILITY.to_owned());
        }
        plugin_framework::DataModelSourceKind::ExternalSource => {
            if external_capabilities.is_some_and(|value| value.supports_list && value.supports_get)
            {
                capabilities.insert(GENERAL_RECORDS_READ_CAPABILITY.to_owned());
            }
        }
    }
    capabilities
}

pub fn template_is_compatible(
    identity: &DataModelTemplateIdentity,
    source: &DataModelTemplateSource,
    capabilities: &BTreeSet<String>,
) -> Result<bool, DataModelTemplateRegistryError> {
    let registry = core_data_model_template_registry()?;
    registry.resolve(identity)?;
    Ok(registry
        .compatible_templates(source, capabilities.iter().map(String::as_str))
        .into_iter()
        .any(|template| template.identity() == identity))
}

pub(crate) fn general_template_descriptor() -> DataModelTemplateDescriptor {
    DataModelTemplateDescriptor {
        descriptor_version: DATA_MODEL_TEMPLATE_DESCRIPTOR_VERSION_V1,
        identity: general_template_identity(),
        source_selector: DataModelTemplateSourceSelector::Any,
        required_capabilities: vec![DataModelCapabilityRequirement {
            code: GENERAL_RECORDS_READ_CAPABILITY.to_owned(),
        }],
        system_fields: vec![
            system_field(
                "id",
                json!({ "type": "string", "format": "uuid" }),
                DataModelSystemFieldWritePolicy::RuntimeGenerated,
            ),
            system_field(
                "scope_id",
                json!({ "type": "string", "format": "uuid" }),
                DataModelSystemFieldWritePolicy::RuntimeManaged,
            ),
            system_field(
                "created_by",
                json!({ "anyOf": [{ "type": "string", "format": "uuid" }, { "type": "null" }] }),
                DataModelSystemFieldWritePolicy::RuntimeManaged,
            ),
            system_field(
                "updated_by",
                json!({ "anyOf": [{ "type": "string", "format": "uuid" }, { "type": "null" }] }),
                DataModelSystemFieldWritePolicy::RuntimeManaged,
            ),
            system_field(
                "created_at",
                json!({ "type": "string", "format": "date-time" }),
                DataModelSystemFieldWritePolicy::DatabaseGenerated,
            ),
            system_field(
                "updated_at",
                json!({ "type": "string", "format": "date-time" }),
                DataModelSystemFieldWritePolicy::DatabaseManaged,
            ),
        ],
        operations: vec![
            operation(
                "list_records",
                DataModelOperationMethod::Get,
                "/api/runtime/models/{model_code}/list",
                "view",
                "List records",
                "List records with filter, sort, pagination, and relation expansion.",
            ),
            operation(
                "get_record",
                DataModelOperationMethod::Get,
                "/api/runtime/models/{model_code}/get/{id}",
                "view",
                "Get record",
                "Get one record by id with optional relation expansion.",
            ),
            operation(
                "create_record",
                DataModelOperationMethod::Post,
                "/api/runtime/models/{model_code}/create",
                "create",
                "Create record",
                "Create one record through the runtime model contract.",
            ),
            operation(
                "update_record",
                DataModelOperationMethod::Patch,
                "/api/runtime/models/{model_code}/update/{id}",
                "update",
                "Update record",
                "Update one record by id through the runtime model contract.",
            ),
            operation(
                "delete_record",
                DataModelOperationMethod::Delete,
                "/api/runtime/models/{model_code}/delete/{id}",
                "delete",
                "Delete record",
                "Delete one record by id through the runtime model contract.",
            ),
        ],
        summary: "General data model".to_owned(),
        description: "Core general-purpose Data Model template.".to_owned(),
    }
}

fn system_field(
    code: &str,
    value_schema: Value,
    write_policy: DataModelSystemFieldWritePolicy,
) -> DataModelTemplateSystemField {
    DataModelTemplateSystemField {
        code: code.to_owned(),
        value_schema,
        required: true,
        write_policy,
        summary: format!("{code} system field"),
        description: format!("Core-managed `{code}` field."),
    }
}

fn operation(
    code: &str,
    method: DataModelOperationMethod,
    path: &str,
    permission_action: &str,
    summary: &str,
    description: &str,
) -> DataModelTemplateOperation {
    DataModelTemplateOperation {
        code: code.to_owned(),
        method,
        path: path.to_owned(),
        input_schema: json!({ "type": "object" }),
        output_schema: json!({ "type": "object" }),
        permission_action: permission_action.to_owned(),
        handler_ref: DataModelOperationHandlerRef {
            provider: "core".to_owned(),
            code: code.to_owned(),
            version: "v1".to_owned(),
        },
        summary: summary.to_owned(),
        description: description.to_owned(),
    }
}

struct CoreResolution;

impl DataModelTemplateResolutionContract for CoreResolution {
    fn contains_capability(&self, capability: &DataModelCapabilityRequirement) -> bool {
        capability.code == GENERAL_RECORDS_READ_CAPABILITY
    }

    fn contains_operation_handler(&self, handler_ref: &DataModelOperationHandlerRef) -> bool {
        CoreGeneralOperationHandler::from_ref(handler_ref).is_some()
            || crate::ordered_tree_template::CoreOrderedTreeOperationHandler::from_ref(handler_ref)
                .is_some()
    }
}
