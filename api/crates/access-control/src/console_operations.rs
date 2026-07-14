use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt::{Display, Formatter},
};

use serde::{Deserialize, Serialize};

use crate::{
    SettingsFeatureLifecycle, SettingsFeatureOwner, SettingsFeatureOwnerKind,
    SettingsFeatureRegistry,
};

pub const CONSOLE_OPERATION_INVENTORY_SCHEMA_VERSION: &str =
    "1flowbase.console-operation-inventory/v1";
pub const APPLICATIONS_RESOURCE_CODE: &str = "applications";
pub const APPLICATIONS_CREATE_OPERATION_ID: &str = "applications.create";
pub const APPLICATIONS_VIEW_OPERATION_ID: &str = "applications.view";
pub const APPLICATIONS_UPDATE_OPERATION_ID: &str = "applications.update";
pub const APPLICATIONS_DELETE_OPERATION_ID: &str = "applications.delete";
pub const APPLICATIONS_PUBLISH_OPERATION_ID: &str = "applications.publish";
pub const APPLICATIONS_API_SET_ENABLED_OPERATION_ID: &str = "applications.api.set_enabled";
pub const APPLICATIONS_ORCHESTRATION_TEMPLATE_EXPORT_OPERATION_ID: &str =
    "applications.orchestration.template.export";
pub const APPLICATIONS_ORCHESTRATION_TEMPLATE_IMPORT_OPERATION_ID: &str =
    "applications.orchestration.template.import";
pub const APPLICATIONS_ORCHESTRATION_VERSION_RESTORE_OPERATION_ID: &str =
    "applications.orchestration.version.restore";
pub const APPLICATIONS_RUN_OPERATION_ID: &str = "applications.run";
pub const APPLICATIONS_LOGS_EXPORT_OPERATION_ID: &str = "applications.logs.export";
pub const APPLICATIONS_LOGS_IMPORT_OPERATION_ID: &str = "applications.logs.import";
pub const APPLICATIONS_CREATE_ACTION_CODE: &str = "create";
pub const APPLICATIONS_VIEW_ACTION_CODE: &str = "view";
pub const APPLICATIONS_UPDATE_ACTION_CODE: &str = "update";
pub const APPLICATIONS_DELETE_ACTION_CODE: &str = "delete";
pub const DATA_SOURCE_INSTANCES_RESOURCE_CODE: &str = "data_source_instances";
pub const DATA_SOURCES_LIST_OPERATION_ID: &str = "data_sources.list";
pub const DATA_SOURCES_CREATE_OPERATION_ID: &str = "data_sources.create";
pub const DATA_SOURCES_DEFAULTS_UPDATE_OPERATION_ID: &str = "data_sources.defaults.update";
pub const DATA_SOURCES_VALIDATE_OPERATION_ID: &str = "data_sources.validate";
pub const DATA_SOURCES_VIEW_OPERATION_ID: &str = "data_sources.view";
pub const DATA_SOURCES_DISCOVER_OPERATION_ID: &str = "data_sources.discover";
pub const DATA_SOURCES_PREVIEW_OPERATION_ID: &str = "data_sources.preview";
pub const DATA_SOURCES_MAP_TO_MODEL_OPERATION_ID: &str = "data_sources.map_to_model";
pub const DATA_SOURCES_SECRET_ROTATE_OPERATION_ID: &str = "data_sources.secret.rotate";
pub const DATA_SOURCES_VIEW_ACTION_CODE: &str = "view";
pub const MODEL_DEFINITIONS_LIST_OPERATION_ID: &str = "model_definitions.list";
pub const MODEL_DEFINITIONS_CREATE_OPERATION_ID: &str = "model_definitions.create";
pub const MODEL_DEFINITIONS_UPDATE_OPERATION_ID: &str = "model_definitions.update";
pub const MODEL_DEFINITIONS_DELETE_OPERATION_ID: &str = "model_definitions.delete";
pub const MODEL_DEFINITIONS_ADVISOR_VIEW_OPERATION_ID: &str = "model_definitions.advisor.view";
pub const MODEL_DEFINITIONS_OPENAPI_VIEW_OPERATION_ID: &str = "model_definitions.openapi.view";
pub const MODEL_FIELDS_CREATE_OPERATION_ID: &str = "model_fields.create";
pub const MODEL_FIELDS_UPDATE_OPERATION_ID: &str = "model_fields.update";
pub const MODEL_FIELDS_DELETE_OPERATION_ID: &str = "model_fields.delete";
pub const MODEL_SCOPE_GRANTS_LIST_OPERATION_ID: &str = "model_scope_grants.list";
pub const MODEL_SCOPE_GRANTS_CREATE_OPERATION_ID: &str = "model_scope_grants.create";
pub const MODEL_SCOPE_GRANTS_UPDATE_OPERATION_ID: &str = "model_scope_grants.update";
pub const FILES_UPLOAD_OPERATION_ID: &str = "files.upload";
pub const FILES_CONTENT_DOWNLOAD_OPERATION_ID: &str = "files.content.download";
pub const FILE_STORAGES_LIST_OPERATION_ID: &str = "file_storages.list";
pub const FILE_STORAGES_CREATE_OPERATION_ID: &str = "file_storages.create";
pub const FILE_STORAGES_UPDATE_OPERATION_ID: &str = "file_storages.update";
pub const FILE_STORAGES_DELETE_OPERATION_ID: &str = "file_storages.delete";
pub const FILE_TABLES_LIST_OPERATION_ID: &str = "file_tables.list";
pub const FILE_TABLES_CREATE_OPERATION_ID: &str = "file_tables.create";
pub const FILE_TABLES_STORAGE_BIND_OPERATION_ID: &str = "file_tables.storage.bind";
pub const FILE_TABLES_DELETE_OPERATION_ID: &str = "file_tables.delete";

pub type ConsoleOperationOwner = SettingsFeatureOwner;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsolePolicyGroup {
    SettingsFeature(String),
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsoleLocaleText {
    pub reference: String,
    pub en_us: String,
    pub zh_hans: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsoleOtherPolicyGroupDisplay {
    pub group_id: String,
    pub label_ref: String,
    pub description_ref: String,
}

/// A locale contribution is compiled at boot with the operation registry. HostExtension loading
/// will provide contributions through this same value object once its runtime merge exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsoleLocaleCatalogContribution {
    pub owner: ConsoleOperationOwner,
    pub lifecycle: SettingsFeatureLifecycle,
    pub texts: Vec<ConsoleLocaleText>,
    pub policy_groups: Vec<ConsoleOtherPolicyGroupDisplay>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsoleLocalizedDisplay {
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsoleLocalizedOption {
    pub value: String,
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsoleLocaleCatalog {
    texts: BTreeMap<String, ConsoleLocaleText>,
    policy_group_displays: BTreeMap<(String, String), ConsoleCatalogTextReferences>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConsoleCatalogTextReferences {
    label_ref: String,
    description_ref: String,
}

#[derive(Debug, Clone, Copy)]
struct ConsoleCatalogOptionSpec {
    value: &'static str,
    label_ref: &'static str,
    description_ref: &'static str,
}

const CONSOLE_POLICY_GROUP_MODE_OPTIONS: &[ConsoleCatalogOptionSpec] = &[
    ConsoleCatalogOptionSpec {
        value: "disabled",
        label_ref: "console.policy.group_modes.disabled.label",
        description_ref: "console.policy.group_modes.disabled.description",
    },
    ConsoleCatalogOptionSpec {
        value: "full",
        label_ref: "console.policy.group_modes.full.label",
        description_ref: "console.policy.group_modes.full.description",
    },
    ConsoleCatalogOptionSpec {
        value: "custom",
        label_ref: "console.policy.group_modes.custom.label",
        description_ref: "console.policy.group_modes.custom.description",
    },
];

const CONSOLE_POLICY_ROW_SCOPE_OPTIONS: &[ConsoleCatalogOptionSpec] = &[
    ConsoleCatalogOptionSpec {
        value: "disabled",
        label_ref: "console.policy.row_scopes.disabled.label",
        description_ref: "console.policy.row_scopes.disabled.description",
    },
    ConsoleCatalogOptionSpec {
        value: "own",
        label_ref: "console.policy.row_scopes.own.label",
        description_ref: "console.policy.row_scopes.own.description",
    },
    ConsoleCatalogOptionSpec {
        value: "scope_all",
        label_ref: "console.policy.row_scopes.scope_all.label",
        description_ref: "console.policy.row_scopes.scope_all.description",
    },
];

impl ConsoleLocaleCatalog {
    pub fn text(&self, locale: &str, reference: &str) -> Option<&str> {
        self.texts.get(reference).and_then(|text| match locale {
            "en_US" => Some(text.en_us.as_str()),
            "zh_Hans" => Some(text.zh_hans.as_str()),
            _ => None,
        })
    }

    pub fn policy_group_display(
        &self,
        policy_group: &ConsolePolicyGroup,
        locale: &str,
    ) -> Result<ConsoleLocalizedDisplay, ConsoleOperationRegistryError> {
        let key = console_policy_group_key(policy_group);
        let references = self.policy_group_displays.get(&key).ok_or_else(|| {
            ConsoleOperationRegistryError::new(format!(
                "compiled locale catalog is missing policy group display {}.{}",
                key.0, key.1
            ))
        })?;
        Ok(ConsoleLocalizedDisplay {
            label: self.required_text(locale, &references.label_ref)?,
            description: self.required_text(locale, &references.description_ref)?,
        })
    }

    pub fn group_mode_options(
        &self,
        locale: &str,
    ) -> Result<Vec<ConsoleLocalizedOption>, ConsoleOperationRegistryError> {
        self.localized_options(CONSOLE_POLICY_GROUP_MODE_OPTIONS, locale)
    }

    pub fn row_scope_options(
        &self,
        locale: &str,
    ) -> Result<Vec<ConsoleLocalizedOption>, ConsoleOperationRegistryError> {
        self.localized_options(CONSOLE_POLICY_ROW_SCOPE_OPTIONS, locale)
    }

    fn localized_options(
        &self,
        options: &[ConsoleCatalogOptionSpec],
        locale: &str,
    ) -> Result<Vec<ConsoleLocalizedOption>, ConsoleOperationRegistryError> {
        options
            .iter()
            .map(|option| {
                Ok(ConsoleLocalizedOption {
                    value: option.value.to_string(),
                    label: self.required_text(locale, option.label_ref)?,
                    description: self.required_text(locale, option.description_ref)?,
                })
            })
            .collect()
    }

    fn required_text(
        &self,
        locale: &str,
        reference: &str,
    ) -> Result<String, ConsoleOperationRegistryError> {
        self.text(locale, reference)
            .map(str::to_string)
            .ok_or_else(|| {
                ConsoleOperationRegistryError::new(format!(
                    "compiled locale catalog is missing {locale} text for {reference}"
                ))
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConsoleAuthorization {
    Authenticated,
    Simple,
    ResourceAction {
        resource_code: String,
        action_code: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsoleRouteBinding {
    pub method: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "operation_id", rename_all = "snake_case")]
pub enum ConsoleRouteOwnership {
    Authenticated,
    ConsoleOperation(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsoleRouteAssemblyBinding {
    pub route: ConsoleRouteBinding,
    pub ownership: ConsoleRouteOwnership,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsoleOperationRegistration {
    pub operation_id: String,
    pub owner: ConsoleOperationOwner,
    pub lifecycle: SettingsFeatureLifecycle,
    pub policy_group: ConsolePolicyGroup,
    pub label_ref: String,
    pub description_ref: Option<String>,
    pub order: i32,
    pub routes: Vec<ConsoleRouteBinding>,
    pub authorization: ConsoleAuthorization,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceAccessScopeKind {
    System,
    Workspace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceAccessAction {
    pub action_code: String,
    pub label_ref: String,
    pub description_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceAccessRegistration {
    pub resource_code: String,
    pub owner: ConsoleOperationOwner,
    pub lifecycle: SettingsFeatureLifecycle,
    pub scope_kind: ResourceAccessScopeKind,
    pub identity_field: String,
    pub scope_field: Option<String>,
    pub owner_field: Option<String>,
    pub label_ref: String,
    pub description_ref: Option<String>,
    pub actions: Vec<ResourceAccessAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsoleOperationInventoryEntry {
    pub operation_id: String,
    pub owner: ConsoleOperationOwner,
    pub lifecycle: SettingsFeatureLifecycle,
    pub policy_group: ConsolePolicyGroup,
    pub label_ref: String,
    pub description_ref: Option<String>,
    pub order: i32,
    pub routes: Vec<ConsoleRouteBinding>,
    pub authorization: ConsoleAuthorization,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsoleOperationCompiledInventory {
    pub schema_version: &'static str,
    pub operations: Vec<ConsoleOperationInventoryEntry>,
    pub resources: Vec<ResourceAccessRegistration>,
    #[serde(skip)]
    pub locale_catalog: Option<ConsoleLocaleCatalog>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsolePolicyGroupChange {
    pub operation_id: String,
    pub before: ConsolePolicyGroup,
    pub after: ConsolePolicyGroup,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsoleOperationRegistryDiff {
    pub schema_version: &'static str,
    pub added_operations: Vec<String>,
    pub removed_operations: Vec<String>,
    pub changed_operations: Vec<String>,
    pub policy_group_changes: Vec<ConsolePolicyGroupChange>,
    pub added_routes: Vec<String>,
    pub removed_routes: Vec<String>,
    pub added_resources: Vec<String>,
    pub removed_resources: Vec<String>,
    pub changed_resources: Vec<String>,
}

#[derive(Debug)]
pub struct ConsoleOperationRegistry {
    inventory: ConsoleOperationCompiledInventory,
    route_owners: BTreeMap<(String, String), String>,
}

impl ConsoleOperationRegistry {
    pub fn compile(
        settings_features: &SettingsFeatureRegistry,
        registrations: impl IntoIterator<Item = ConsoleOperationRegistration>,
        resources: impl IntoIterator<Item = ResourceAccessRegistration>,
    ) -> Result<Self, ConsoleOperationRegistryError> {
        Self::compile_internal(settings_features, registrations, resources, None)
    }

    pub fn compile_with_locale_catalog(
        settings_features: &SettingsFeatureRegistry,
        registrations: impl IntoIterator<Item = ConsoleOperationRegistration>,
        resources: impl IntoIterator<Item = ResourceAccessRegistration>,
        locale_contributions: impl IntoIterator<Item = ConsoleLocaleCatalogContribution>,
    ) -> Result<Self, ConsoleOperationRegistryError> {
        Self::compile_internal(
            settings_features,
            registrations,
            resources,
            Some(locale_contributions.into_iter().collect()),
        )
    }

    fn compile_internal(
        settings_features: &SettingsFeatureRegistry,
        registrations: impl IntoIterator<Item = ConsoleOperationRegistration>,
        resources: impl IntoIterator<Item = ResourceAccessRegistration>,
        locale_contributions: Option<Vec<ConsoleLocaleCatalogContribution>>,
    ) -> Result<Self, ConsoleOperationRegistryError> {
        let settings_feature_ids = settings_features
            .inventory()
            .features
            .iter()
            .map(|feature| feature.feature_id.as_str())
            .collect::<BTreeSet<_>>();
        let mut operations = BTreeMap::new();
        let mut route_owners = BTreeMap::new();
        let mut compiled_resources = BTreeMap::new();

        for mut resource in resources {
            validate_resource(&resource)?;
            if compiled_resources.contains_key(&resource.resource_code) {
                return Err(ConsoleOperationRegistryError::new(format!(
                    "duplicate resource access registration {}",
                    resource.resource_code
                )));
            }
            resource
                .actions
                .sort_by(|left, right| left.action_code.cmp(&right.action_code));
            compiled_resources.insert(resource.resource_code.clone(), resource);
        }

        let registrations = registrations.into_iter().collect::<Vec<_>>();
        for registration in &registrations {
            validate_operation(registration, &settings_feature_ids, &compiled_resources)?;
        }
        let explicit_operation_routes = registrations
            .iter()
            .flat_map(|registration| registration.routes.iter())
            .map(|route| (route.method.to_ascii_uppercase(), route_shape(&route.path)))
            .collect::<BTreeSet<_>>();

        // Settings API ownership stays in the #1256 registry. This projection gives the
        // unified inventory an operation identity without creating another route table. Stable
        // operation registrations replace their legacy feature-level route ownership atomically.
        for feature in &settings_features.inventory().features {
            let routes = feature
                .api_routes
                .iter()
                .filter(|route| {
                    !explicit_operation_routes
                        .contains(&(route.method.to_ascii_uppercase(), route_shape(&route.path)))
                })
                .map(|route| ConsoleRouteBinding {
                    method: route.method.clone(),
                    path: route.path.clone(),
                })
                .collect::<Vec<_>>();
            if routes.is_empty() {
                continue;
            }
            let operation = ConsoleOperationRegistration {
                operation_id: settings_feature_operation_id(&feature.feature_id),
                owner: feature.owner.clone(),
                lifecycle: feature.lifecycle,
                policy_group: ConsolePolicyGroup::SettingsFeature(feature.feature_id.clone()),
                label_ref: feature.console_surface.label_key.clone(),
                description_ref: Some(feature.console_surface.description_key.clone()),
                order: feature.console_surface.order,
                routes,
                authorization: ConsoleAuthorization::Simple,
            };
            for route in &operation.routes {
                validate_route(route)?;
            }
            compile_operation(operation, &mut operations, &mut route_owners)?;
        }

        for registration in registrations {
            compile_operation(registration, &mut operations, &mut route_owners)?;
        }

        let mut inventory = ConsoleOperationCompiledInventory {
            schema_version: CONSOLE_OPERATION_INVENTORY_SCHEMA_VERSION,
            operations: operations
                .into_values()
                .map(ConsoleOperationInventoryEntry::from)
                .collect(),
            resources: compiled_resources.into_values().collect(),
            locale_catalog: None,
        };
        if let Some(locale_contributions) = locale_contributions {
            inventory.locale_catalog = Some(compile_console_locale_catalog(
                settings_features,
                &inventory,
                locale_contributions,
            )?);
        }

        Ok(Self {
            inventory,
            route_owners,
        })
    }

    pub fn inventory(&self) -> &ConsoleOperationCompiledInventory {
        &self.inventory
    }

    pub fn access_for_console_route(
        &self,
        method: &str,
        path: &str,
    ) -> Result<ConsoleRouteAccess<'_>, ConsoleOperationRegistryError> {
        if !path.starts_with("/api/console/") && path != "/api/console" {
            return Err(ConsoleOperationRegistryError::new(format!(
                "route is outside the console contract: {path}"
            )));
        }
        let method = method.to_ascii_uppercase();
        let operation_id = self
            .route_owners
            .iter()
            .filter(|((registered_method, registered_path), _)| {
                registered_method == &method && route_matches(registered_path, path)
            })
            .max_by_key(|((_, registered_path), _)| route_literal_specificity(registered_path))
            .map(|(_, operation_id)| operation_id)
            .ok_or_else(|| {
                ConsoleOperationRegistryError::new(format!(
                    "unregistered console route {method} {path}"
                ))
            })?;
        let operation = self
            .inventory
            .operations
            .binary_search_by(|operation| operation.operation_id.cmp(operation_id))
            .ok()
            .and_then(|index| self.inventory.operations.get(index))
            .ok_or_else(|| {
                ConsoleOperationRegistryError::new(format!(
                    "compiled operation {operation_id} is missing from inventory"
                ))
            })?;

        Ok(ConsoleRouteAccess {
            operation_id: &operation.operation_id,
            policy_group: &operation.policy_group,
            authorization: &operation.authorization,
            resource_access: match &operation.authorization {
                ConsoleAuthorization::ResourceAction { resource_code, .. } => self
                    .inventory
                    .resources
                    .binary_search_by(|resource| resource.resource_code.cmp(resource_code))
                    .ok()
                    .and_then(|index| self.inventory.resources.get(index)),
                ConsoleAuthorization::Authenticated | ConsoleAuthorization::Simple => None,
            },
        })
    }

    pub fn validate_console_route_coverage(
        &self,
        routes: impl IntoIterator<Item = ConsoleRouteAssemblyBinding>,
    ) -> Result<(), ConsoleOperationRegistryError> {
        let mut missing = Vec::new();
        let mut assembled_owners = BTreeMap::new();
        for binding in routes {
            validate_route(&binding.route)?;
            let method = binding.route.method.to_ascii_uppercase();
            let shape = route_shape(&binding.route.path);
            let key = (method.clone(), shape);
            if let Some(existing) = assembled_owners.insert(key.clone(), binding.ownership.clone())
            {
                return Err(ConsoleOperationRegistryError::new(format!(
                    "duplicate assembled console route ownership {method} {} between {} and {}",
                    binding.route.path,
                    ownership_name(&existing),
                    ownership_name(&binding.ownership),
                )));
            }

            let Some(operation_id) = self.route_owners.get(&key) else {
                missing.push(format!("{method} {}", binding.route.path));
                continue;
            };
            let operation = self
                .inventory
                .operations
                .binary_search_by(|operation| operation.operation_id.cmp(operation_id))
                .ok()
                .and_then(|index| self.inventory.operations.get(index))
                .ok_or_else(|| {
                    ConsoleOperationRegistryError::new(format!(
                        "compiled operation {operation_id} is missing from inventory"
                    ))
                })?;
            let ownership_matches = match &binding.ownership {
                ConsoleRouteOwnership::Authenticated => {
                    operation.authorization == ConsoleAuthorization::Authenticated
                }
                ConsoleRouteOwnership::ConsoleOperation(expected_operation_id) => {
                    operation.operation_id == *expected_operation_id
                        && operation.authorization != ConsoleAuthorization::Authenticated
                }
            };
            if !ownership_matches {
                return Err(ConsoleOperationRegistryError::new(format!(
                    "assembled console route ownership mismatch {method} {}: expected {}, compiled {}",
                    binding.route.path,
                    ownership_name(&binding.ownership),
                    operation.operation_id,
                )));
            }
        }
        if missing.is_empty() {
            let mut unmounted = self
                .inventory
                .operations
                .iter()
                .flat_map(|operation| operation.routes.iter())
                .filter_map(|route| {
                    let key = (route.method.to_ascii_uppercase(), route_shape(&route.path));
                    (!assembled_owners.contains_key(&key))
                        .then(|| format!("{} {}", route.method, route.path))
                })
                .collect::<Vec<_>>();
            if unmounted.is_empty() {
                Ok(())
            } else {
                unmounted.sort();
                Err(ConsoleOperationRegistryError::new(format!(
                    "compiled console route ownership is not mounted: {}",
                    unmounted.join(", ")
                )))
            }
        } else {
            missing.sort();
            Err(ConsoleOperationRegistryError::new(format!(
                "console route coverage is missing compiled ownership: {}",
                missing.join(", ")
            )))
        }
    }

    pub fn diff(&self, baseline: &Self) -> ConsoleOperationRegistryDiff {
        let current_operations = operation_map(&self.inventory.operations);
        let baseline_operations = operation_map(&baseline.inventory.operations);
        let current_ids = current_operations.keys().copied().collect::<BTreeSet<_>>();
        let baseline_ids = baseline_operations.keys().copied().collect::<BTreeSet<_>>();
        let added_operations = current_ids
            .difference(&baseline_ids)
            .map(|id| (*id).to_string())
            .collect();
        let removed_operations = baseline_ids
            .difference(&current_ids)
            .map(|id| (*id).to_string())
            .collect();
        let mut changed_operations = Vec::new();
        let mut policy_group_changes = Vec::new();

        for operation_id in current_ids.intersection(&baseline_ids) {
            let current = current_operations[operation_id];
            let before = baseline_operations[operation_id];
            if current.policy_group != before.policy_group {
                policy_group_changes.push(ConsolePolicyGroupChange {
                    operation_id: (*operation_id).to_string(),
                    before: before.policy_group.clone(),
                    after: current.policy_group.clone(),
                });
            }
            if operation_contract_without_group(current) != operation_contract_without_group(before)
            {
                changed_operations.push((*operation_id).to_string());
            }
        }

        let current_routes = route_inventory(&self.inventory.operations);
        let baseline_routes = route_inventory(&baseline.inventory.operations);
        let current_resources = resource_map(&self.inventory.resources);
        let baseline_resources = resource_map(&baseline.inventory.resources);
        let current_resource_ids = current_resources.keys().copied().collect::<BTreeSet<_>>();
        let baseline_resource_ids = baseline_resources.keys().copied().collect::<BTreeSet<_>>();
        let changed_resources = current_resource_ids
            .intersection(&baseline_resource_ids)
            .filter(|code| current_resources[*code] != baseline_resources[*code])
            .map(|code| (*code).to_string())
            .collect();

        ConsoleOperationRegistryDiff {
            schema_version: CONSOLE_OPERATION_INVENTORY_SCHEMA_VERSION,
            added_operations,
            removed_operations,
            changed_operations,
            policy_group_changes,
            added_routes: current_routes
                .difference(&baseline_routes)
                .cloned()
                .collect(),
            removed_routes: baseline_routes
                .difference(&current_routes)
                .cloned()
                .collect(),
            added_resources: current_resource_ids
                .difference(&baseline_resource_ids)
                .map(|code| (*code).to_string())
                .collect(),
            removed_resources: baseline_resource_ids
                .difference(&current_resource_ids)
                .map(|code| (*code).to_string())
                .collect(),
            changed_resources,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsoleRouteAccess<'a> {
    pub operation_id: &'a str,
    pub policy_group: &'a ConsolePolicyGroup,
    pub authorization: &'a ConsoleAuthorization,
    pub resource_access: Option<&'a ResourceAccessRegistration>,
}

impl From<ConsoleOperationRegistration> for ConsoleOperationInventoryEntry {
    fn from(registration: ConsoleOperationRegistration) -> Self {
        Self {
            operation_id: registration.operation_id,
            owner: registration.owner,
            lifecycle: registration.lifecycle,
            policy_group: registration.policy_group,
            label_ref: registration.label_ref,
            description_ref: registration.description_ref,
            order: registration.order,
            routes: registration.routes,
            authorization: registration.authorization,
        }
    }
}

fn compile_operation(
    mut registration: ConsoleOperationRegistration,
    operations: &mut BTreeMap<String, ConsoleOperationRegistration>,
    route_owners: &mut BTreeMap<(String, String), String>,
) -> Result<(), ConsoleOperationRegistryError> {
    if operations.contains_key(&registration.operation_id) {
        return Err(ConsoleOperationRegistryError::new(format!(
            "duplicate operation_id {}",
            registration.operation_id
        )));
    }
    registration.routes = registration
        .routes
        .into_iter()
        .map(normalize_route)
        .collect();
    registration.routes.sort();
    for route in &registration.routes {
        let key = (route.method.clone(), route_shape(&route.path));
        if let Some(existing) = route_owners.iter().find_map(|((method, shape), owner)| {
            (method == &route.method && route_templates_are_ambiguous(shape, &key.1))
                .then_some(owner)
        }) {
            return Err(ConsoleOperationRegistryError::new(format!(
                "duplicate console route ownership {} {} between {existing} and {}",
                route.method, route.path, registration.operation_id
            )));
        }
        route_owners.insert(key, registration.operation_id.clone());
    }
    operations.insert(registration.operation_id.clone(), registration);
    Ok(())
}

fn compile_console_locale_catalog(
    settings_features: &SettingsFeatureRegistry,
    inventory: &ConsoleOperationCompiledInventory,
    contributions: Vec<ConsoleLocaleCatalogContribution>,
) -> Result<ConsoleLocaleCatalog, ConsoleOperationRegistryError> {
    let mut texts = BTreeMap::new();
    let mut declared_other_groups = BTreeMap::new();

    for contribution in contributions {
        validate_owner(&contribution.owner, "locale contribution owner")?;
        if contribution.lifecycle == SettingsFeatureLifecycle::Inactive {
            return Err(ConsoleOperationRegistryError::new(format!(
                "inactive locale contribution {} cannot provide catalog text",
                contribution.owner.owner_id
            )));
        }
        for text in contribution.texts {
            validate_non_empty(&text.reference, "locale reference")?;
            if text.en_us.trim().is_empty() {
                return Err(ConsoleOperationRegistryError::new(format!(
                    "empty locale text en_US for {}",
                    text.reference
                )));
            }
            if text.zh_hans.trim().is_empty() {
                return Err(ConsoleOperationRegistryError::new(format!(
                    "empty locale text zh_Hans for {}",
                    text.reference
                )));
            }
            if texts.contains_key(&text.reference) {
                return Err(ConsoleOperationRegistryError::new(format!(
                    "duplicate locale reference {}",
                    text.reference
                )));
            }
            texts.insert(text.reference.clone(), text);
        }
        for group in contribution.policy_groups {
            validate_non_empty(&group.group_id, "Other policy group display group_id")?;
            validate_non_empty(&group.label_ref, "Other policy group display label_ref")?;
            validate_non_empty(
                &group.description_ref,
                "Other policy group display description_ref",
            )?;
            let key = ("other".to_string(), group.group_id.clone());
            let display = ConsoleCatalogTextReferences {
                label_ref: group.label_ref,
                description_ref: group.description_ref,
            };
            if declared_other_groups
                .insert(key.clone(), (contribution.owner.clone(), display))
                .is_some()
            {
                return Err(ConsoleOperationRegistryError::new(format!(
                    "duplicate Other policy group display {}.{}",
                    key.0, key.1
                )));
            }
        }
    }

    let mut required_references = BTreeSet::new();
    let mut policy_group_displays = BTreeMap::new();
    for feature in &settings_features.inventory().features {
        let key = ("settings_feature".to_string(), feature.feature_id.clone());
        let display = ConsoleCatalogTextReferences {
            label_ref: feature.console_surface.label_key.clone(),
            description_ref: feature.console_surface.description_key.clone(),
        };
        required_references.insert(display.label_ref.clone());
        required_references.insert(display.description_ref.clone());
        policy_group_displays.insert(key, display);
    }

    let mut referenced_other_groups = BTreeSet::new();
    for operation in &inventory.operations {
        required_references.insert(operation.label_ref.clone());
        let description_ref = operation.description_ref.as_ref().ok_or_else(|| {
            ConsoleOperationRegistryError::new(format!(
                "operation {} must provide a locale description reference",
                operation.operation_id
            ))
        })?;
        required_references.insert(description_ref.clone());

        if let ConsolePolicyGroup::Other(group_id) = &operation.policy_group {
            let key = ("other".to_string(), group_id.clone());
            let (owner, display) = declared_other_groups.get(&key).ok_or_else(|| {
                ConsoleOperationRegistryError::new(format!(
                    "missing Other policy group display {}.{}",
                    key.0, key.1
                ))
            })?;
            if owner != &operation.owner {
                return Err(ConsoleOperationRegistryError::new(format!(
                    "Other policy group display {}.{} must use operation owner {}",
                    key.0, key.1, operation.owner.owner_id
                )));
            }
            required_references.insert(display.label_ref.clone());
            required_references.insert(display.description_ref.clone());
            policy_group_displays.insert(key.clone(), display.clone());
            referenced_other_groups.insert(key);
        }
    }

    for resource in &inventory.resources {
        required_references.insert(resource.label_ref.clone());
        let description_ref = resource.description_ref.as_ref().ok_or_else(|| {
            ConsoleOperationRegistryError::new(format!(
                "resource {} must provide a locale description reference",
                resource.resource_code
            ))
        })?;
        required_references.insert(description_ref.clone());
        for action in &resource.actions {
            required_references.insert(action.label_ref.clone());
            let description_ref = action.description_ref.as_ref().ok_or_else(|| {
                ConsoleOperationRegistryError::new(format!(
                    "resource action {}.{} must provide a locale description reference",
                    resource.resource_code, action.action_code
                ))
            })?;
            required_references.insert(description_ref.clone());
        }
    }

    for option in CONSOLE_POLICY_GROUP_MODE_OPTIONS
        .iter()
        .chain(CONSOLE_POLICY_ROW_SCOPE_OPTIONS)
    {
        required_references.insert(option.label_ref.to_string());
        required_references.insert(option.description_ref.to_string());
    }

    for (key, _) in &declared_other_groups {
        if !referenced_other_groups.contains(key) {
            return Err(ConsoleOperationRegistryError::new(format!(
                "unreferenced Other policy group display {}.{}",
                key.0, key.1
            )));
        }
    }
    for reference in &required_references {
        if !texts.contains_key(reference) {
            return Err(ConsoleOperationRegistryError::new(format!(
                "missing locale reference {reference}"
            )));
        }
    }
    for reference in texts.keys() {
        if !required_references.contains(reference) {
            return Err(ConsoleOperationRegistryError::new(format!(
                "unreferenced locale reference {reference}"
            )));
        }
    }

    Ok(ConsoleLocaleCatalog {
        texts,
        policy_group_displays,
    })
}

fn validate_operation(
    registration: &ConsoleOperationRegistration,
    settings_feature_ids: &BTreeSet<&str>,
    resources: &BTreeMap<String, ResourceAccessRegistration>,
) -> Result<(), ConsoleOperationRegistryError> {
    validate_non_empty(&registration.operation_id, "operation_id")?;
    validate_owner(&registration.owner, "operation owner")?;
    validate_host_extension_namespace(
        &registration.owner,
        &registration.operation_id,
        "operation_id",
    )?;
    validate_non_empty(&registration.label_ref, "operation label_ref")?;
    validate_optional_non_empty(
        registration.description_ref.as_deref(),
        "operation description_ref",
    )?;
    if registration.lifecycle == SettingsFeatureLifecycle::Inactive {
        return Err(ConsoleOperationRegistryError::new(format!(
            "inactive operation {} cannot own console routes",
            registration.operation_id
        )));
    }
    if registration.routes.is_empty() {
        return Err(ConsoleOperationRegistryError::new(format!(
            "operation {} must own at least one console route",
            registration.operation_id
        )));
    }
    for route in &registration.routes {
        validate_route(route)?;
    }
    if let ConsolePolicyGroup::SettingsFeature(feature_id) = &registration.policy_group {
        if !settings_feature_ids.contains(feature_id.as_str()) {
            return Err(ConsoleOperationRegistryError::new(format!(
                "operation {} references unknown settings feature {feature_id}",
                registration.operation_id
            )));
        }
    }
    if let ConsolePolicyGroup::Other(group_id) = &registration.policy_group {
        validate_non_empty(group_id, "Other policy group_id")?;
    }
    if let ConsoleAuthorization::ResourceAction {
        resource_code,
        action_code,
    } = &registration.authorization
    {
        let known = resources.get(resource_code).is_some_and(|resource| {
            resource
                .actions
                .iter()
                .any(|action| action.action_code == *action_code)
        });
        if !known {
            return Err(ConsoleOperationRegistryError::new(format!(
                "operation {} references unknown resource action {resource_code}.{action_code}",
                registration.operation_id
            )));
        }
    }
    Ok(())
}

fn validate_resource(
    registration: &ResourceAccessRegistration,
) -> Result<(), ConsoleOperationRegistryError> {
    validate_non_empty(&registration.resource_code, "resource_code")?;
    validate_owner(&registration.owner, "resource owner")?;
    validate_host_extension_namespace(
        &registration.owner,
        &registration.resource_code,
        "resource_code",
    )?;
    validate_non_empty(&registration.identity_field, "resource identity_field")?;
    validate_optional_non_empty(registration.scope_field.as_deref(), "resource scope_field")?;
    validate_optional_non_empty(registration.owner_field.as_deref(), "resource owner_field")?;
    validate_non_empty(&registration.label_ref, "resource label_ref")?;
    validate_optional_non_empty(
        registration.description_ref.as_deref(),
        "resource description_ref",
    )?;
    if registration.lifecycle == SettingsFeatureLifecycle::Inactive {
        return Err(ConsoleOperationRegistryError::new(format!(
            "inactive resource {} cannot expose actions",
            registration.resource_code
        )));
    }
    if registration.actions.is_empty() {
        return Err(ConsoleOperationRegistryError::new(format!(
            "resource {} must register at least one action",
            registration.resource_code
        )));
    }
    let mut actions = BTreeSet::new();
    for action in &registration.actions {
        validate_non_empty(&action.action_code, "resource action_code")?;
        validate_non_empty(&action.label_ref, "resource action label_ref")?;
        validate_optional_non_empty(
            action.description_ref.as_deref(),
            "resource action description_ref",
        )?;
        if !actions.insert(action.action_code.as_str()) {
            return Err(ConsoleOperationRegistryError::new(format!(
                "duplicate resource action {}.{}",
                registration.resource_code, action.action_code
            )));
        }
    }
    Ok(())
}

fn validate_route(route: &ConsoleRouteBinding) -> Result<(), ConsoleOperationRegistryError> {
    let method = route.method.to_ascii_uppercase();
    if !matches!(
        method.as_str(),
        "GET" | "HEAD" | "OPTIONS" | "POST" | "PUT" | "PATCH" | "DELETE"
    ) {
        return Err(ConsoleOperationRegistryError::new(format!(
            "unsupported console route method {}",
            route.method
        )));
    }
    if !route.path.starts_with("/api/console/") && route.path != "/api/console" {
        return Err(ConsoleOperationRegistryError::new(format!(
            "console route path must start with /api/console: {}",
            route.path
        )));
    }
    Ok(())
}

fn validate_owner(
    owner: &ConsoleOperationOwner,
    field: &str,
) -> Result<(), ConsoleOperationRegistryError> {
    validate_non_empty(&owner.owner_id, &format!("{field} owner_id"))?;
    validate_non_empty(&owner.version, &format!("{field} version"))
}

fn validate_host_extension_namespace(
    owner: &ConsoleOperationOwner,
    identifier: &str,
    field: &str,
) -> Result<(), ConsoleOperationRegistryError> {
    if owner.kind == SettingsFeatureOwnerKind::HostExtension
        && identifier != owner.owner_id
        && !identifier.starts_with(&format!("{}.", owner.owner_id))
    {
        return Err(ConsoleOperationRegistryError::new(format!(
            "HostExtension {field} must equal owner_id or start with <owner_id>."
        )));
    }
    Ok(())
}

fn validate_non_empty(value: &str, field: &str) -> Result<(), ConsoleOperationRegistryError> {
    if value.trim().is_empty() {
        Err(ConsoleOperationRegistryError::new(format!(
            "{field} must not be empty"
        )))
    } else {
        Ok(())
    }
}

fn validate_optional_non_empty(
    value: Option<&str>,
    field: &str,
) -> Result<(), ConsoleOperationRegistryError> {
    if value.is_some_and(|value| value.trim().is_empty()) {
        Err(ConsoleOperationRegistryError::new(format!(
            "{field} must not be empty when present"
        )))
    } else {
        Ok(())
    }
}

fn normalize_route(mut route: ConsoleRouteBinding) -> ConsoleRouteBinding {
    route.method.make_ascii_uppercase();
    route
}

fn route_shape(path: &str) -> String {
    path.split('/')
        .map(|segment| {
            if segment.starts_with(':') || segment.starts_with('{') && segment.ends_with('}') {
                "{}"
            } else {
                segment
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn ownership_name(ownership: &ConsoleRouteOwnership) -> &str {
    match ownership {
        ConsoleRouteOwnership::Authenticated => "authenticated",
        ConsoleRouteOwnership::ConsoleOperation(operation_id) => operation_id,
    }
}

fn route_matches(template_shape: &str, request_path: &str) -> bool {
    let template_segments = template_shape.split('/').collect::<Vec<_>>();
    let request_segments = request_path.split('/').collect::<Vec<_>>();
    template_segments.len() == request_segments.len()
        && template_segments
            .iter()
            .zip(request_segments)
            .all(|(template, actual)| {
                *template == "{}" && !actual.is_empty() || template == &actual
            })
}

fn route_templates_overlap(left: &str, right: &str) -> bool {
    let left_segments = left.split('/').collect::<Vec<_>>();
    let right_segments = right.split('/').collect::<Vec<_>>();
    left_segments.len() == right_segments.len()
        && left_segments
            .iter()
            .zip(right_segments)
            .all(|(left, right)| {
                let left = *left;
                left == right || left == "{}" || right == "{}"
            })
}

fn route_templates_are_ambiguous(left: &str, right: &str) -> bool {
    if !route_templates_overlap(left, right) {
        return false;
    }

    let mut left_is_more_specific = false;
    let mut right_is_more_specific = false;
    for (left, right) in left.split('/').zip(right.split('/')) {
        match (left == "{}", right == "{}") {
            (false, true) => left_is_more_specific = true,
            (true, false) => right_is_more_specific = true,
            (false, false) | (true, true) => {}
        }
    }

    left_is_more_specific == right_is_more_specific
}

fn route_literal_specificity(path: &str) -> usize {
    path.split('/')
        .filter(|segment| !segment.is_empty() && *segment != "{}")
        .count()
}

fn settings_feature_operation_id(feature_id: &str) -> String {
    format!("settings_feature.access.{feature_id}")
}

fn console_policy_group_key(policy_group: &ConsolePolicyGroup) -> (String, String) {
    match policy_group {
        ConsolePolicyGroup::SettingsFeature(group_id) => {
            ("settings_feature".to_string(), group_id.clone())
        }
        ConsolePolicyGroup::Other(group_id) => ("other".to_string(), group_id.clone()),
    }
}

fn operation_map(
    operations: &[ConsoleOperationInventoryEntry],
) -> BTreeMap<&str, &ConsoleOperationInventoryEntry> {
    operations
        .iter()
        .map(|operation| (operation.operation_id.as_str(), operation))
        .collect()
}

fn resource_map(
    resources: &[ResourceAccessRegistration],
) -> BTreeMap<&str, &ResourceAccessRegistration> {
    resources
        .iter()
        .map(|resource| (resource.resource_code.as_str(), resource))
        .collect()
}

fn operation_contract_without_group(
    operation: &ConsoleOperationInventoryEntry,
) -> (
    &ConsoleOperationOwner,
    SettingsFeatureLifecycle,
    &str,
    Option<&str>,
    i32,
    &[ConsoleRouteBinding],
    &ConsoleAuthorization,
) {
    (
        &operation.owner,
        operation.lifecycle,
        &operation.label_ref,
        operation.description_ref.as_deref(),
        operation.order,
        &operation.routes,
        &operation.authorization,
    )
}

fn route_inventory(operations: &[ConsoleOperationInventoryEntry]) -> BTreeSet<String> {
    operations
        .iter()
        .flat_map(|operation| {
            operation.routes.iter().map(|route| {
                format!(
                    "{} {} -> {}",
                    route.method, route.path, operation.operation_id
                )
            })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsoleOperationRegistryError {
    message: String,
}

impl ConsoleOperationRegistryError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for ConsoleOperationRegistryError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(formatter)
    }
}

impl Error for ConsoleOperationRegistryError {}
