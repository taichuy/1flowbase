use std::{
    collections::{BTreeMap, BTreeSet},
    convert::Infallible,
    sync::Arc,
};

use access_control::{
    ConsoleAuthorization, ConsoleInterfaceRegistration, ConsoleOperationOwner,
    ConsoleOperationRegistration, ConsoleOperationRegistry, ConsolePolicyGroup,
    ConsoleRouteAssemblyBinding, ConsoleRouteBinding, ConsoleRouteOwnership, ResourceAccessAction,
    ResourceAccessRegistration, ResourceAccessScopeKind, SettingsFeatureLifecycle,
    SettingsFeatureOwnerKind, SettingsFeatureRegistry, APPLICATIONS_CREATE_ACTION_CODE,
    APPLICATIONS_DELETE_ACTION_CODE, APPLICATIONS_RESOURCE_CODE, APPLICATIONS_UPDATE_ACTION_CODE,
    APPLICATIONS_VIEW_ACTION_CODE, DATA_SOURCES_VIEW_ACTION_CODE,
    DATA_SOURCE_INSTANCES_RESOURCE_CODE,
};
use axum::{
    handler::Handler,
    routing::{get, patch, post, MethodRouter},
    Router,
};
use plugin_framework::HostExtensionContributionManifest;
use utoipa::OpenApi;

use super::core_console_i18n::core_console_locale_catalog_contribution;
use super::core_console_operation_specs::{
    CoreConsoleAuthorizationSpec, CoreConsoleOperationSpec, CoreConsolePolicyGroupSpec,
    CoreConsoleRouteSelector, CORE_CONSOLE_OPERATION_SPECS,
};
use crate::app_state::ApiState;

pub struct ConsoleMethodRouter<S> {
    router: MethodRouter<S, Infallible>,
    methods: Vec<(
        &'static str,
        ConsoleRouteOwnership,
        Option<crate::middleware::system_maintenance::SystemMaintenanceRequestClass>,
    )>,
}

pub fn console_get<H, T, S>(handler: H, ownership: ConsoleRouteOwnership) -> ConsoleMethodRouter<S>
where
    H: Handler<T, S>,
    T: 'static,
    S: Clone + Send + Sync + 'static,
{
    ConsoleMethodRouter {
        router: get(handler),
        methods: vec![("GET", ownership, None)],
    }
}

pub fn console_delete<H, T, S>(
    handler: H,
    ownership: ConsoleRouteOwnership,
) -> ConsoleMethodRouter<S>
where
    H: Handler<T, S>,
    T: 'static,
    S: Clone + Send + Sync + 'static,
{
    ConsoleMethodRouter {
        router: axum::routing::delete(handler),
        methods: vec![("DELETE", ownership, None)],
    }
}

pub fn console_patch<H, T, S>(
    handler: H,
    ownership: ConsoleRouteOwnership,
) -> ConsoleMethodRouter<S>
where
    H: Handler<T, S>,
    T: 'static,
    S: Clone + Send + Sync + 'static,
{
    ConsoleMethodRouter {
        router: patch(handler),
        methods: vec![("PATCH", ownership, None)],
    }
}

pub fn console_post<H, T, S>(handler: H, ownership: ConsoleRouteOwnership) -> ConsoleMethodRouter<S>
where
    H: Handler<T, S>,
    T: 'static,
    S: Clone + Send + Sync + 'static,
{
    ConsoleMethodRouter {
        router: post(handler),
        methods: vec![("POST", ownership, None)],
    }
}

pub fn console_put<H, T, S>(handler: H, ownership: ConsoleRouteOwnership) -> ConsoleMethodRouter<S>
where
    H: Handler<T, S>,
    T: 'static,
    S: Clone + Send + Sync + 'static,
{
    ConsoleMethodRouter {
        router: axum::routing::put(handler),
        methods: vec![("PUT", ownership, None)],
    }
}

impl<S> ConsoleMethodRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    pub fn delete<H, T>(mut self, handler: H, ownership: ConsoleRouteOwnership) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
    {
        self.router = self.router.delete(handler);
        self.methods.push(("DELETE", ownership, None));
        self
    }

    pub fn patch<H, T>(mut self, handler: H, ownership: ConsoleRouteOwnership) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
    {
        self.router = self.router.patch(handler);
        self.methods.push(("PATCH", ownership, None));
        self
    }

    pub fn post<H, T>(mut self, handler: H, ownership: ConsoleRouteOwnership) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
    {
        self.router = self.router.post(handler);
        self.methods.push(("POST", ownership, None));
        self
    }

    pub fn put<H, T>(mut self, handler: H, ownership: ConsoleRouteOwnership) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
    {
        self.router = self.router.put(handler);
        self.methods.push(("PUT", ownership, None));
        self
    }

    pub fn coordinator_control(mut self) -> Self {
        if let Some((_, _, request_class)) = self.methods.last_mut() {
            *request_class = Some(
                crate::middleware::system_maintenance::SystemMaintenanceRequestClass::CoordinatorControl,
            );
        }
        self
    }
}

#[derive(Clone)]
pub struct ConsoleRouteAssembly<S> {
    router: Router<S>,
    bindings: Vec<ConsoleRouteAssemblyBinding>,
    maintenance_control_routes: Vec<ConsoleRouteBinding>,
}

impl<S> Default for ConsoleRouteAssembly<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<S> ConsoleRouteAssembly<S>
where
    S: Clone + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self {
            router: Router::new(),
            bindings: Vec::new(),
            maintenance_control_routes: Vec::new(),
        }
    }

    pub fn route(mut self, path: &str, methods: ConsoleMethodRouter<S>) -> Self {
        let ConsoleMethodRouter { router, methods } = methods;
        for (method, ownership, request_class) in methods {
            let route = ConsoleRouteBinding {
                method: method.to_string(),
                path: format!("/api/console{path}"),
            };
            if request_class.is_some() {
                self.maintenance_control_routes.push(route.clone());
            }
            self.bindings
                .push(ConsoleRouteAssemblyBinding { route, ownership });
        }
        self.router = self.router.route(
            &crate::external_route_assembly::axum_route_path(path),
            router,
        );
        self
    }

    pub fn merge(mut self, other: Self) -> Self {
        self.router = self.router.merge(other.router);
        self.bindings.extend(other.bindings);
        self.maintenance_control_routes
            .extend(other.maintenance_control_routes);
        self
    }

    pub fn bindings(&self) -> &[ConsoleRouteAssemblyBinding] {
        &self.bindings
    }

    #[cfg(test)]
    pub(crate) fn external_endpoint_contributions(
        &self,
    ) -> impl Iterator<Item = crate::external_endpoint_catalog::ExternalEndpointContribution> + '_
    {
        self.bindings.iter().map(|binding| {
            crate::external_endpoint_catalog::ExternalEndpointContribution::unclassified_http(
                "console-route-assembly",
                &binding.route.method,
                &binding.route.path,
            )
        })
    }

    pub fn maintenance_control_routes(&self) -> &[ConsoleRouteBinding] {
        &self.maintenance_control_routes
    }

    pub fn into_router(self) -> Router<S> {
        self.router
    }
}

fn console_health_route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::Authenticated;

    ConsoleRouteAssembly::new().route("/health", console_get(crate::console_health, Authenticated))
}

pub fn migrated_core_console_route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    migrated_core_console_route_assembly_with_interface_operations(None)
}

/// Core contract bindings come from the same route assembly as the mounted routes.
pub fn migrated_core_console_contract_bindings() -> Vec<ConsoleRouteAssemblyBinding> {
    migrated_core_console_route_assembly().bindings().to_vec()
}

pub(crate) fn migrated_core_console_route_assembly_with_interface_operations(
    interface_registry: Option<&interface_runtime::CompiledInterfaceRegistry>,
) -> ConsoleRouteAssembly<Arc<ApiState>> {
    migrated_core_console_route_assembly_with_interface_operations_and_plugin_upload_max_bytes(
        interface_registry,
        crate::config::DEFAULT_PLUGIN_UPLOAD_MAX_BYTES,
    )
}

pub(crate) fn migrated_core_console_route_assembly_with_interface_operations_and_plugin_upload_max_bytes(
    interface_registry: Option<&interface_runtime::CompiledInterfaceRegistry>,
    plugin_upload_max_bytes: usize,
) -> ConsoleRouteAssembly<Arc<ApiState>> {
    ConsoleRouteAssembly::new()
        .merge(console_health_route_assembly())
        .merge(super::session::route_assembly())
        .merge(super::me::route_assembly())
        .merge(super::external_auth::console_route_assembly())
        .merge(super::me_authentication_methods::route_assembly())
        .merge(super::assistant::route_assembly())
        .merge(super::navigation::route_assembly())
        .merge(super::application_management::route_assembly())
        .merge(super::applications::route_assembly())
        .merge(super::application_api::route_assembly())
        .merge(super::application_orchestration::route_assembly())
        .merge(super::application_runtime::route_assembly())
        .merge(super::data_models::route_assembly())
        .merge(super::docs::route_assembly())
        .merge(super::data_sources::route_assembly())
        .merge(super::files::route_assembly())
        .merge(super::file_storages::route_assembly())
        .merge(super::file_tables::route_assembly())
        .merge(
            super::host_infrastructure::route_assembly_with_interface_operations(
                interface_registry,
            ),
        )
        .merge(super::runtime_i18n_catalog::route_assembly())
        .merge(super::i18n_catalog::route_assembly())
        .merge(super::mcp_management::route_assembly())
        .merge(super::user_api_keys::route_assembly())
        .merge(super::workspace::route_assembly())
        .merge(super::members::route_assembly())
        .merge(super::notifications::route_assembly())
        .merge(super::communications::route_assembly())
        .merge(super::organization::route_assembly())
        .merge(super::billing::route_assembly())
        .merge(super::model_definitions::route_assembly())
        .merge(super::model_providers::route_assembly())
        .merge(
            super::network_center::route_assembly_with_plugin_upload_max_bytes(
                plugin_upload_max_bytes,
            ),
        )
        .merge(super::frontend_block_catalog::route_assembly())
        .merge(super::js_dependencies::route_assembly())
        .merge(super::node_contributions::route_assembly())
        .merge(super::roles::route_assembly())
        .merge(super::permissions::route_assembly())
        .merge(super::frontstage::route_assembly())
        .merge(super::plugins::route_assembly_with_plugin_upload_max_bytes(
            plugin_upload_max_bytes,
        ))
        .merge(super::auth_center::route_assembly())
        .merge(super::branding::route_assembly())
        .merge(super::system::route_assembly())
        .merge(super::system_backups::route_assembly())
        .merge(super::system_templates::route_assembly())
        .merge(super::ui_management::route_assembly())
        .merge(super::workspaces::route_assembly())
}

fn routes_owned_by(
    bindings: &[ConsoleRouteAssemblyBinding],
    operation_id: &str,
) -> Vec<ConsoleRouteBinding> {
    bindings
        .iter()
        .filter_map(|binding| match &binding.ownership {
            ConsoleRouteOwnership::ConsoleOperation(owner) if owner == operation_id => {
                Some(binding.route.clone())
            }
            ConsoleRouteOwnership::Authenticated | ConsoleRouteOwnership::ConsoleOperation(_) => {
                None
            }
        })
        .collect()
}

pub(crate) fn route_templates_match(left: &str, right: &str) -> bool {
    let left = left.trim_matches('/').split('/').collect::<Vec<_>>();
    let right = right.trim_matches('/').split('/').collect::<Vec<_>>();
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left == &right
                || ((left.starts_with(':') || left.starts_with('{'))
                    && (right.starts_with(':') || right.starts_with('{')))
        })
}

fn route_identity_shape(path: &str) -> String {
    path.split('/')
        .map(|segment| {
            if segment.starts_with(':') || (segment.starts_with('{') && segment.ends_with('}')) {
                "{}"
            } else {
                segment
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn validate_settings_feature_route_assembly(
    settings_features: &SettingsFeatureRegistry,
    bindings: &[ConsoleRouteAssemblyBinding],
) -> anyhow::Result<()> {
    for feature in &settings_features.inventory().features {
        for route in &feature.api_routes {
            let assembled = bindings.iter().any(|binding| {
                binding.route.method.eq_ignore_ascii_case(&route.method)
                    && route_templates_match(&binding.route.path, &route.path)
            });
            if !assembled {
                anyhow::bail!(
                    "settings feature route has no compiled assembly: {} {}",
                    route.method,
                    route.path
                );
            }
        }
    }
    Ok(())
}

fn policy_group_for_spec(spec: &CoreConsoleOperationSpec) -> ConsolePolicyGroup {
    match spec.policy_group {
        CoreConsolePolicyGroupSpec::SettingsFeature(feature_id) => {
            ConsolePolicyGroup::SettingsFeature(feature_id.to_string())
        }
        CoreConsolePolicyGroupSpec::Other(group_id) => {
            ConsolePolicyGroup::Other(group_id.to_string())
        }
    }
}

fn authorization_for_spec(spec: &CoreConsoleOperationSpec) -> ConsoleAuthorization {
    match spec.authorization {
        CoreConsoleAuthorizationSpec::Authenticated => ConsoleAuthorization::Authenticated,
        CoreConsoleAuthorizationSpec::Simple => ConsoleAuthorization::Simple,
        CoreConsoleAuthorizationSpec::ResourceAction {
            resource_code,
            action_code,
        } => ConsoleAuthorization::ResourceAction {
            resource_code: resource_code.to_string(),
            action_code: action_code.to_string(),
        },
    }
}

fn routes_for_core_operation_spec(
    spec: &CoreConsoleOperationSpec,
    bindings: &[ConsoleRouteAssemblyBinding],
) -> Vec<ConsoleRouteBinding> {
    match spec.route_selector {
        CoreConsoleRouteSelector::AuthenticatedRoutes => bindings
            .iter()
            .filter(|binding| binding.ownership == ConsoleRouteOwnership::Authenticated)
            .map(|binding| binding.route.clone())
            .collect(),
        CoreConsoleRouteSelector::OwnedOperation => routes_owned_by(bindings, spec.operation_id),
    }
}

fn validate_explicit_operation_specs(
    bindings: &[ConsoleRouteAssemblyBinding],
    host_operation_ids: &BTreeSet<String>,
    settings_features: &SettingsFeatureRegistry,
) -> anyhow::Result<()> {
    let projected_settings_feature_operations = settings_features
        .inventory()
        .features
        .iter()
        .map(|feature| format!("settings_feature.access.{}", feature.feature_id))
        .collect::<BTreeSet<_>>();
    for binding in bindings {
        let ConsoleRouteOwnership::ConsoleOperation(operation_id) = &binding.ownership else {
            continue;
        };
        if !CORE_CONSOLE_OPERATION_SPECS
            .iter()
            .any(|spec| spec.operation_id == operation_id)
            && !host_operation_ids.contains(operation_id)
            && !projected_settings_feature_operations.contains(operation_id)
        {
            anyhow::bail!(
                "no explicit Core or HostExtension operation specification for {operation_id}; register policy group and authorization before mounting its route"
            );
        }
    }
    Ok(())
}

fn validate_operation_spec_exact_set(
    route_operation_ids: impl IntoIterator<Item = String>,
    specification_ids: impl IntoIterator<Item = String>,
) -> anyhow::Result<()> {
    let route_operation_ids = route_operation_ids.into_iter().collect::<BTreeSet<_>>();
    let mut specification_counts = BTreeMap::<String, usize>::new();
    for specification_id in specification_ids {
        *specification_counts.entry(specification_id).or_default() += 1;
    }
    let specification_ids = specification_counts
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let missing = route_operation_ids
        .difference(&specification_ids)
        .cloned()
        .collect::<Vec<_>>();
    let extra = specification_ids
        .difference(&route_operation_ids)
        .cloned()
        .collect::<Vec<_>>();
    let duplicates = specification_counts
        .into_iter()
        .filter_map(|(operation_id, count)| (count > 1).then_some(operation_id))
        .collect::<Vec<_>>();
    if !missing.is_empty() || !extra.is_empty() || !duplicates.is_empty() {
        anyhow::bail!(
            "console operation specification exact-set mismatch; missing=[{}]; extra=[{}]; duplicate=[{}]",
            missing.join(", "),
            extra.join(", "),
            duplicates.join(", "),
        );
    }
    Ok(())
}

fn validate_complete_explicit_operation_specs(
    bindings: &[ConsoleRouteAssemblyBinding],
    host_operation_ids: &[String],
    settings_features: &SettingsFeatureRegistry,
) -> anyhow::Result<()> {
    let route_operation_ids = bindings
        .iter()
        .filter_map(|binding| match &binding.ownership {
            ConsoleRouteOwnership::ConsoleOperation(operation_id) => Some(operation_id.clone()),
            ConsoleRouteOwnership::Authenticated => None,
        })
        .collect::<BTreeSet<_>>();
    let mut specification_ids = CORE_CONSOLE_OPERATION_SPECS
        .iter()
        .filter(|spec| spec.route_selector == CoreConsoleRouteSelector::OwnedOperation)
        .map(|spec| spec.operation_id.to_string())
        .chain(host_operation_ids.iter().cloned())
        .collect::<Vec<_>>();
    let declared_specifications = specification_ids.iter().cloned().collect::<BTreeSet<_>>();
    specification_ids.extend(
        settings_features
            .inventory()
            .features
            .iter()
            .map(|feature| format!("settings_feature.access.{}", feature.feature_id))
            .filter(|operation_id| {
                route_operation_ids.contains(operation_id)
                    && !declared_specifications.contains(operation_id)
            }),
    );
    validate_operation_spec_exact_set(route_operation_ids, specification_ids)
}

pub fn compile_migrated_core_console_operation_registry(
    settings_features: &SettingsFeatureRegistry,
    bindings: &[ConsoleRouteAssemblyBinding],
) -> anyhow::Result<ConsoleOperationRegistry> {
    compile_migrated_console_operation_registry(settings_features, bindings, &[])
}

pub(crate) fn compile_migrated_console_operation_registry(
    settings_features: &SettingsFeatureRegistry,
    bindings: &[ConsoleRouteAssemblyBinding],
    host_contributions: &[HostExtensionContributionManifest],
) -> anyhow::Result<ConsoleOperationRegistry> {
    compile_migrated_console_operation_registry_with_managed(
        settings_features,
        bindings,
        host_contributions,
        &[],
    )
}

pub(crate) fn compile_migrated_console_operation_registry_with_managed(
    settings_features: &SettingsFeatureRegistry,
    bindings: &[ConsoleRouteAssemblyBinding],
    host_contributions: &[HostExtensionContributionManifest],
    managed_services: &[crate::managed_services::ManagedServiceRegistration],
) -> anyhow::Result<ConsoleOperationRegistry> {
    validate_settings_feature_route_assembly(settings_features, bindings)?;
    let host_console_contributions = host_contributions
        .iter()
        .map(HostExtensionContributionManifest::console_contribution)
        .collect::<Result<Vec<_>, _>>()?;
    let host_operation_ids = host_console_contributions
        .iter()
        .flat_map(|contribution| contribution.operations.iter())
        .map(|operation| operation.operation_id.clone())
        .chain(
            managed_services
                .iter()
                .flat_map(|s| s.operations())
                .map(|op| op.operation_id),
        )
        .collect::<BTreeSet<_>>();
    validate_explicit_operation_specs(bindings, &host_operation_ids, settings_features)?;
    if !managed_services.is_empty() {
        validate_complete_explicit_operation_specs(
            bindings,
            &host_operation_ids.iter().cloned().collect::<Vec<_>>(),
            settings_features,
        )?;
    }

    let core_owner = ConsoleOperationOwner {
        kind: SettingsFeatureOwnerKind::Core,
        owner_id: "boot-core".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    };
    let registrations = CORE_CONSOLE_OPERATION_SPECS
        .iter()
        .enumerate()
        .map(|(order, spec)| {
            let routes = routes_for_core_operation_spec(spec, bindings);
            ConsoleOperationRegistration {
                operation_id: spec.operation_id.to_string(),
                authorization_profile_id: (spec.route_selector
                    == CoreConsoleRouteSelector::OwnedOperation)
                    .then(|| spec.operation_id.to_string()),
                owner: core_owner.clone(),
                lifecycle: SettingsFeatureLifecycle::Active,
                policy_group: policy_group_for_spec(spec),
                order: order as i32,
                routes,
                authorization: authorization_for_spec(spec),
            }
        })
        .collect::<Vec<_>>();
    let mut registrations = expand_core_interface_registrations(registrations)?;
    let applications_resource = ResourceAccessRegistration {
        resource_code: APPLICATIONS_RESOURCE_CODE.to_string(),
        owner: core_owner.clone(),
        lifecycle: SettingsFeatureLifecycle::Active,
        scope_kind: ResourceAccessScopeKind::Workspace,
        identity_field: "id".to_string(),
        // #1259 freezes the logical access contract as scope_id. The application repository still
        // maps its workspace_id storage field; that enforcement cutover belongs to #1271.
        scope_field: Some("scope_id".to_string()),
        owner_field: Some("created_by".to_string()),
        label_ref: "Applications".to_string(),
        description_ref: Some("Applications in the current workspace".to_string()),
        actions: [
            APPLICATIONS_CREATE_ACTION_CODE,
            APPLICATIONS_VIEW_ACTION_CODE,
            APPLICATIONS_UPDATE_ACTION_CODE,
            APPLICATIONS_DELETE_ACTION_CODE,
        ]
        .into_iter()
        .map(|action_code| ResourceAccessAction {
            action_code: action_code.to_string(),
            label_ref: match action_code {
                APPLICATIONS_CREATE_ACTION_CODE => "Create",
                APPLICATIONS_VIEW_ACTION_CODE => "View",
                APPLICATIONS_UPDATE_ACTION_CODE => "Update",
                APPLICATIONS_DELETE_ACTION_CODE => "Delete",
                _ => unreachable!("compiled application action code must be known"),
            }
            .to_string(),
            description_ref: Some(
                match action_code {
                    APPLICATIONS_CREATE_ACTION_CODE => "Create an application",
                    APPLICATIONS_VIEW_ACTION_CODE => "View an application",
                    APPLICATIONS_UPDATE_ACTION_CODE => "Update an application",
                    APPLICATIONS_DELETE_ACTION_CODE => "Delete an application",
                    _ => unreachable!("compiled application action code must be known"),
                }
                .to_string(),
            ),
        })
        .collect(),
    };
    let data_source_instances_resource = ResourceAccessRegistration {
        resource_code: DATA_SOURCE_INSTANCES_RESOURCE_CODE.to_string(),
        owner: core_owner.clone(),
        lifecycle: SettingsFeatureLifecycle::Active,
        scope_kind: ResourceAccessScopeKind::Workspace,
        identity_field: "id".to_string(),
        scope_field: Some("scope_id".to_string()),
        owner_field: Some("created_by".to_string()),
        label_ref: "Data source instances".to_string(),
        description_ref: Some(
            "Configured data source instances in the current workspace".to_string(),
        ),
        actions: vec![ResourceAccessAction {
            action_code: DATA_SOURCES_VIEW_ACTION_CODE.to_string(),
            label_ref: "View".to_string(),
            description_ref: Some("View a data source instance".to_string()),
        }],
    };
    registrations.extend(
        host_console_contributions
            .iter()
            .flat_map(|contribution| contribution.operations.iter().cloned()),
    );
    registrations.extend(
        managed_services
            .iter()
            .flat_map(|service| service.operations()),
    );
    let mut resources = vec![applications_resource, data_source_instances_resource];
    resources.extend(
        host_console_contributions
            .iter()
            .flat_map(|contribution| contribution.resources.iter().cloned()),
    );
    let locale_contributions = std::iter::once(core_console_locale_catalog_contribution())
        .chain(
            host_console_contributions
                .iter()
                .map(|contribution| contribution.locale_catalog.clone()),
        )
        .chain(
            managed_services
                .iter()
                .map(|service| service.locale_catalog()),
        )
        .collect::<Vec<_>>();
    let registry = ConsoleOperationRegistry::compile_with_locale_catalog(
        settings_features,
        registrations,
        resources,
        locale_contributions,
    )?;
    let metadata_bindings = bindings
        .iter()
        .filter(|binding| {
            !managed_services
                .iter()
                .flat_map(|service| &service.declaration.operations)
                .any(|op| {
                    op.stream.is_some()
                        && binding.route.method == "GET"
                        && route_templates_match(&op.path, &binding.route.path)
                })
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut interfaces = compile_console_interface_metadata(&metadata_bindings, &registry)?;
    for metadata in managed_services.iter().flat_map(|s| s.metadata()) {
        interfaces.retain(|existing| existing.interface_id != metadata.interface_id);
        interfaces.push(metadata);
    }
    let registry = registry.with_interface_metadata(interfaces)?;
    let interface_bindings = bindings.iter().cloned().map(|mut binding| {
        if !matches!(binding.ownership, ConsoleRouteOwnership::Authenticated) {
            if let Some(operation) = registry.inventory().operations.iter().find(|operation| {
                operation.routes.first().is_some_and(|route| {
                    route.method.eq_ignore_ascii_case(&binding.route.method)
                        && route_templates_match(&route.path, &binding.route.path)
                })
            }) {
                binding.ownership =
                    ConsoleRouteOwnership::ConsoleOperation(operation.operation_id.clone());
            }
        }
        binding
    });
    registry.validate_console_route_coverage(interface_bindings)?;
    Ok(registry)
}

pub(crate) fn compile_complete_migrated_console_operation_registry(
    settings_features: &SettingsFeatureRegistry,
    bindings: &[ConsoleRouteAssemblyBinding],
    host_contributions: &[HostExtensionContributionManifest],
) -> anyhow::Result<ConsoleOperationRegistry> {
    let host_operation_ids = host_contributions
        .iter()
        .map(HostExtensionContributionManifest::console_contribution)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flat_map(|contribution| contribution.operations)
        .map(|operation| operation.operation_id)
        .collect::<Vec<_>>();
    validate_complete_explicit_operation_specs(bindings, &host_operation_ids, settings_features)?;
    compile_migrated_console_operation_registry(settings_features, bindings, host_contributions)
}

pub fn validate_migrated_core_console_route_coverage(
    settings_features: &SettingsFeatureRegistry,
) -> anyhow::Result<()> {
    let bindings = migrated_core_console_contract_bindings();
    compile_complete_migrated_console_operation_registry(settings_features, &bindings, &[])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use access_control::ConsoleRouteOwnership;

    use super::{
        compile_complete_migrated_console_operation_registry, console_health_route_assembly,
        migrated_core_console_contract_bindings, migrated_core_console_route_assembly,
        validate_operation_spec_exact_set, ConsoleRouteAssembly,
    };

    fn route_keys<S>(assembly: &ConsoleRouteAssembly<S>) -> BTreeSet<(String, String, String)>
    where
        S: Clone + Send + Sync + 'static,
    {
        assembly
            .bindings()
            .iter()
            .map(|binding| {
                (
                    binding.route.method.clone(),
                    binding.route.path.clone(),
                    match &binding.ownership {
                        ConsoleRouteOwnership::Authenticated => "authenticated".to_string(),
                        ConsoleRouteOwnership::ConsoleOperation(operation_id) => {
                            operation_id.clone()
                        }
                    },
                )
            })
            .collect()
    }

    #[test]
    fn migrated_assembly_contains_every_console_router_owner_assembly() {
        let expected = ConsoleRouteAssembly::new()
            .merge(console_health_route_assembly())
            .merge(crate::routes::session::route_assembly())
            .merge(crate::routes::me::route_assembly())
            .merge(crate::routes::external_auth::console_route_assembly())
            .merge(crate::routes::me_authentication_methods::route_assembly())
            .merge(crate::routes::assistant::route_assembly())
            .merge(crate::routes::navigation::route_assembly())
            .merge(crate::routes::application_management::route_assembly())
            .merge(crate::routes::applications::route_assembly())
            .merge(crate::routes::application_api::route_assembly())
            .merge(crate::routes::application_orchestration::route_assembly())
            .merge(crate::routes::application_runtime::route_assembly())
            .merge(crate::routes::data_models::route_assembly())
            .merge(crate::routes::docs::route_assembly())
            .merge(crate::routes::data_sources::route_assembly())
            .merge(crate::routes::files::route_assembly())
            .merge(crate::routes::file_storages::route_assembly())
            .merge(crate::routes::file_tables::route_assembly())
            .merge(crate::routes::host_infrastructure::route_assembly())
            .merge(crate::routes::runtime_i18n_catalog::route_assembly())
            .merge(crate::routes::i18n_catalog::route_assembly())
            .merge(crate::routes::mcp_management::route_assembly())
            .merge(crate::routes::user_api_keys::route_assembly())
            .merge(crate::routes::workspace::route_assembly())
            .merge(crate::routes::members::route_assembly())
            .merge(crate::routes::notifications::route_assembly())
            .merge(crate::routes::communications::route_assembly())
            .merge(crate::routes::organization::route_assembly())
            .merge(crate::routes::model_definitions::route_assembly())
            .merge(crate::routes::model_providers::route_assembly())
            .merge(crate::routes::network_center::route_assembly())
            .merge(crate::routes::frontend_block_catalog::route_assembly())
            .merge(crate::routes::js_dependencies::route_assembly())
            .merge(crate::routes::node_contributions::route_assembly())
            .merge(crate::routes::ui_management::route_assembly())
            .merge(crate::routes::roles::route_assembly())
            .merge(crate::routes::permissions::route_assembly())
            .merge(crate::routes::frontstage::route_assembly())
            .merge(crate::routes::plugins::route_assembly())
            .merge(crate::routes::auth_center::route_assembly())
            .merge(crate::routes::billing::route_assembly())
            .merge(crate::routes::branding::route_assembly())
            .merge(crate::routes::system::route_assembly())
            .merge(crate::routes::system_backups::route_assembly())
            .merge(crate::routes::system_templates::route_assembly())
            .merge(crate::routes::workspaces::route_assembly());

        assert_eq!(
            route_keys(&migrated_core_console_route_assembly()),
            route_keys(&expected)
        );
    }

    #[test]
    fn complete_operation_specifications_match_the_production_route_set() {
        let settings = crate::app_state::compile_core_settings_feature_registry().unwrap();
        let bindings = migrated_core_console_contract_bindings();

        compile_complete_migrated_console_operation_registry(&settings, &bindings, &[]).unwrap();
    }

    #[test]
    fn operation_specification_exact_set_reports_every_difference() {
        let error = validate_operation_spec_exact_set(
            [
                "operation.present",
                "operation.missing",
                "operation.present",
            ]
            .into_iter()
            .map(str::to_string),
            ["operation.present", "operation.extra", "operation.extra"]
                .into_iter()
                .map(str::to_string),
        )
        .unwrap_err();
        let message = error.to_string();

        assert!(message.contains("missing=[operation.missing]"));
        assert!(message.contains("extra=[operation.extra]"));
        assert!(message.contains("duplicate=[operation.extra]"));
    }
}

#[path = "console_route_assembly/interface_metadata.rs"]
mod interface_metadata;

use interface_metadata::*;
