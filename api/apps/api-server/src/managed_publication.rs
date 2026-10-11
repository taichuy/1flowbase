//! The ingress selects one complete host generation. Candidate construction never mutates the
//! serving router, and publication contains no fallible work or await point.
use std::{
    collections::BTreeMap,
    panic::AssertUnwindSafe,
    sync::{Arc, RwLock},
};

use axum::{extract::Request, Router};
use futures_util::FutureExt;
use plugin_framework::HostExtensionContributionManifest;
use tower::ServiceExt;
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use uuid::Uuid;

use crate::{
    app_state::{compile_console_boot_plan_with_managed_services, ApiState},
    config::{ApiConfig, ApiEnvironment},
    extension_bus::ManagedWorkspaceSnapshot,
    host_extensions::console::{
        linked_host_console_route_sources, resolve_linked_host_extension_console_contribution,
    },
    managed_services::ManagedServiceRegistration,
};

pub(crate) struct PreparedGeneration {
    router: Router,
    pub(crate) permission_catalog: control_plane_contracts::CompiledConsolePolicyCatalog,
}

pub(crate) struct ManagedGenerationPublisher {
    current: RwLock<Router>,
    native_interfaces: Vec<String>,
    // This template has an empty, independent boot registry: it must not retain the first
    // generation's managed snapshots after that generation's last request completes.
    template: ApiState,
    config: ApiConfig,
    host_contributions: Vec<HostExtensionContributionManifest>,
}

impl ManagedGenerationPublisher {
    pub(crate) fn new(
        state: &Arc<ApiState>,
        config: ApiConfig,
        host_contributions: Vec<HostExtensionContributionManifest>,
        initial_router: Router,
    ) -> anyhow::Result<Arc<Self>> {
        let mut template = state.as_ref().clone();
        template.extension_boot_snapshot = Some(Arc::new(
            state
                .extension_boot_snapshot
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("host generation requires a boot snapshot"))?
                .fork_generation(BTreeMap::new())?,
        ));
        let managed_interfaces = state
            .console_surface_registry
            .managed_services()
            .iter()
            .flat_map(|service| {
                service
                    .declaration
                    .operations
                    .iter()
                    .map(|op| op.interface_id.as_str())
            })
            .collect::<std::collections::BTreeSet<_>>();
        let native_interfaces = state
            .extension_boot_snapshot
            .as_ref()
            .unwrap()
            .interface_registry()
            .unwrap()
            .snapshot()
            .definitions()
            .filter(|definition| !managed_interfaces.contains(definition.interface_id().as_str()))
            .map(|definition| definition.interface_id().as_str().to_owned())
            .collect();
        Ok(Arc::new(Self {
            native_interfaces,
            current: RwLock::new(initial_router),
            template,
            config,
            host_contributions,
        }))
    }

    pub(crate) fn interface_module(
        &self,
        registrations: &[ManagedServiceRegistration],
    ) -> anyhow::Result<plugin_framework::extension_bus::ModuleDescriptor> {
        let ids = self
            .native_interfaces
            .iter()
            .map(String::as_str)
            .chain(registrations.iter().flat_map(|service| {
                service
                    .declaration
                    .operations
                    .iter()
                    .map(|op| op.interface_id.as_str())
            }))
            .collect::<std::collections::BTreeSet<_>>();
        Ok(plugin_framework::extension_bus::compile_managed_interface_module(ids)?)
    }

    pub(crate) fn ingress(self: &Arc<Self>) -> Router {
        let publisher = self.clone();
        Router::new().fallback_service(tower::service_fn(move |request: Request| {
            // The guard is dropped synchronously before dispatch. Even requests suspended in
            // middleware, streaming responses and protocol upgrades retain their selected router.
            let router = publisher
                .current
                .read()
                .unwrap_or_else(|poison| poison.into_inner())
                .clone();
            async move { router.oneshot(request).await }
        }))
    }

    pub(crate) async fn prepare(
        &self,
        registrations: Vec<ManagedServiceRegistration>,
        snapshots: BTreeMap<Uuid, Arc<ManagedWorkspaceSnapshot>>,
    ) -> anyhow::Result<PreparedGeneration> {
        // Axum rejects overlapping route declarations with a panic. Treat that as candidate
        // validation failure so package activation can roll back without disturbing ingress.
        AssertUnwindSafe(self.prepare_inner(registrations, snapshots))
            .catch_unwind()
            .await
            .map_err(|_| anyhow::anyhow!("host generation route assembly rejected candidate"))?
    }

    async fn prepare_inner(
        &self,
        registrations: Vec<ManagedServiceRegistration>,
        snapshots: BTreeMap<Uuid, Arc<ManagedWorkspaceSnapshot>>,
    ) -> anyhow::Result<PreparedGeneration> {
        let boot = Arc::new(
            self.template
                .extension_boot_snapshot
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("host generation template has no boot snapshot"))?
                .fork_generation(snapshots)?,
        );
        let host_extensions = self
            .host_contributions
            .iter()
            .cloned()
            .map(|contribution| {
                resolve_linked_host_extension_console_contribution(
                    contribution,
                    linked_host_console_route_sources(),
                )
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let registry = boot
            .interface_registry()
            .ok_or_else(|| anyhow::anyhow!("host generation has no interface registry"))?
            .snapshot();
        let plan = compile_console_boot_plan_with_managed_services(
            host_extensions,
            Some(registry.as_ref()),
            self.config.plugin_upload_max_bytes,
            registrations,
        )?;
        let mut state = self.template.clone();
        state.settings_feature_registry = plan.settings_feature_registry;
        state.console_operation_registry = plan.console_operation_registry;
        state.console_surface_registry = Arc::new(
            plan.console_surface_registry
                .as_ref()
                .clone()
                .with_native_targets(
                    self.template
                        .console_surface_registry
                        .native_targets()
                        .to_vec(),
                ),
        );
        state.extension_boot_snapshot = Some(boot.clone());
        let mut docs_document = serde_json::to_value(crate::openapi::ApiDoc::openapi())?;
        crate::managed_services::append_openapi(
            &mut docs_document,
            state.console_surface_registry.managed_services(),
        );
        state.api_docs = Arc::new(
            crate::openapi_docs::build_api_docs_registry_with_cookie_name(
                docs_document,
                &self.config.cookie_name,
            )?,
        );
        let state = Arc::new(state);
        boot.publish_complete_catalog(&state)?;
        let document = crate::openapi::dynamic_openapi_document(&state)
            .await
            .map_err(|error| error.0)?;
        let include_docs = self.config.env != ApiEnvironment::Production;
        let (router, mounted) =
            crate::console_router_with_assembly(state.clone(), include_docs, plan.route_assembly);
        crate::publish_external_endpoint_catalog(&state, &mounted, include_docs, &document)?;
        let permission_catalog =
            control_plane::role::console_policy_migration::compiled_catalog_from_inventory(
                state.console_operation_registry.inventory(),
            )?;
        Ok(PreparedGeneration {
            permission_catalog,
            router: router
                .layer(crate::cors_layer(&self.config))
                .layer(TraceLayer::new_for_http()),
        })
    }

    pub(crate) fn publish(&self, prepared: PreparedGeneration) {
        let previous = {
            let mut current = self
                .current
                .write()
                .unwrap_or_else(|poison| poison.into_inner());
            std::mem::replace(&mut *current, prepared.router)
        };
        // Release the old router outside the publication lock; active requests own their clones.
        drop(previous);
    }
}
