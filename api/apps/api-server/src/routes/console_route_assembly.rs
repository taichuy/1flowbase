use std::{collections::{BTreeMap, BTreeSet}, convert::Infallible, sync::Arc};

use access_control::{
    ConsoleAuthorization, ConsoleOperationOwner, ConsoleOperationRegistration,
    ConsoleOperationRegistry, ConsolePolicyGroup, ConsoleRouteAssemblyBinding, ConsoleRouteBinding,
    ConsoleRouteOwnership, ResourceAccessAction, ResourceAccessRegistration,
    ResourceAccessScopeKind, SettingsFeatureLifecycle, SettingsFeatureOwnerKind,
    SettingsFeatureRegistry,
    APPLICATIONS_API_SET_ENABLED_OPERATION_ID, APPLICATIONS_CREATE_ACTION_CODE,
    APPLICATIONS_CREATE_OPERATION_ID, APPLICATIONS_DELETE_ACTION_CODE,
    APPLICATIONS_DELETE_OPERATION_ID, APPLICATIONS_LOGS_EXPORT_OPERATION_ID,
    APPLICATIONS_LOGS_IMPORT_OPERATION_ID, APPLICATIONS_ORCHESTRATION_TEMPLATE_EXPORT_OPERATION_ID,
    APPLICATIONS_ORCHESTRATION_TEMPLATE_IMPORT_OPERATION_ID,
    APPLICATIONS_ORCHESTRATION_VERSION_RESTORE_OPERATION_ID, APPLICATIONS_PUBLISH_OPERATION_ID,
    APPLICATIONS_RESOURCE_CODE, APPLICATIONS_RUN_OPERATION_ID, APPLICATIONS_UPDATE_ACTION_CODE,
    APPLICATIONS_UPDATE_OPERATION_ID, APPLICATIONS_VIEW_ACTION_CODE,
    APPLICATIONS_VIEW_OPERATION_ID, DATA_SOURCES_CREATE_OPERATION_ID,
    DATA_SOURCES_DEFAULTS_UPDATE_OPERATION_ID, DATA_SOURCES_DISCOVER_OPERATION_ID,
    DATA_SOURCES_LIST_OPERATION_ID, DATA_SOURCES_MAP_TO_MODEL_OPERATION_ID,
    DATA_SOURCES_PREVIEW_OPERATION_ID, DATA_SOURCES_SECRET_ROTATE_OPERATION_ID,
    DATA_SOURCES_VALIDATE_OPERATION_ID, DATA_SOURCES_VIEW_ACTION_CODE,
    DATA_SOURCES_VIEW_OPERATION_ID, DATA_SOURCE_INSTANCES_RESOURCE_CODE,
    FILES_CONTENT_DOWNLOAD_OPERATION_ID, FILES_UPLOAD_OPERATION_ID,
    FILE_STORAGES_CREATE_OPERATION_ID, FILE_STORAGES_DELETE_OPERATION_ID,
    FILE_STORAGES_LIST_OPERATION_ID, FILE_STORAGES_UPDATE_OPERATION_ID,
    FILE_TABLES_CREATE_OPERATION_ID, FILE_TABLES_DELETE_OPERATION_ID,
    FILE_TABLES_LIST_OPERATION_ID, FILE_TABLES_STORAGE_BIND_OPERATION_ID,
    MODEL_DEFINITIONS_ADVISOR_VIEW_OPERATION_ID, MODEL_DEFINITIONS_CREATE_OPERATION_ID,
    MODEL_DEFINITIONS_DELETE_OPERATION_ID, MODEL_DEFINITIONS_LIST_OPERATION_ID,
    MODEL_DEFINITIONS_OPENAPI_VIEW_OPERATION_ID, MODEL_DEFINITIONS_UPDATE_OPERATION_ID,
    MODEL_FIELDS_CREATE_OPERATION_ID, MODEL_FIELDS_DELETE_OPERATION_ID,
    MODEL_FIELDS_UPDATE_OPERATION_ID, MODEL_SCOPE_GRANTS_CREATE_OPERATION_ID,
    MODEL_SCOPE_GRANTS_LIST_OPERATION_ID, MODEL_SCOPE_GRANTS_UPDATE_OPERATION_ID,
    SYSTEM_APPLICATIONS_SETTINGS_FEATURE_ID, SYSTEM_DATA_MODELS_SETTINGS_FEATURE_ID,
    SYSTEM_FILES_SETTINGS_FEATURE_ID,
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

fn frontstage_route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::Authenticated;

    let bindings = [
        ("GET", "/api/console/frontstage/:workspace_id/pages"),
        ("POST", "/api/console/frontstage/:workspace_id/pages"),
        ("POST", "/api/console/frontstage/:workspace_id/pages/groups"),
        ("PATCH", "/api/console/frontstage/:workspace_id/pages/:page_id"),
        ("DELETE", "/api/console/frontstage/:workspace_id/pages/:page_id"),
        (
            "POST",
            "/api/console/frontstage/:workspace_id/pages/:page_id/move",
        ),
        (
            "GET",
            "/api/console/frontstage/:workspace_id/pages/:page_id/tabs",
        ),
        (
            "POST",
            "/api/console/frontstage/:workspace_id/pages/:page_id/tabs",
        ),
        (
            "GET",
            "/api/console/frontstage/:workspace_id/pages/:page_id/tabs/:tab_id",
        ),
        (
            "PATCH",
            "/api/console/frontstage/:workspace_id/pages/:page_id/tabs/:tab_id",
        ),
        (
            "DELETE",
            "/api/console/frontstage/:workspace_id/pages/:page_id/tabs/:tab_id",
        ),
        (
            "PUT",
            "/api/console/frontstage/:workspace_id/pages/:page_id/tabs/:tab_id/document",
        ),
        (
            "POST",
            "/api/console/frontstage/:workspace_id/pages/:page_id/tabs/:tab_id/queries/dispatch",
        ),
        (
            "POST",
            "/api/console/frontstage/:workspace_id/pages/:page_id/tabs/:tab_id/actions/dispatch",
        ),
        (
            "GET",
            "/api/console/frontstage/:workspace_id/pages/:page_id/block-codes/:code_ref",
        ),
        (
            "PUT",
            "/api/console/frontstage/:workspace_id/pages/:page_id/block-codes/:code_ref",
        ),
    ]
    .into_iter()
    .map(|(method, path)| ConsoleRouteAssemblyBinding {
        route: ConsoleRouteBinding {
            method: method.to_string(),
            path: path.to_string(),
        },
        ownership: Authenticated,
    })
    .collect();

    ConsoleRouteAssembly {
        router: super::frontstage::router(),
        bindings,
    }
}

pub fn migrated_core_console_route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    ConsoleRouteAssembly::new()
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
        .merge(frontstage_route_assembly())
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

fn generic_operation_policy_group(
    settings_features: &SettingsFeatureRegistry,
    operation_id: &str,
    routes: &[ConsoleRouteBinding],
) -> anyhow::Result<ConsolePolicyGroup> {
    if let Some(feature_id) = operation_id.strip_prefix("settings_feature.access.") {
        let feature = settings_features
            .inventory()
            .features
            .iter()
            .find(|feature| feature.feature_id == feature_id)
            .ok_or_else(|| anyhow::anyhow!("settings feature operation owner is not registered: {operation_id}"))?;
        if !routes.iter().any(|route| {
            feature.api_routes.iter().any(|registered_route| {
                registered_route.method.eq_ignore_ascii_case(&route.method)
                    && route_templates_match(&route.path, &registered_route.path)
            })
        }) {
            anyhow::bail!(
                "settings feature operation has no route in its feature registration: {operation_id}"
            );
        }
        return Ok(ConsolePolicyGroup::SettingsFeature(feature_id.to_string()));
    }

    let matching_features = settings_features
        .inventory()
        .features
        .iter()
        .filter(|feature| {
            routes.iter().any(|route| {
                feature.api_routes.iter().any(|registered_route| {
                    registered_route.method.eq_ignore_ascii_case(&route.method)
                        && route_templates_match(&route.path, &registered_route.path)
                })
            })
        })
        .map(|feature| feature.feature_id.as_str())
        .collect::<BTreeSet<_>>();

    match matching_features.len() {
        0 => {
            let namespace = operation_id
                .split('.')
                .next()
                .filter(|namespace| !namespace.is_empty())
                .unwrap_or("core")
                .replace('_', "-");
            Ok(ConsolePolicyGroup::Other(format!("other.{namespace}")))
        }
        1 => Ok(ConsolePolicyGroup::SettingsFeature(
            matching_features
                .iter()
                .next()
                .expect("one matching feature must have an id")
                .to_string(),
        )),
        _ => anyhow::bail!(
            "console operation has ambiguous SettingsFeature ownership: {operation_id}"
        ),
    }
}

pub fn compile_migrated_core_console_operation_registry(
    settings_features: &SettingsFeatureRegistry,
    bindings: &[ConsoleRouteAssemblyBinding],
) -> anyhow::Result<ConsoleOperationRegistry> {
    validate_settings_feature_route_assembly(settings_features, bindings)?;
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
    let applications_simple_operations = [
        (APPLICATIONS_PUBLISH_OPERATION_ID, 140),
        (APPLICATIONS_API_SET_ENABLED_OPERATION_ID, 150),
        (APPLICATIONS_ORCHESTRATION_TEMPLATE_EXPORT_OPERATION_ID, 160),
        (APPLICATIONS_ORCHESTRATION_TEMPLATE_IMPORT_OPERATION_ID, 170),
        (APPLICATIONS_ORCHESTRATION_VERSION_RESTORE_OPERATION_ID, 180),
        (APPLICATIONS_RUN_OPERATION_ID, 190),
        (APPLICATIONS_LOGS_EXPORT_OPERATION_ID, 200),
        (APPLICATIONS_LOGS_IMPORT_OPERATION_ID, 210),
    ]
    .into_iter()
    .map(|(operation_id, order)| ConsoleOperationRegistration {
        operation_id: operation_id.to_string(),
        owner: core_owner.clone(),
        lifecycle: SettingsFeatureLifecycle::Active,
        policy_group: ConsolePolicyGroup::SettingsFeature(
            SYSTEM_APPLICATIONS_SETTINGS_FEATURE_ID.to_string(),
        ),
        label_ref: format!("console.operations.{operation_id}.label"),
        description_ref: Some(format!("console.operations.{operation_id}.description")),
        order,
        routes: routes_owned_by(bindings, operation_id),
        authorization: ConsoleAuthorization::Simple,
    });
    let data_model_simple_operations = [
        (DATA_SOURCES_LIST_OPERATION_ID, 300),
        (DATA_SOURCES_CREATE_OPERATION_ID, 310),
        (DATA_SOURCES_DEFAULTS_UPDATE_OPERATION_ID, 320),
        (DATA_SOURCES_VALIDATE_OPERATION_ID, 330),
        (DATA_SOURCES_DISCOVER_OPERATION_ID, 340),
        (DATA_SOURCES_PREVIEW_OPERATION_ID, 350),
        (DATA_SOURCES_MAP_TO_MODEL_OPERATION_ID, 360),
        (MODEL_DEFINITIONS_LIST_OPERATION_ID, 370),
        (MODEL_DEFINITIONS_CREATE_OPERATION_ID, 380),
        (MODEL_DEFINITIONS_UPDATE_OPERATION_ID, 390),
        (MODEL_DEFINITIONS_DELETE_OPERATION_ID, 400),
        (MODEL_DEFINITIONS_ADVISOR_VIEW_OPERATION_ID, 410),
        (MODEL_FIELDS_CREATE_OPERATION_ID, 420),
        (MODEL_FIELDS_UPDATE_OPERATION_ID, 430),
        (MODEL_FIELDS_DELETE_OPERATION_ID, 440),
        (MODEL_SCOPE_GRANTS_LIST_OPERATION_ID, 450),
        (MODEL_SCOPE_GRANTS_CREATE_OPERATION_ID, 460),
        (MODEL_SCOPE_GRANTS_UPDATE_OPERATION_ID, 470),
        (MODEL_DEFINITIONS_OPENAPI_VIEW_OPERATION_ID, 480),
    ]
    .into_iter()
    .map(|(operation_id, order)| ConsoleOperationRegistration {
        operation_id: operation_id.to_string(),
        owner: core_owner.clone(),
        lifecycle: SettingsFeatureLifecycle::Active,
        policy_group: ConsolePolicyGroup::SettingsFeature(
            SYSTEM_DATA_MODELS_SETTINGS_FEATURE_ID.to_string(),
        ),
        label_ref: format!("console.operations.{operation_id}.label"),
        description_ref: Some(format!("console.operations.{operation_id}.description")),
        order,
        routes: routes_owned_by(bindings, operation_id),
        authorization: ConsoleAuthorization::Simple,
    });
    let data_sources_view_operation = ConsoleOperationRegistration {
        operation_id: DATA_SOURCES_VIEW_OPERATION_ID.to_string(),
        owner: core_owner.clone(),
        lifecycle: SettingsFeatureLifecycle::Active,
        policy_group: ConsolePolicyGroup::SettingsFeature(
            SYSTEM_DATA_MODELS_SETTINGS_FEATURE_ID.to_string(),
        ),
        label_ref: "console.operations.data_sources.view.label".to_string(),
        description_ref: Some("console.operations.data_sources.view.description".to_string()),
        order: 490,
        routes: routes_owned_by(bindings, DATA_SOURCES_VIEW_OPERATION_ID),
        authorization: ConsoleAuthorization::ResourceAction {
            resource_code: DATA_SOURCE_INSTANCES_RESOURCE_CODE.to_string(),
            action_code: DATA_SOURCES_VIEW_ACTION_CODE.to_string(),
        },
    };
    let data_source_secret_rotate_operation = ConsoleOperationRegistration {
        operation_id: DATA_SOURCES_SECRET_ROTATE_OPERATION_ID.to_string(),
        owner: core_owner.clone(),
        lifecycle: SettingsFeatureLifecycle::Active,
        policy_group: ConsolePolicyGroup::Other("other.data-sources".to_string()),
        label_ref: "console.operations.data_sources.secret.rotate.label".to_string(),
        description_ref: Some(
            "console.operations.data_sources.secret.rotate.description".to_string(),
        ),
        order: 500,
        routes: routes_owned_by(bindings, DATA_SOURCES_SECRET_ROTATE_OPERATION_ID),
        authorization: ConsoleAuthorization::Simple,
    };
    let file_settings_simple_operations = [
        (FILE_STORAGES_LIST_OPERATION_ID, 600),
        (FILE_STORAGES_CREATE_OPERATION_ID, 610),
        (FILE_STORAGES_UPDATE_OPERATION_ID, 620),
        (FILE_STORAGES_DELETE_OPERATION_ID, 630),
        (FILE_TABLES_LIST_OPERATION_ID, 640),
        (FILE_TABLES_CREATE_OPERATION_ID, 650),
        (FILE_TABLES_STORAGE_BIND_OPERATION_ID, 660),
        (FILE_TABLES_DELETE_OPERATION_ID, 670),
    ]
    .into_iter()
    .map(|(operation_id, order)| ConsoleOperationRegistration {
        operation_id: operation_id.to_string(),
        owner: core_owner.clone(),
        lifecycle: SettingsFeatureLifecycle::Active,
        policy_group: ConsolePolicyGroup::SettingsFeature(
            SYSTEM_FILES_SETTINGS_FEATURE_ID.to_string(),
        ),
        label_ref: format!("console.operations.{operation_id}.label"),
        description_ref: Some(format!("console.operations.{operation_id}.description")),
        order,
        routes: routes_owned_by(bindings, operation_id),
        authorization: ConsoleAuthorization::Simple,
    });
    let file_other_simple_operations = [
        (FILES_UPLOAD_OPERATION_ID, 680),
        (FILES_CONTENT_DOWNLOAD_OPERATION_ID, 690),
    ]
    .into_iter()
    .map(|(operation_id, order)| ConsoleOperationRegistration {
        operation_id: operation_id.to_string(),
        owner: core_owner.clone(),
        lifecycle: SettingsFeatureLifecycle::Active,
        policy_group: ConsolePolicyGroup::Other("other.files".to_string()),
        label_ref: format!("console.operations.{operation_id}.label"),
        description_ref: Some(format!("console.operations.{operation_id}.description")),
        order,
        routes: routes_owned_by(bindings, operation_id),
        authorization: ConsoleAuthorization::Simple,
    });
    let known_operation_ids = BTreeSet::from([
        CORE_AUTHENTICATED_OPERATION_ID,
        APPLICATIONS_CREATE_OPERATION_ID,
        APPLICATIONS_VIEW_OPERATION_ID,
        APPLICATIONS_UPDATE_OPERATION_ID,
        APPLICATIONS_DELETE_OPERATION_ID,
        APPLICATIONS_PUBLISH_OPERATION_ID,
        APPLICATIONS_API_SET_ENABLED_OPERATION_ID,
        APPLICATIONS_ORCHESTRATION_TEMPLATE_EXPORT_OPERATION_ID,
        APPLICATIONS_ORCHESTRATION_TEMPLATE_IMPORT_OPERATION_ID,
        APPLICATIONS_ORCHESTRATION_VERSION_RESTORE_OPERATION_ID,
        APPLICATIONS_RUN_OPERATION_ID,
        APPLICATIONS_LOGS_EXPORT_OPERATION_ID,
        APPLICATIONS_LOGS_IMPORT_OPERATION_ID,
        DATA_SOURCES_LIST_OPERATION_ID,
        DATA_SOURCES_CREATE_OPERATION_ID,
        DATA_SOURCES_DEFAULTS_UPDATE_OPERATION_ID,
        DATA_SOURCES_VALIDATE_OPERATION_ID,
        DATA_SOURCES_DISCOVER_OPERATION_ID,
        DATA_SOURCES_PREVIEW_OPERATION_ID,
        DATA_SOURCES_MAP_TO_MODEL_OPERATION_ID,
        DATA_SOURCES_VIEW_OPERATION_ID,
        DATA_SOURCES_SECRET_ROTATE_OPERATION_ID,
        MODEL_DEFINITIONS_LIST_OPERATION_ID,
        MODEL_DEFINITIONS_CREATE_OPERATION_ID,
        MODEL_DEFINITIONS_UPDATE_OPERATION_ID,
        MODEL_DEFINITIONS_DELETE_OPERATION_ID,
        MODEL_DEFINITIONS_ADVISOR_VIEW_OPERATION_ID,
        MODEL_DEFINITIONS_OPENAPI_VIEW_OPERATION_ID,
        MODEL_FIELDS_CREATE_OPERATION_ID,
        MODEL_FIELDS_UPDATE_OPERATION_ID,
        MODEL_FIELDS_DELETE_OPERATION_ID,
        MODEL_SCOPE_GRANTS_LIST_OPERATION_ID,
        MODEL_SCOPE_GRANTS_CREATE_OPERATION_ID,
        MODEL_SCOPE_GRANTS_UPDATE_OPERATION_ID,
        FILES_UPLOAD_OPERATION_ID,
        FILES_CONTENT_DOWNLOAD_OPERATION_ID,
        FILE_STORAGES_LIST_OPERATION_ID,
        FILE_STORAGES_CREATE_OPERATION_ID,
        FILE_STORAGES_UPDATE_OPERATION_ID,
        FILE_STORAGES_DELETE_OPERATION_ID,
        FILE_TABLES_LIST_OPERATION_ID,
        FILE_TABLES_CREATE_OPERATION_ID,
        FILE_TABLES_STORAGE_BIND_OPERATION_ID,
        FILE_TABLES_DELETE_OPERATION_ID,
    ]);
    let mut generic_operation_routes = BTreeMap::<String, Vec<ConsoleRouteBinding>>::new();
    for binding in bindings {
        let ConsoleRouteOwnership::ConsoleOperation(operation_id) = &binding.ownership else {
            continue;
        };
        if known_operation_ids.contains(operation_id.as_str()) {
            continue;
        }
        generic_operation_routes
            .entry(operation_id.clone())
            .or_default()
            .push(binding.route.clone());
    }
    let generic_operations = generic_operation_routes
        .into_iter()
        .enumerate()
        .map(|(index, (operation_id, mut routes))| {
            routes.sort();
            let policy_group = generic_operation_policy_group(
                settings_features,
                &operation_id,
                &routes,
            )?;
            let (label_ref, description_ref) = match &policy_group {
                ConsolePolicyGroup::SettingsFeature(feature_id) => {
                    let feature = settings_features
                        .inventory()
                        .features
                        .iter()
                        .find(|feature| feature.feature_id == *feature_id)
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "compiled operation references missing SettingsFeature: {feature_id}"
                            )
                        })?;
                    (feature.console_surface.label_key.clone(), None)
                }
                ConsolePolicyGroup::Other(_) => (
                    format!("console.operations.{operation_id}.label"),
                    Some(format!("console.operations.{operation_id}.description")),
                ),
            };
            Ok(ConsoleOperationRegistration {
                operation_id,
                owner: core_owner.clone(),
                lifecycle: SettingsFeatureLifecycle::Active,
                policy_group,
                label_ref,
                description_ref,
                order: 1_000 + index as i32,
                routes,
                authorization: ConsoleAuthorization::Simple,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
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
    let registry = ConsoleOperationRegistry::compile(
        settings_features,
        std::iter::once(authenticated_operation)
            .chain(applications_operations)
            .chain(applications_simple_operations)
            .chain(data_model_simple_operations)
            .chain(std::iter::once(data_sources_view_operation))
            .chain(std::iter::once(data_source_secret_rotate_operation))
            .chain(file_settings_simple_operations)
            .chain(file_other_simple_operations)
            .chain(generic_operations),
        [applications_resource, data_source_instances_resource],
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

    use super::{migrated_core_console_route_assembly, ConsoleRouteAssembly};

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
