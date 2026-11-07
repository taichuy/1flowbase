//! API event discovery consumes the complete shared API catalog, independently of MCP policy.
use super::api_projection::route_identity;
use crate::{
    error_response::ApiError,
    openapi_interface::{
        OpenApiCapabilityCatalogDependencies, OpenApiCapabilityCatalogEntry,
        OpenApiCapabilitySource,
    },
};
use domain::{event_center::*, ActorContext};
use interface_runtime::{
    CompiledInterfaceRegistry, DocumentOperationDescriptor, InterfaceScope, PrincipalProfile,
};

pub(crate) async fn summaries_with(
    dependencies: &OpenApiCapabilityCatalogDependencies,
    actor: &ActorContext,
) -> Result<Vec<ApiEventOperationSummary>, ApiError> {
    let entries = crate::openapi_interface::build_openapi_capability_catalog_with(
        dependencies,
        actor.current_workspace_id,
    )
    .await?;
    let mut summaries = Vec::with_capacity(entries.len());
    for entry in &entries {
        summaries.push(
            project_current_entry(dependencies, actor, entry, false)
                .await?
                .operation,
        );
    }
    Ok(summaries)
}
pub(crate) async fn get_with(
    dependencies: &OpenApiCapabilityCatalogDependencies,
    actor: &ActorContext,
    operation_id: &str,
) -> Result<Option<ApiEventOperationDetails>, ApiError> {
    let entries = crate::openapi_interface::build_openapi_capability_catalog_with(
        dependencies,
        actor.current_workspace_id,
    )
    .await?;
    let Some(entry) = entries
        .iter()
        .find(|entry| entry.interface.operation_id == operation_id)
    else {
        return Ok(None);
    };
    project_current_entry(dependencies, actor, entry, true)
        .await
        .map(Some)
}
async fn project_current_entry(
    dependencies: &OpenApiCapabilityCatalogDependencies,
    actor: &ActorContext,
    entry: &OpenApiCapabilityCatalogEntry,
    include_schemas: bool,
) -> Result<ApiEventOperationDetails, ApiError> {
    let selected = match entry.source {
        OpenApiCapabilitySource::PublishedWorkflow => crate::routes::application_public_api::workflow_extension_interface::document_descriptor_for_entry(dependencies,actor,entry).await,
        OpenApiCapabilitySource::BuiltinDataModelCrud|OpenApiCapabilitySource::WorkspaceDataModelCrud => crate::routes::runtime_models::event_projection::document_descriptor_for_entry(dependencies,actor,entry).await,
        _=>Ok(None),
    };
    let descriptor = match selected {
        Ok(descriptor) => descriptor,
        Err(error)
            if error
                .0
                .downcast_ref::<control_plane::errors::ControlPlaneError>()
                .is_some_and(|error| {
                    matches!(
                        error,
                        control_plane::errors::ControlPlaneError::PermissionDenied(_)
                    )
                }) =>
        {
            let mut details = project_entry(
                entry,
                dependencies.interface_registry.as_deref(),
                actor,
                None,
                false,
            );
            details.operation.scope = EventSourceScope::Workspace {
                workspace_id: actor.current_workspace_id,
            };
            details.operation.before = unavailable(ApiEventUnavailableReason::PermissionDenied);
            details.operation.completion = unavailable(ApiEventUnavailableReason::PermissionDenied);
            return Ok(details);
        }
        Err(error) => return Err(error),
    };
    Ok(project_entry(
        entry,
        dependencies.interface_registry.as_deref(),
        actor,
        descriptor.as_ref(),
        include_schemas,
    ))
}
fn unavailable(reason: ApiEventUnavailableReason) -> ApiEventPhaseAvailability {
    ApiEventPhaseAvailability::Unavailable { reason }
}
fn reason(code: Option<&str>) -> Option<ApiEventUnavailableReason> {
    code.map(|code| {
        serde_json::from_value(serde_json::Value::String(code.to_owned()))
            .unwrap_or(ApiEventUnavailableReason::TargetUnavailable)
    })
}
/// Dynamic owners supply a descriptor for their actual current authorized snapshot. The descriptor
/// is accepted only against the exact compiled dynamic binding; this never invokes its selector.
pub(crate) fn details_for_entry(
    entry: &OpenApiCapabilityCatalogEntry,
    registry: Option<&CompiledInterfaceRegistry>,
    actor: &ActorContext,
    dynamic: Option<&DocumentOperationDescriptor>,
) -> ApiEventOperationDetails {
    project_entry(entry, registry, actor, dynamic, true)
}
fn project_entry(
    entry: &OpenApiCapabilityCatalogEntry,
    registry: Option<&CompiledInterfaceRegistry>,
    actor: &ActorContext,
    dynamic: Option<&DocumentOperationDescriptor>,
    include_schemas: bool,
) -> ApiEventOperationDetails {
    let api = &entry.interface;
    let mut summary = ApiEventOperationSummary {
        operation_id: api.operation_id.clone(),
        method: api.method.clone(),
        path: api.path.clone(),
        name: api.name.clone(),
        description: api.description.clone(),
        scope: EventSourceScope::Unbound,
        before: unavailable(ApiEventUnavailableReason::MissingBinding),
        completion: unavailable(ApiEventUnavailableReason::MissingBinding),
    };
    let mut request = None;
    let mut result = None;
    let mut rejected = Some(ApiEventUnavailableReason::MissingBinding);
    if entry.disabled_reason == Some("missing_openapi_contract") {
        rejected = Some(ApiEventUnavailableReason::MissingOpenapiContract);
    } else if let Some(registry) = registry {
        let candidates = registry
            .bindings()
            .filter(|binding| {
                if let Some(dynamic) = dynamic {
                    return binding.binding_id() == &dynamic.binding_id;
                }
                binding.projection().http_route().is_some_and(|route| {
                    route_identity(route.method(), route.path())
                        == route_identity(&api.method, &api.path)
                })
            })
            .collect::<Vec<_>>();
        if let Some(binding) = candidates.first() {
            let plan = registry
                .plan(binding.binding_id())
                .expect("compiled binding owns plan");
            summary.scope = match plan.definition().scope() {
                InterfaceScope::System => EventSourceScope::System,
                InterfaceScope::Workspace => EventSourceScope::Workspace {
                    workspace_id: actor.current_workspace_id,
                },
            };
            rejected = if plan.definition().principal_profile() == PrincipalProfile::Public {
                Some(ApiEventUnavailableReason::NoWorkspacePrincipal)
            } else if plan.definition().scope() == InterfaceScope::System && !actor.is_root {
                Some(ApiEventUnavailableReason::PermissionDenied)
            } else {
                None
            };
            if rejected.is_none() {
                let capabilities = candidates
                    .iter()
                    .filter_map(|binding| registry.document_operation(binding.binding_id()))
                    .collect::<Vec<_>>();
                let descriptors = capabilities
                    .iter()
                    .filter_map(|capability| {
                        if capability.dynamic {
                            dynamic.filter(|descriptor| {
                                descriptor.binding_id == *capability.binding_id
                                    && descriptor.interface_identity
                                        == *capability.interface_identity
                                    && !descriptor.target_identity.trim().is_empty()
                                    && !descriptor.target_generation.trim().is_empty()
                                    && descriptor.registration_generation.as_ref()
                                        == capability.registration_generation
                            })
                        } else {
                            capability.static_descriptor
                        }
                    })
                    .filter(|descriptor| descriptor.operation_id.as_ref() == api.operation_id)
                    .collect::<Vec<_>>();
                rejected = if capabilities.len() != candidates.len() || capabilities.is_empty() {
                    Some(ApiEventUnavailableReason::MissingTypedCodec)
                } else if capabilities
                    .iter()
                    .any(|capability| !capability.observer_enabled)
                {
                    Some(ApiEventUnavailableReason::TargetUnavailable)
                } else if descriptors.is_empty() {
                    Some(ApiEventUnavailableReason::TargetUnavailable)
                } else {
                    None
                };
                if rejected.is_none() {
                    // One documented API may have blocking and streaming bindings. Each
                    // binding freezes its own real terminal; the operation schema describes
                    // those supported alternatives without confusing frames with terminals.
                    (summary.before, request) = phase_contract(&descriptors, true, include_schemas);
                    (summary.completion, result) =
                        phase_contract(&descriptors, false, include_schemas);
                }
            }
        }
    }
    if matches!(
        entry.source,
        OpenApiCapabilitySource::BuiltinDataModelCrud
            | OpenApiCapabilitySource::WorkspaceDataModelCrud
            | OpenApiCapabilitySource::PublishedWorkflow
    ) && dynamic.is_none()
    {
        summary.scope = EventSourceScope::Workspace {
            workspace_id: actor.current_workspace_id,
        };
        rejected = Some(ApiEventUnavailableReason::TargetUnavailable);
    }
    if let Some(reason) = rejected {
        summary.before = unavailable(reason);
        summary.completion = unavailable(reason);
        request = None;
        result = None;
    }
    let before_schema = (include_schemas && summary.before.available()).then(|| {
        api_operation_event_schema(
            InterfaceEventPhase::Before,
            request.clone().expect("available request schema"),
            serde_json::Value::Bool(false),
        )
    });
    let completion_schema = (include_schemas && summary.completion.available()).then(|| {
        api_operation_event_schema(
            InterfaceEventPhase::Completion,
            serde_json::Value::Bool(false),
            result.clone().expect("available result schema"),
        )
    });
    ApiEventOperationDetails {
        operation: summary,
        request_schema: request,
        result_schema: result,
        before_schema,
        completion_schema,
    }
}

fn phase_contract(
    descriptors: &[&DocumentOperationDescriptor],
    before: bool,
    include_schema: bool,
) -> (ApiEventPhaseAvailability, Option<serde_json::Value>) {
    let mut schemas = Vec::<&serde_json::Value>::new();
    for descriptor in descriptors {
        let (schema, owner_reason) = if before {
            (
                descriptor.schemas.request.as_ref(),
                descriptor.schemas.request_unavailable.as_deref(),
            )
        } else {
            (
                descriptor.schemas.result.as_ref(),
                descriptor.schemas.result_unavailable.as_deref(),
            )
        };
        if let Some(reason) = reason(owner_reason) {
            return (unavailable(reason), None);
        }
        let Some(schema) = schema.filter(|value| !value.is_null()) else {
            return (
                unavailable(ApiEventUnavailableReason::MissingTypedCodec),
                None,
            );
        };
        if !schemas.contains(&schema) {
            schemas.push(schema);
        }
    }
    if schemas.is_empty() {
        return (
            unavailable(ApiEventUnavailableReason::MissingTypedCodec),
            None,
        );
    }
    let schema = include_schema.then(|| {
        if schemas.len() == 1 {
            schemas[0].clone()
        } else {
            serde_json::json!({"anyOf":schemas})
        }
    });
    (ApiEventPhaseAvailability::Available, schema)
}

/// Gate ordinary documented JSON operations against actual compiled typed codec evidence.
/// Explicit owner restrictions and protocols without a workspace principal remain discoverable.
pub(crate) fn validate_static_inventory(
    registry: &CompiledInterfaceRegistry,
    docs: &crate::openapi_docs::ApiDocsRegistry,
) -> anyhow::Result<()> {
    let mut gaps = Vec::new();
    for category in &docs.catalog().categories {
        let Some(category) = docs.category_operations(&category.id) else {
            continue;
        };
        for operation in &category.operations {
            let bindings = registry
                .bindings()
                .filter(|binding| {
                    binding.projection().http_route().is_some_and(|route| {
                        route_identity(route.method(), route.path())
                            == route_identity(&operation.method, &operation.path)
                    })
                })
                .collect::<Vec<_>>();
            if bindings.is_empty() {
                continue;
            }
            for binding in bindings {
                let plan = registry
                    .plan(binding.binding_id())
                    .expect("compiled binding owns plan");
                if plan.definition().principal_profile() == PrincipalProfile::Public {
                    continue;
                }
                match registry.document_operation(binding.binding_id()) {
                    None => gaps.push(format!(
                        "{} [{}]: missing typed codec",
                        operation.id,
                        binding.binding_id()
                    )),
                    Some(capability) if capability.dynamic => {}
                    Some(capability) => {
                        let descriptor = capability.static_descriptor.expect("static descriptor");
                        if descriptor.operation_id.as_ref() != operation.id
                            || descriptor
                                .schemas
                                .request
                                .as_ref()
                                .is_some_and(serde_json::Value::is_null)
                            || descriptor
                                .schemas
                                .result
                                .as_ref()
                                .is_some_and(serde_json::Value::is_null)
                        {
                            gaps.push(format!(
                                "{} [{}]: unresolved document metadata",
                                operation.id,
                                binding.binding_id()
                            ));
                        }
                    }
                }
            }
        }
    }
    if gaps.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("API event codec inventory incomplete: {}", gaps.join("; "))
    }
}

#[cfg_attr(test, api_test_selection::group(contracts))]
#[cfg(test)]
#[path = "_tests/api_catalog.rs"]
mod tests;
