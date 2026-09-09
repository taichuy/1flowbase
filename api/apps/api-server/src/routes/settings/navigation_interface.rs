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
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![mp::object_schema(&[(
            "variant",
            mp::tag_schema("Get"),
        )])]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Get => {
                mp::object_value(&[("variant", serde_json::Value::String("Get".to_owned()))])
            }
        })
    }

    const CONTRACT_ID: &'static str = "console-navigation-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum ConsoleNavigationOutput {
    Navigation(ConsoleNavigationResponse),
}

impl InterfaceContract for ConsoleNavigationOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![mp::object_schema(&[
            ("variant", mp::tag_schema("Navigation")),
            (
                "0",
                mp::object_schema(&[
                    (
                        "route_definitions",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("route_id",mp::text_schema()), ("surface_key",mp::object_schema(&[("byte_count",mp::count_schema())])), ("path",mp::object_schema(&[("byte_count",mp::count_schema())])), ("surface_kind",mp::object_schema(&[("byte_count",mp::count_schema())]))])}),
                    ),
                    (
                        "navigation_items",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("item_id",mp::text_schema()), ("route_id",mp::text_schema()), ("parent_item_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("label_key",mp::object_schema(&[("byte_count",mp::count_schema())])), ("navigation_slot",mp::object_schema(&[("byte_count",mp::count_schema())])), ("order",serde_json::json!({"type":"integer"}))])}),
                    ),
                    (
                        "permission_bindings",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("binding_id",mp::text_schema()), ("route_id",mp::text_schema()), ("permission_codes",mp::object_schema(&[("item_count",mp::count_schema())])), ("requirement",mp::object_schema(&[("byte_count",mp::count_schema())]))])}),
                    ),
                ]),
            ),
        ])]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Navigation(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("Navigation".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("route_definitions", {
                            if (&(_field_0).route_definitions).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).route_definitions)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("route_id", mp::text(&(item).route_id)?),
                                            (
                                                "surface_key",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).surface_key).len()),
                                                )]),
                                            ),
                                            (
                                                "path",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).path).len()),
                                                )]),
                                            ),
                                            (
                                                "surface_kind",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).surface_kind).len()),
                                                )]),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("navigation_items", {
                            if (&(_field_0).navigation_items).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).navigation_items)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("item_id", mp::text(&(item).item_id)?),
                                            ("route_id", mp::text(&(item).route_id)?),
                                            (
                                                "parent_item_id",
                                                match (&(item).parent_item_id).as_ref() {
                                                    Some(item) => mp::text(item)?,
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "label_key",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).label_key).len()),
                                                )]),
                                            ),
                                            (
                                                "navigation_slot",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!(
                                                        (&(item).navigation_slot).len()
                                                    ),
                                                )]),
                                            ),
                                            ("order", serde_json::json!(*(&(item).order))),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("permission_bindings", {
                            if (&(_field_0).permission_bindings).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).permission_bindings)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("binding_id", mp::text(&(item).binding_id)?),
                                            ("route_id", mp::text(&(item).route_id)?),
                                            (
                                                "permission_codes",
                                                mp::object_value(&[(
                                                    "item_count",
                                                    serde_json::json!(
                                                        (&(item).permission_codes).len()
                                                    ),
                                                )]),
                                            ),
                                            (
                                                "requirement",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).requirement).len()),
                                                )]),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                    ]),
                ),
            ]),
        })
    }

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
