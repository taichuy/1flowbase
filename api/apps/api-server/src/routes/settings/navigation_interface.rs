use std::{collections::BTreeMap, sync::Arc};

use control_plane::ports::RoleRepository;
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;

use super::navigation::ConsoleNavigationResponse;
use crate::{
    console_surface_registry::ConsoleSurfaceRegistry,
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

pub(crate) enum ConsoleNavigationInput {
    Get,
}

impl InterfaceContract for ConsoleNavigationInput {
    const CONTRACT_ID: &'static str = "console-navigation-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum ConsoleNavigationOutput {
    Navigation(ConsoleNavigationResponse),
}

impl InterfaceContract for ConsoleNavigationOutput {
    const CONTRACT_ID: &'static str = "console-navigation-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct ConsoleNavigationDependencies {
    pub(crate) store: MainDurableStore,
    pub(crate) surfaces: Arc<ConsoleSurfaceRegistry>,
    pub(crate) settings_features: Vec<access_control::SettingsFeatureInventoryEntry>,
}

struct ConsoleNavigationAdapter(ConsoleNavigationDependencies);

pub(crate) fn port(
    dependencies: ConsoleNavigationDependencies,
) -> Arc<dyn ConsoleInterfacePort<ConsoleNavigationInput, ConsoleNavigationOutput>> {
    Arc::new(ConsoleNavigationAdapter(dependencies))
}

impl ConsoleNavigationAdapter {
    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: ConsoleNavigationInput,
    ) -> Result<ConsoleNavigationOutput, ApiError> {
        let actor = principal.actor();
        match input {
            ConsoleNavigationInput::Get => {
                let mut navigation = self.0.surfaces.accessible_navigation(actor);
                let stored_order = self
                    .0
                    .store
                    .get_workspace_console_settings_order(actor.current_workspace_id)
                    .await?;
                let mut active_features = self
                    .0
                    .settings_features
                    .iter()
                    .filter(|feature| {
                        feature.lifecycle == access_control::SettingsFeatureLifecycle::Active
                    })
                    .collect::<Vec<_>>();
                active_features.sort_by(|left, right| {
                    left.console_surface
                        .order
                        .cmp(&right.console_surface.order)
                        .then(left.feature_id.cmp(&right.feature_id))
                });
                let active_ids = active_features
                    .iter()
                    .map(|feature| feature.feature_id.as_str())
                    .collect::<std::collections::BTreeSet<_>>();
                let mut ordered_ids = stored_order
                    .group_ids
                    .iter()
                    .filter(|group_id| active_ids.contains(group_id.as_str()))
                    .cloned()
                    .collect::<Vec<_>>();
                let missing_ids = active_features
                    .iter()
                    .map(|feature| feature.feature_id.clone())
                    .filter(|feature_id| !ordered_ids.contains(feature_id))
                    .collect::<Vec<_>>();
                ordered_ids.extend(missing_ids);
                let route_positions = active_features
                    .iter()
                    .filter_map(|feature| {
                        ordered_ids
                            .iter()
                            .position(|feature_id| feature_id == &feature.feature_id)
                            .map(|position| {
                                (feature.console_surface.route_id.as_str(), position as i32)
                            })
                    })
                    .collect::<BTreeMap<_, _>>();
                for item in &mut navigation.navigation_items {
                    if let Some(position) = route_positions.get(item.route_id.as_str()) {
                        item.order = *position;
                    }
                }
                Ok(ConsoleNavigationOutput::Navigation(navigation.into()))
            }
        }
    }
}

impl ConsoleInterfacePort<ConsoleNavigationInput, ConsoleNavigationOutput>
    for ConsoleNavigationAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ConsoleNavigationInput,
    ) -> ConsoleInterfaceFuture<'a, ConsoleNavigationOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[ConsoleInterfaceDeclaration {
    interface_id: "console.navigation.view",
    binding_id: "http.console.navigation.get.v1",
    method: "GET",
    path: "/api/console/navigation",
    mutating: false,
}];

pub(crate) fn compile_registry(
    port: Arc<dyn ConsoleInterfacePort<ConsoleNavigationInput, ConsoleNavigationOutput>>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-navigation",
        "graph:console-navigation-v1",
        DECLARATIONS,
        port,
    )
}

#[cfg(test)]
struct UnavailableConsoleNavigationPort;

#[cfg(test)]
impl ConsoleInterfacePort<ConsoleNavigationInput, ConsoleNavigationOutput>
    for UnavailableConsoleNavigationPort
{
    fn execute<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        _input: ConsoleNavigationInput,
    ) -> ConsoleInterfaceFuture<'a, ConsoleNavigationOutput> {
        Box::pin(async {
            Err(ConsoleInterfaceTargetError(
                anyhow::anyhow!("console navigation fixture unavailable").into(),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f12d_registry_freezes_console_navigation_binding() {
        let registry = compile_registry(Arc::new(UnavailableConsoleNavigationPort)).unwrap();
        for declaration in DECLARATIONS {
            let binding = registry
                .binding(&interface_runtime::BindingId::new(declaration.binding_id).unwrap())
                .expect("declared console navigation binding must be frozen");
            let route = binding.projection().http_route().unwrap();
            assert_eq!(route.method(), declaration.method);
            assert_eq!(route.path(), declaration.path);
        }
        assert_eq!(registry.bindings().count(), DECLARATIONS.len());
    }
}
