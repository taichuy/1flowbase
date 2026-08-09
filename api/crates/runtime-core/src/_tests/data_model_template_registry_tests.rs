use std::collections::BTreeSet;

use plugin_framework::{
    DataModelCapabilityRequirement, DataModelOperationHandlerRef, DataModelSourceKind,
    DataModelTemplateDescriptor, DataModelTemplateIdentity, DataModelTemplateSource,
    DataModelTemplateSourceSelector,
};
use serde_json::Value;

use crate::{
    data_model_template_registry::{
        DataModelTemplateCatalog, DataModelTemplateRegistry, DataModelTemplateRegistryError,
        DataModelTemplateResolutionContract,
    },
    general_data_model_template::{core_data_model_template_registry, general_template_identity},
    ordered_tree_template::ordered_tree_template_identity,
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
            .map(|operation| (
                operation.code.as_str(),
                operation.method,
                operation.path.as_str(),
                operation.permission_action.as_str(),
                operation.handler_ref.canonical_name(),
            ))
            .collect::<Vec<_>>(),
        [
            (
                "list_records",
                plugin_framework::DataModelOperationMethod::Get,
                "/api/runtime/models/{model_code}/list",
                "view",
                "core/list_records/v1".to_owned(),
            ),
            (
                "get_record",
                plugin_framework::DataModelOperationMethod::Get,
                "/api/runtime/models/{model_code}/get/{id}",
                "view",
                "core/get_record/v1".to_owned(),
            ),
            (
                "create_record",
                plugin_framework::DataModelOperationMethod::Post,
                "/api/runtime/models/{model_code}/create",
                "create",
                "core/create_record/v1".to_owned(),
            ),
            (
                "update_record",
                plugin_framework::DataModelOperationMethod::Patch,
                "/api/runtime/models/{model_code}/update/{id}",
                "update",
                "core/update_record/v1".to_owned(),
            ),
            (
                "delete_record",
                plugin_framework::DataModelOperationMethod::Delete,
                "/api/runtime/models/{model_code}/delete/{id}",
                "delete",
                "core/delete_record/v1".to_owned(),
            ),
        ]
    );
    let matched = registry
        .resolve(&descriptor.identity)
        .unwrap()
        .match_operation(
            plugin_framework::DataModelOperationMethod::Patch,
            "/api/runtime/models/orders/update/018f0000-0000-7000-8000-000000000000",
        )
        .expect("production update route must match");
    assert_eq!(matched.operation.code, "update_record");
    assert_eq!(
        matched
            .path_parameters
            .get("model_code")
            .map(String::as_str),
        Some("orders")
    );
    assert_eq!(
        matched.path_parameters.get("id").map(String::as_str),
        Some("018f0000-0000-7000-8000-000000000000")
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
        ["records.read"],
    );
    assert_eq!(compatible.len(), 1);
    assert_eq!(compatible[0].identity(), &descriptor.identity);
}

// AC-008/AC-009: the compiled descriptor is the route, permission, and handler inventory.
#[test]
fn ordered_tree_v1_compiles_exactly_twelve_operations_without_regressing_other_templates() {
    let registry = core_data_model_template_registry().expect("core templates must compile");
    let ordered = registry
        .resolve(&ordered_tree_template_identity())
        .expect("ordered-tree template must resolve")
        .descriptor();

    assert_eq!(
        ordered
            .operations
            .iter()
            .map(|operation| (
                operation.code.as_str(),
                operation.method,
                operation.path.as_str(),
                operation.permission_action.as_str(),
                operation.handler_ref.canonical_name(),
            ))
            .collect::<Vec<_>>(),
        [
            (
                "list_records",
                plugin_framework::DataModelOperationMethod::Get,
                "/api/runtime/models/{model_code}/list",
                "view",
                "core/ordered_tree_list_records/v1".to_owned()
            ),
            (
                "get_record",
                plugin_framework::DataModelOperationMethod::Get,
                "/api/runtime/models/{model_code}/get/{id}",
                "view",
                "core/ordered_tree_get_record/v1".to_owned()
            ),
            (
                "create_record",
                plugin_framework::DataModelOperationMethod::Post,
                "/api/runtime/models/{model_code}/create",
                "create",
                "core/ordered_tree_create_record/v1".to_owned()
            ),
            (
                "update_record",
                plugin_framework::DataModelOperationMethod::Patch,
                "/api/runtime/models/{model_code}/update/{id}",
                "update",
                "core/ordered_tree_update_record/v1".to_owned()
            ),
            (
                "delete_record",
                plugin_framework::DataModelOperationMethod::Delete,
                "/api/runtime/models/{model_code}/delete/{id}",
                "delete",
                "core/ordered_tree_delete_record/v1".to_owned()
            ),
            (
                "tree_roots",
                plugin_framework::DataModelOperationMethod::Get,
                "/api/runtime/models/{model_code}/tree/roots",
                "view",
                "core/ordered_tree_list_roots/v1".to_owned()
            ),
            (
                "tree_children",
                plugin_framework::DataModelOperationMethod::Get,
                "/api/runtime/models/{model_code}/tree/children/{id}",
                "view",
                "core/ordered_tree_list_children/v1".to_owned()
            ),
            (
                "tree_ancestors",
                plugin_framework::DataModelOperationMethod::Get,
                "/api/runtime/models/{model_code}/tree/ancestors/{id}",
                "view",
                "core/ordered_tree_list_ancestors/v1".to_owned()
            ),
            (
                "tree_descendants",
                plugin_framework::DataModelOperationMethod::Get,
                "/api/runtime/models/{model_code}/tree/descendants/{id}",
                "view",
                "core/ordered_tree_list_descendants/v1".to_owned()
            ),
            (
                "tree_search",
                plugin_framework::DataModelOperationMethod::Get,
                "/api/runtime/models/{model_code}/tree/search",
                "view",
                "core/ordered_tree_search/v1".to_owned()
            ),
            (
                "tree_move",
                plugin_framework::DataModelOperationMethod::Post,
                "/api/runtime/models/{model_code}/tree/move/{id}",
                "update",
                "core/ordered_tree_move/v1".to_owned()
            ),
            (
                "tree_delete_subtree",
                plugin_framework::DataModelOperationMethod::Post,
                "/api/runtime/models/{model_code}/tree/delete-subtree/{id}",
                "delete",
                "core/ordered_tree_delete_subtree/v1".to_owned()
            ),
        ]
    );
    assert_eq!(
        registry
            .resolve(&general_template_identity())
            .expect("general template must remain registered")
            .descriptor()
            .operations
            .len(),
        5
    );

    let catalog = DataModelTemplateCatalog::core();
    let external = external_descriptor("fixture_provider", "fixture_records", "v1");
    catalog
        .replace_provider(
            "fixture-installation",
            "fixture_provider",
            vec![external.clone()],
        )
        .expect("external descriptor must remain installable");
    assert_eq!(
        catalog
            .resolve(&external.identity)
            .expect("external template must resolve")
            .descriptor(),
        &external
    );
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
            ["records.read"],
        )
        .is_empty());
    assert_eq!(
        registry
            .compatible_templates(
                &DataModelTemplateSource {
                    kind: DataModelSourceKind::ExternalSource,
                    provider: Some("example".to_owned()),
                },
                ["records.read"],
            )
            .len(),
        1
    );
}

#[test]
fn ac_004_external_provider_templates_replace_atomically_and_filter_by_capability_subset() {
    let catalog = DataModelTemplateCatalog::core();
    let descriptor = external_descriptor("acme_source", "contacts", "v1");
    catalog
        .replace_provider("acme_source@1.0.0", "acme_source", vec![descriptor.clone()])
        .unwrap();

    let source = DataModelTemplateSource {
        kind: DataModelSourceKind::ExternalSource,
        provider: Some("acme_source".to_owned()),
    };
    assert!(catalog.compatible_templates(&source, []).is_empty());
    let compatible_identities = catalog
        .compatible_templates(&source, ["records.read"])
        .into_iter()
        .map(|template| template.identity().clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        compatible_identities,
        BTreeSet::from([general_template_identity(), descriptor.identity.clone()]),
        "external resources retain core/general/v1 alongside provider templates"
    );

    let replacement = external_descriptor("acme_source", "contacts", "v2");
    catalog
        .replace_provider(
            "acme_source@1.0.0",
            "acme_source",
            vec![replacement.clone()],
        )
        .unwrap();
    assert!(catalog.resolve(&descriptor.identity).is_err());
    assert!(catalog.resolve(&replacement.identity).is_ok());
    let compatible_identities = catalog
        .compatible_templates(&source, ["records.read"])
        .into_iter()
        .map(|template| template.identity().clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        compatible_identities,
        BTreeSet::from([general_template_identity(), replacement.identity]),
        "provider replacement must not remove core/general/v1 from external compatibility"
    );
}

#[test]
fn ac_004_external_provider_catalog_rejects_duplicate_identity_without_mutating_registry() {
    let catalog = DataModelTemplateCatalog::core();
    let descriptor = external_descriptor("acme_source", "contacts", "v1");
    catalog
        .replace_provider("installation-1", "acme_source", vec![descriptor.clone()])
        .unwrap();

    let error = catalog
        .replace_provider("installation-2", "acme_source", vec![descriptor.clone()])
        .unwrap_err();
    assert!(matches!(
        error,
        DataModelTemplateRegistryError::DuplicateIdentity(_)
    ));
    assert!(catalog.resolve(&descriptor.identity).is_ok());
}

fn external_descriptor(provider: &str, code: &str, version: &str) -> DataModelTemplateDescriptor {
    let mut descriptor = general_v1_descriptor();
    descriptor.identity = DataModelTemplateIdentity {
        provider: provider.to_owned(),
        code: code.to_owned(),
        version: version.to_owned(),
    };
    descriptor.source_selector = DataModelTemplateSourceSelector::ExternalProvider {
        provider: provider.to_owned(),
    };
    for operation in &mut descriptor.operations {
        operation.handler_ref.provider = provider.to_owned();
    }
    descriptor
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
    core_data_model_template_registry()
        .expect("production registry must compile")
        .resolve(&general_template_identity())
        .expect("production general template must resolve")
        .descriptor()
        .clone()
}
