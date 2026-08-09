use std::collections::BTreeSet;

use plugin_framework::{
    DataModelCapabilityRequirement, DataModelOperationHandlerRef, DataModelOperationMethod,
    DataModelSourceKind, DataModelSystemFieldWritePolicy, DataModelTemplateDescriptor,
    DataModelTemplateIdentity, DataModelTemplateOperation, DataModelTemplateSource,
    DataModelTemplateSourceSelector, DataModelTemplateSystemField,
    DATA_MODEL_TEMPLATE_DESCRIPTOR_VERSION_V1,
};
use serde_json::{json, Value};

use crate::data_model_template_registry::{
    DataModelTemplateRegistry, DataModelTemplateRegistryError, DataModelTemplateResolutionContract,
};

#[derive(Default)]
struct ResolutionInventory {
    capabilities: BTreeSet<String>,
    operation_handlers: BTreeSet<DataModelOperationHandlerRef>,
}

impl DataModelTemplateResolutionContract for ResolutionInventory {
    fn contains_capability(&self, capability: &DataModelCapabilityRequirement) -> bool {
        self.capabilities.contains(&capability.code)
    }

    fn contains_operation_handler(&self, handler_ref: &DataModelOperationHandlerRef) -> bool {
        self.operation_handlers.contains(handler_ref)
    }
}

#[test]
fn ac_004_general_v1_descriptor_is_serializable_and_compiles_as_one_truth() {
    let descriptor = general_v1_descriptor();
    let serialized = serde_json::to_value(&descriptor).expect("fixture must serialize");
    let decoded: DataModelTemplateDescriptor =
        serde_json::from_value(serialized).expect("serialized descriptor must round-trip");
    assert_eq!(decoded, descriptor);

    let registry = DataModelTemplateRegistry::compile(
        [descriptor.clone()],
        &resolution_inventory_for(&descriptor),
    )
    .expect("authentic general/v1 descriptor must compile");
    let compiled = registry
        .resolve(&descriptor.identity)
        .expect("compiled identity must resolve")
        .descriptor();

    assert_eq!(compiled.identity.canonical_name(), "core/general/v1");
    assert_eq!(
        compiled
            .system_fields
            .iter()
            .map(|field| field.code.as_str())
            .collect::<Vec<_>>(),
        [
            "id",
            "scope_id",
            "created_by",
            "updated_by",
            "created_at",
            "updated_at"
        ]
    );
    assert_eq!(
        compiled
            .operations
            .iter()
            .map(|operation| operation.code.as_str())
            .collect::<Vec<_>>(),
        [
            "list_records",
            "get_record",
            "create_record",
            "update_record",
            "delete_record"
        ]
    );
    assert!(compiled.operations.iter().all(|operation| {
        operation.input_schema.is_object()
            && operation.output_schema.is_object()
            && !operation.permission_action.is_empty()
            && !operation.handler_ref.code.is_empty()
            && !operation.summary.is_empty()
            && !operation.description.is_empty()
    }));

    let compatible = registry.compatible_templates(
        &DataModelTemplateSource {
            kind: DataModelSourceKind::MainSource,
            provider: None,
        },
        ["runtime.crud"],
    );
    assert_eq!(compatible.len(), 1);
    assert_eq!(compatible[0].identity(), &descriptor.identity);
}

#[test]
fn ac_004_registry_rejects_invalid_or_incomplete_operation_contracts() {
    let base = general_v1_descriptor();

    let mut missing_fields = base.clone();
    missing_fields.system_fields.clear();
    assert!(matches!(
        compile_with_descriptor_inventory(missing_fields),
        Err(DataModelTemplateRegistryError::InvalidDescriptor(_))
    ));

    let mut missing_schema = base.clone();
    missing_schema.operations[0].input_schema = Value::Null;
    assert!(matches!(
        compile_with_descriptor_inventory(missing_schema),
        Err(DataModelTemplateRegistryError::InvalidDescriptor(_))
    ));

    let mut missing_permission = base.clone();
    missing_permission.operations[0].permission_action.clear();
    assert!(matches!(
        compile_with_descriptor_inventory(missing_permission),
        Err(DataModelTemplateRegistryError::InvalidDescriptor(_))
    ));

    let mut missing_handler = base.clone();
    missing_handler.operations[0].handler_ref.code.clear();
    assert!(matches!(
        compile_with_descriptor_inventory(missing_handler),
        Err(DataModelTemplateRegistryError::InvalidDescriptor(_))
    ));

    let mut invalid_path = base.clone();
    invalid_path.operations[0].path = "runtime/models/{model_code}/list".to_owned();
    assert!(matches!(
        compile_with_descriptor_inventory(invalid_path),
        Err(DataModelTemplateRegistryError::InvalidDescriptor(_))
    ));

    let mut duplicate_route = base;
    let mut duplicate = duplicate_route.operations[0].clone();
    duplicate.code = "list_records_again".to_owned();
    duplicate_route.operations.push(duplicate);
    assert!(matches!(
        compile_with_descriptor_inventory(duplicate_route),
        Err(DataModelTemplateRegistryError::InvalidDescriptor(_))
    ));
}

#[test]
fn ac_004_registry_rejects_duplicate_unknown_and_unresolved_contracts() {
    let descriptor = general_v1_descriptor();
    let inventory = resolution_inventory_for(&descriptor);

    assert!(matches!(
        DataModelTemplateRegistry::compile([descriptor.clone(), descriptor.clone()], &inventory),
        Err(DataModelTemplateRegistryError::DuplicateIdentity(_))
    ));

    let registry = DataModelTemplateRegistry::compile([descriptor.clone()], &inventory)
        .expect("base descriptor must compile");
    let unknown = DataModelTemplateIdentity {
        provider: "core".to_owned(),
        code: "missing".to_owned(),
        version: "v1".to_owned(),
    };
    assert!(matches!(
        registry.resolve(&unknown),
        Err(DataModelTemplateRegistryError::UnknownIdentity(identity)) if identity == unknown
    ));

    let handlers_only = ResolutionInventory {
        capabilities: BTreeSet::new(),
        operation_handlers: inventory.operation_handlers.clone(),
    };
    assert!(matches!(
        DataModelTemplateRegistry::compile([descriptor.clone()], &handlers_only),
        Err(DataModelTemplateRegistryError::UnresolvedCapability { .. })
    ));

    let capabilities_only = ResolutionInventory {
        capabilities: inventory.capabilities,
        operation_handlers: BTreeSet::new(),
    };
    assert!(matches!(
        DataModelTemplateRegistry::compile([descriptor], &capabilities_only),
        Err(DataModelTemplateRegistryError::UnresolvedOperationHandler { .. })
    ));
}

#[test]
fn ac_004_compatibility_requires_selector_and_capability_subset() {
    let mut descriptor = general_v1_descriptor();
    descriptor.source_selector = DataModelTemplateSourceSelector::ExternalProvider {
        provider: "example".to_owned(),
    };
    let registry = DataModelTemplateRegistry::compile(
        [descriptor.clone()],
        &resolution_inventory_for(&descriptor),
    )
    .expect("base descriptor must compile");

    assert!(registry
        .compatible_templates(
            &DataModelTemplateSource {
                kind: DataModelSourceKind::MainSource,
                provider: None,
            },
            [],
        )
        .is_empty());
    assert!(registry
        .compatible_templates(
            &DataModelTemplateSource {
                kind: DataModelSourceKind::ExternalSource,
                provider: Some("other".to_owned()),
            },
            ["runtime.crud"],
        )
        .is_empty());
    assert_eq!(
        registry
            .compatible_templates(
                &DataModelTemplateSource {
                    kind: DataModelSourceKind::ExternalSource,
                    provider: Some("example".to_owned()),
                },
                ["runtime.crud"],
            )
            .len(),
        1
    );
}

fn compile_with_descriptor_inventory(
    descriptor: DataModelTemplateDescriptor,
) -> Result<DataModelTemplateRegistry, DataModelTemplateRegistryError> {
    let inventory = resolution_inventory_for(&descriptor);
    DataModelTemplateRegistry::compile([descriptor], &inventory)
}

fn resolution_inventory_for(descriptor: &DataModelTemplateDescriptor) -> ResolutionInventory {
    ResolutionInventory {
        capabilities: descriptor
            .required_capabilities
            .iter()
            .map(|capability| capability.code.clone())
            .collect(),
        operation_handlers: descriptor
            .operations
            .iter()
            .map(|operation| operation.handler_ref.clone())
            .collect(),
    }
}

fn general_v1_descriptor() -> DataModelTemplateDescriptor {
    DataModelTemplateDescriptor {
        descriptor_version: DATA_MODEL_TEMPLATE_DESCRIPTOR_VERSION_V1,
        identity: DataModelTemplateIdentity {
            provider: "core".to_owned(),
            code: "general".to_owned(),
            version: "v1".to_owned(),
        },
        source_selector: DataModelTemplateSourceSelector::Any,
        required_capabilities: vec![DataModelCapabilityRequirement {
            code: "runtime.crud".to_owned(),
        }],
        system_fields: vec![
            system_field(
                "id",
                "string",
                DataModelSystemFieldWritePolicy::RuntimeGenerated,
            ),
            system_field(
                "scope_id",
                "string",
                DataModelSystemFieldWritePolicy::RuntimeManaged,
            ),
            system_field(
                "created_by",
                "string",
                DataModelSystemFieldWritePolicy::RuntimeManaged,
            ),
            system_field(
                "updated_by",
                "string",
                DataModelSystemFieldWritePolicy::RuntimeManaged,
            ),
            system_field(
                "created_at",
                "string",
                DataModelSystemFieldWritePolicy::DatabaseGenerated,
            ),
            system_field(
                "updated_at",
                "string",
                DataModelSystemFieldWritePolicy::DatabaseManaged,
            ),
        ],
        operations: vec![
            operation(
                "list_records",
                DataModelOperationMethod::Get,
                "/api/runtime/models/{model_code}/list",
                "view",
            ),
            operation(
                "get_record",
                DataModelOperationMethod::Get,
                "/api/runtime/models/{model_code}/get/{id}",
                "view",
            ),
            operation(
                "create_record",
                DataModelOperationMethod::Post,
                "/api/runtime/models/{model_code}/create",
                "create",
            ),
            operation(
                "update_record",
                DataModelOperationMethod::Patch,
                "/api/runtime/models/{model_code}/update/{id}",
                "update",
            ),
            operation(
                "delete_record",
                DataModelOperationMethod::Delete,
                "/api/runtime/models/{model_code}/delete/{id}",
                "delete",
            ),
        ],
        summary: "General data model".to_owned(),
        description: "Core general data model template.".to_owned(),
    }
}

fn system_field(
    code: &str,
    schema_type: &str,
    write_policy: DataModelSystemFieldWritePolicy,
) -> DataModelTemplateSystemField {
    DataModelTemplateSystemField {
        code: code.to_owned(),
        value_schema: json!({ "type": schema_type }),
        required: true,
        write_policy,
        summary: format!("{code} system field"),
        description: format!("Canonical {code} system field."),
    }
}

fn operation(
    code: &str,
    method: DataModelOperationMethod,
    path: &str,
    permission_action: &str,
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
        summary: format!("{code} operation"),
        description: format!("Canonical {code} runtime operation."),
    }
}
