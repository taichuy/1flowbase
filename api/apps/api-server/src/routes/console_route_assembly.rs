use std::{collections::BTreeSet, convert::Infallible};

use access_control::{
    ConsoleAuthorization, ConsoleOperationOwner, ConsoleOperationRegistration,
    ConsoleOperationRegistry, ConsolePolicyGroup, ConsoleRouteAssemblyBinding, ConsoleRouteBinding,
    ConsoleRouteOwnership, ResourceAccessAction, ResourceAccessRegistration,
    ResourceAccessScopeKind, SettingsFeatureLifecycle, SettingsFeatureOwnerKind,
    SettingsFeatureRegistration, SettingsFeatureRegistry, APPLICATIONS_CREATE_ACTION_CODE,
    APPLICATIONS_CREATE_OPERATION_ID, APPLICATIONS_DELETE_ACTION_CODE,
    APPLICATIONS_DELETE_OPERATION_ID, APPLICATIONS_RESOURCE_CODE, APPLICATIONS_UPDATE_ACTION_CODE,
    APPLICATIONS_UPDATE_OPERATION_ID, APPLICATIONS_VIEW_ACTION_CODE,
    APPLICATIONS_VIEW_OPERATION_ID, SYSTEM_APPLICATIONS_SETTINGS_FEATURE_ID,
};
use axum::{
    handler::Handler,
    routing::{get, patch, post, MethodRouter},
    Router,
};

use crate::app_state::ApiState;

const CORE_AUTHENTICATED_OPERATION_ID: &str = "core.authenticated";

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

pub fn migrated_core_console_route_assembly() -> ConsoleRouteAssembly<std::sync::Arc<ApiState>> {
    ConsoleRouteAssembly::new()
        .merge(super::session::route_assembly())
        .merge(super::me::route_assembly())
        .merge(super::navigation::route_assembly())
        .merge(super::application_management::route_assembly())
        .merge(super::applications::route_assembly())
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

pub fn compile_migrated_core_console_operation_registry(
    settings_features: &SettingsFeatureRegistry,
    bindings: &[ConsoleRouteAssemblyBinding],
) -> anyhow::Result<ConsoleOperationRegistry> {
    let mounted_operation_ids = bindings
        .iter()
        .filter_map(|binding| match &binding.ownership {
            ConsoleRouteOwnership::Authenticated => None,
            ConsoleRouteOwnership::ConsoleOperation(operation_id) => Some(operation_id.as_str()),
        })
        .collect::<BTreeSet<_>>();
    let migrated_settings = SettingsFeatureRegistry::compile(
        settings_features
            .inventory()
            .features
            .iter()
            .filter(|feature| mounted_operation_ids.contains(feature.permission_code.as_str()))
            .map(|feature| SettingsFeatureRegistration {
                feature_id: feature.feature_id.clone(),
                owner: feature.owner.clone(),
                lifecycle: feature.lifecycle,
                console_surface: feature.console_surface.clone(),
                api_routes: feature.api_routes.clone(),
            }),
    )?;
    let authenticated_routes = bindings
        .iter()
        .filter(|binding| binding.ownership == ConsoleRouteOwnership::Authenticated)
        .map(|binding| binding.route.clone())
        .collect::<Vec<_>>();
    let core_owner = ConsoleOperationOwner {
        kind: SettingsFeatureOwnerKind::Core,
        owner_id: "boot-core".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    };
    let authenticated_operation = ConsoleOperationRegistration {
        operation_id: CORE_AUTHENTICATED_OPERATION_ID.to_string(),
        owner: core_owner.clone(),
        lifecycle: SettingsFeatureLifecycle::Active,
        policy_group: ConsolePolicyGroup::Other("core.authenticated".to_string()),
        label_ref: "console.operations.core_authenticated.label".to_string(),
        description_ref: None,
        order: 0,
        routes: authenticated_routes,
        authorization: ConsoleAuthorization::Authenticated,
    };
    let applications_operations = [
        ConsoleOperationRegistration {
            operation_id: APPLICATIONS_CREATE_OPERATION_ID.to_string(),
            owner: core_owner.clone(),
            lifecycle: SettingsFeatureLifecycle::Active,
            policy_group: ConsolePolicyGroup::SettingsFeature(
                SYSTEM_APPLICATIONS_SETTINGS_FEATURE_ID.to_string(),
            ),
            label_ref: "console.operations.applications.create.label".to_string(),
            description_ref: Some("console.operations.applications.create.description".to_string()),
            order: 100,
            routes: routes_owned_by(bindings, APPLICATIONS_CREATE_OPERATION_ID),
            authorization: ConsoleAuthorization::Simple,
        },
        ConsoleOperationRegistration {
            operation_id: APPLICATIONS_VIEW_OPERATION_ID.to_string(),
            owner: core_owner.clone(),
            lifecycle: SettingsFeatureLifecycle::Active,
            policy_group: ConsolePolicyGroup::SettingsFeature(
                SYSTEM_APPLICATIONS_SETTINGS_FEATURE_ID.to_string(),
            ),
            label_ref: "console.operations.applications.view.label".to_string(),
            description_ref: Some("console.operations.applications.view.description".to_string()),
            order: 110,
            routes: routes_owned_by(bindings, APPLICATIONS_VIEW_OPERATION_ID),
            authorization: ConsoleAuthorization::ResourceAction {
                resource_code: APPLICATIONS_RESOURCE_CODE.to_string(),
                action_code: APPLICATIONS_VIEW_ACTION_CODE.to_string(),
            },
        },
        ConsoleOperationRegistration {
            operation_id: APPLICATIONS_UPDATE_OPERATION_ID.to_string(),
            owner: core_owner.clone(),
            lifecycle: SettingsFeatureLifecycle::Active,
            policy_group: ConsolePolicyGroup::SettingsFeature(
                SYSTEM_APPLICATIONS_SETTINGS_FEATURE_ID.to_string(),
            ),
            label_ref: "console.operations.applications.update.label".to_string(),
            description_ref: Some("console.operations.applications.update.description".to_string()),
            order: 120,
            routes: routes_owned_by(bindings, APPLICATIONS_UPDATE_OPERATION_ID),
            authorization: ConsoleAuthorization::ResourceAction {
                resource_code: APPLICATIONS_RESOURCE_CODE.to_string(),
                action_code: APPLICATIONS_UPDATE_ACTION_CODE.to_string(),
            },
        },
        ConsoleOperationRegistration {
            operation_id: APPLICATIONS_DELETE_OPERATION_ID.to_string(),
            owner: core_owner.clone(),
            lifecycle: SettingsFeatureLifecycle::Active,
            policy_group: ConsolePolicyGroup::SettingsFeature(
                SYSTEM_APPLICATIONS_SETTINGS_FEATURE_ID.to_string(),
            ),
            label_ref: "console.operations.applications.delete.label".to_string(),
            description_ref: Some("console.operations.applications.delete.description".to_string()),
            order: 130,
            routes: routes_owned_by(bindings, APPLICATIONS_DELETE_OPERATION_ID),
            authorization: ConsoleAuthorization::ResourceAction {
                resource_code: APPLICATIONS_RESOURCE_CODE.to_string(),
                action_code: APPLICATIONS_DELETE_ACTION_CODE.to_string(),
            },
        },
    ];
    let applications_resource = ResourceAccessRegistration {
        resource_code: APPLICATIONS_RESOURCE_CODE.to_string(),
        owner: core_owner,
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
    let registry = ConsoleOperationRegistry::compile(
        &migrated_settings,
        std::iter::once(authenticated_operation).chain(applications_operations),
        [applications_resource],
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
