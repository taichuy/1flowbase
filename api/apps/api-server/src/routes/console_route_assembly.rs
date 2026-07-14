use std::{collections::BTreeSet, convert::Infallible, sync::Arc};

use access_control::{
    ConsoleAuthorization, ConsoleOperationOwner, ConsoleOperationRegistration,
    ConsoleOperationRegistry, ConsolePolicyGroup, ConsoleRouteAssemblyBinding, ConsoleRouteBinding,
    ConsoleRouteOwnership, ResourceAccessAction, ResourceAccessRegistration,
    ResourceAccessScopeKind, SettingsFeatureLifecycle, SettingsFeatureOwnerKind,
    SettingsFeatureRegistry, APPLICATIONS_CREATE_ACTION_CODE, APPLICATIONS_DELETE_ACTION_CODE,
    APPLICATIONS_RESOURCE_CODE, APPLICATIONS_UPDATE_ACTION_CODE, APPLICATIONS_VIEW_ACTION_CODE,
    DATA_SOURCES_VIEW_ACTION_CODE, DATA_SOURCE_INSTANCES_RESOURCE_CODE,
};
use axum::{
    handler::Handler,
    routing::{get, patch, post, MethodRouter},
    Router,
};
use plugin_framework::HostExtensionContributionManifest;

use super::core_console_i18n::core_console_locale_catalog_contribution;
use super::core_console_operation_specs::{
    CoreConsoleAuthorizationSpec, CoreConsoleOperationSpec, CoreConsolePolicyGroupSpec,
    CORE_CONSOLE_OPERATION_SPECS,
};
use crate::app_state::ApiState;

pub struct ConsoleMethodRouter<S> {
    router: MethodRouter<S, Infallible>,
    methods: Vec<(&'static str, ConsoleRouteOwnership)>,
}

pub fn console_get<H, T, S>(handler: H, ownership: ConsoleRouteOwnership) -> ConsoleMethodRouter<S>
where
    H: Handler<T, S>,
    T: 'static,
    S: Clone + Send + Sync + 'static,
{
    ConsoleMethodRouter {
        router: get(handler),
        methods: vec![("GET", ownership)],
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
        methods: vec![("DELETE", ownership)],
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
        methods: vec![("PATCH", ownership)],
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
        methods: vec![("POST", ownership)],
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
        methods: vec![("PUT", ownership)],
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
        self.methods.push(("DELETE", ownership));
        self
    }

    pub fn patch<H, T>(mut self, handler: H, ownership: ConsoleRouteOwnership) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
    {
        self.router = self.router.patch(handler);
        self.methods.push(("PATCH", ownership));
        self
    }

    pub fn post<H, T>(mut self, handler: H, ownership: ConsoleRouteOwnership) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
    {
        self.router = self.router.post(handler);
        self.methods.push(("POST", ownership));
        self
    }

    pub fn put<H, T>(mut self, handler: H, ownership: ConsoleRouteOwnership) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
    {
        self.router = self.router.put(handler);
        self.methods.push(("PUT", ownership));
        self
    }
}

pub struct ConsoleRouteAssembly<S> {
    router: Router<S>,
    bindings: Vec<ConsoleRouteAssemblyBinding>,
}

impl<S> ConsoleRouteAssembly<S>
where
    S: Clone + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self {
            router: Router::new(),
            bindings: Vec::new(),
        }
    }

    pub fn route(mut self, path: &'static str, methods: ConsoleMethodRouter<S>) -> Self {
        self.bindings
            .extend(methods.methods.into_iter().map(|(method, ownership)| {
                ConsoleRouteAssemblyBinding {
                    route: ConsoleRouteBinding {
                        method: method.to_string(),
                        path: format!("/api/console{path}"),
                    },
                    ownership,
                }
            }));
        self.router = self.router.route(path, methods.router);
        self
    }

    pub fn merge(mut self, other: Self) -> Self {
        self.router = self.router.merge(other.router);
        self.bindings.extend(other.bindings);
        self
    }

    pub fn bindings(&self) -> &[ConsoleRouteAssemblyBinding] {
        &self.bindings
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
    ConsoleRouteAssembly::new()
        .merge(console_health_route_assembly())
        .merge(super::session::route_assembly())
        .merge(super::me::route_assembly())
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
        .merge(super::host_infrastructure::route_assembly())
        .merge(super::mcp_management::route_assembly())
        .merge(super::user_api_keys::route_assembly())
        .merge(super::workspace::route_assembly())
        .merge(super::members::route_assembly())
        .merge(super::model_definitions::route_assembly())
        .merge(super::model_providers::route_assembly())
        .merge(super::frontend_block_catalog::route_assembly())
        .merge(super::js_dependencies::route_assembly())
        .merge(super::node_contributions::route_assembly())
        .merge(super::roles::route_assembly())
        .merge(super::permissions::route_assembly())
        .merge(super::frontstage::route_assembly())
        .merge(super::plugins::route_assembly())
        .merge(super::auth_center::route_assembly())
        .merge(super::system::route_assembly())
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

fn route_templates_match(left: &str, right: &str) -> bool {
    let left = left.trim_matches('/').split('/').collect::<Vec<_>>();
    let right = right.trim_matches('/').split('/').collect::<Vec<_>>();
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left == &right
                || ((left.starts_with(':') || left.starts_with('{'))
                    && (right.starts_with(':') || right.starts_with('{')))
        })
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
    match spec.authorization {
        CoreConsoleAuthorizationSpec::Authenticated => bindings
            .iter()
            .filter(|binding| binding.ownership == ConsoleRouteOwnership::Authenticated)
            .map(|binding| binding.route.clone())
            .collect(),
        CoreConsoleAuthorizationSpec::Simple
        | CoreConsoleAuthorizationSpec::ResourceAction { .. } => {
            routes_owned_by(bindings, spec.operation_id)
        }
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
                "no explicit Core or HostExtension operation specification for {operation_id}; register policy group, i18n refs, and authorization before mounting its route"
            );
        }
    }
    Ok(())
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
    validate_settings_feature_route_assembly(settings_features, bindings)?;
    let host_console_contributions = host_contributions
        .iter()
        .map(HostExtensionContributionManifest::console_contribution)
        .collect::<Result<Vec<_>, _>>()?;
    let host_operation_ids = host_console_contributions
        .iter()
        .flat_map(|contribution| contribution.operations.iter())
        .map(|operation| operation.operation_id.clone())
        .collect::<BTreeSet<_>>();
    validate_explicit_operation_specs(bindings, &host_operation_ids, settings_features)?;

    let core_owner = ConsoleOperationOwner {
        kind: SettingsFeatureOwnerKind::Core,
        owner_id: "boot-core".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    };
    let mut registrations = CORE_CONSOLE_OPERATION_SPECS
        .iter()
        .enumerate()
        .map(|(order, spec)| ConsoleOperationRegistration {
            operation_id: spec.operation_id.to_string(),
            owner: core_owner.clone(),
            lifecycle: SettingsFeatureLifecycle::Active,
            policy_group: policy_group_for_spec(spec),
            label_ref: spec.label_ref.to_string(),
            description_ref: Some(spec.description_ref.to_string()),
            order: order as i32,
            routes: routes_for_core_operation_spec(spec, bindings),
            authorization: authorization_for_spec(spec),
        })
        .collect::<Vec<_>>();
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
        label_ref: "console.resources.applications.label".to_string(),
        description_ref: Some("console.resources.applications.description".to_string()),
        actions: [
            APPLICATIONS_CREATE_ACTION_CODE,
            APPLICATIONS_VIEW_ACTION_CODE,
            APPLICATIONS_UPDATE_ACTION_CODE,
            APPLICATIONS_DELETE_ACTION_CODE,
        ]
        .into_iter()
        .map(|action_code| ResourceAccessAction {
            action_code: action_code.to_string(),
            label_ref: format!("console.resources.applications.actions.{action_code}.label"),
            description_ref: Some(format!(
                "console.resources.applications.actions.{action_code}.description"
            )),
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
        label_ref: "console.resources.data_source_instances.label".to_string(),
        description_ref: Some("console.resources.data_source_instances.description".to_string()),
        actions: vec![ResourceAccessAction {
            action_code: DATA_SOURCES_VIEW_ACTION_CODE.to_string(),
            label_ref: "console.resources.data_source_instances.actions.view.label".to_string(),
            description_ref: Some(
                "console.resources.data_source_instances.actions.view.description".to_string(),
            ),
        }],
    };
    registrations.extend(
        host_console_contributions
            .iter()
            .flat_map(|contribution| contribution.operations.iter().cloned()),
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
        .collect::<Vec<_>>();
    let registry = ConsoleOperationRegistry::compile_with_locale_catalog(
        settings_features,
        registrations,
        resources,
        locale_contributions,
    )?;
    registry.validate_console_route_coverage(bindings.iter().cloned())?;
    Ok(registry)
}

pub fn validate_migrated_core_console_route_coverage(
    settings_features: &SettingsFeatureRegistry,
) -> anyhow::Result<()> {
    let assembly = migrated_core_console_route_assembly();
    compile_migrated_core_console_operation_registry(settings_features, assembly.bindings())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use access_control::ConsoleRouteOwnership;

    use super::{
        console_health_route_assembly, migrated_core_console_route_assembly, ConsoleRouteAssembly,
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
            .merge(crate::routes::mcp_management::route_assembly())
            .merge(crate::routes::user_api_keys::route_assembly())
            .merge(crate::routes::workspace::route_assembly())
            .merge(crate::routes::members::route_assembly())
            .merge(crate::routes::model_definitions::route_assembly())
            .merge(crate::routes::model_providers::route_assembly())
            .merge(crate::routes::frontend_block_catalog::route_assembly())
            .merge(crate::routes::js_dependencies::route_assembly())
            .merge(crate::routes::node_contributions::route_assembly())
            .merge(crate::routes::roles::route_assembly())
            .merge(crate::routes::permissions::route_assembly())
            .merge(crate::routes::frontstage::route_assembly())
            .merge(crate::routes::plugins::route_assembly())
            .merge(crate::routes::auth_center::route_assembly())
            .merge(crate::routes::system::route_assembly())
            .merge(crate::routes::workspaces::route_assembly());

        assert_eq!(
            route_keys(&migrated_core_console_route_assembly()),
            route_keys(&expected)
        );
    }
}
