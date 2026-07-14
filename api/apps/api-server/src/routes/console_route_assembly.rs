use std::{collections::BTreeSet, convert::Infallible};

use access_control::{
    ConsoleAuthorization, ConsoleOperationOwner, ConsoleOperationRegistration,
    ConsoleOperationRegistry, ConsolePolicyGroup, ConsoleRouteAssemblyBinding, ConsoleRouteBinding,
    ConsoleRouteOwnership, SettingsFeatureLifecycle, SettingsFeatureOwnerKind,
    SettingsFeatureRegistration, SettingsFeatureRegistry,
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
    let authenticated_operation = ConsoleOperationRegistration {
        operation_id: CORE_AUTHENTICATED_OPERATION_ID.to_string(),
        owner: ConsoleOperationOwner {
            kind: SettingsFeatureOwnerKind::Core,
            owner_id: "boot-core".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
        lifecycle: SettingsFeatureLifecycle::Active,
        policy_group: ConsolePolicyGroup::Other("core.authenticated".to_string()),
        label_ref: "console.operations.core_authenticated.label".to_string(),
        description_ref: None,
        order: 0,
        routes: authenticated_routes,
        authorization: ConsoleAuthorization::Authenticated,
    };
    let registry =
        ConsoleOperationRegistry::compile(&migrated_settings, [authenticated_operation], [])?;
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
