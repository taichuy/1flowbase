//! Host-owned replay references are locators, never authorization credentials.
#[cfg(test)]
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::{json, Value};

use super::{
    result_delivery::media,
    virtual_ui::{self, VirtualToolOutcome},
    McpToolArguments,
};
use crate::error_response::ApiError;

pub(super) const READ_TOOL: &str = "mcp_resources_read";
pub(super) const LIST_TOOL: &str = "mcp_resources_list";
#[cfg(test)]
const PREFIX: &str = "flowbase-resource://read/";

#[derive(Clone, Debug, serde::Serialize)]
#[serde(transparent)]
pub(super) struct ResourceReference(pub(super) extension_contracts::ResourceReference);
impl std::ops::Deref for ResourceReference {
    type Target = extension_contracts::ResourceReference;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for ResourceReference {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl ResourceReference {
    pub(super) fn encode(&self) -> Result<String, &'static str> {
        McpToolArguments::from_protocol(self.arguments.clone())
            .map_err(|_| "Resource reference exceeds transport budget")?;
        let uri = self.0.encode()?;
        McpToolArguments::from_protocol(json!({"uri":uri}))
            .map_err(|_| "Resource reference exceeds transport budget")?;
        Ok(uri)
    }
    pub(super) fn decode(uri: &str) -> Result<Self, &'static str> {
        McpToolArguments::from_protocol(json!({"uri":uri}))
            .map_err(|_| "Resource reference exceeds transport budget")?;
        let reference = extension_contracts::ResourceReference::decode(uri)?;
        McpToolArguments::from_protocol(reference.arguments.clone())
            .map_err(|_| "Resource reference exceeds transport budget")?;
        Ok(Self(reference))
    }
}

pub(super) fn list_result() -> Value {
    json!({"resources":[]})
}

pub(super) fn capabilities() -> Value {
    json!({"tools":{},"resources":{}})
}

/// Wrap only embedded resource identities and links to those same identities.
pub(super) fn wrap_content(
    content: &mut Value,
    actor: &domain::ActorContext,
    instance: &domain::McpInstanceRecord,
    tool: &domain::McpToolRecord,
    arguments: &Value,
) -> Result<(), &'static str> {
    if instance.workspace_id != actor.current_workspace_id
        || tool.workspace_id != actor.current_workspace_id
    {
        return Err("Resource not visible");
    }
    let Some(blocks) = content.as_array_mut() else {
        return Err("Invalid native resource content");
    };
    let mut replacements = std::collections::BTreeMap::new();
    for block in blocks.iter().filter(|block| block["type"] == "resource") {
        let original = block
            .pointer("/resource/uri")
            .and_then(Value::as_str)
            .ok_or("Invalid native resource URI")?;
        let reference = ResourceReference(extension_contracts::ResourceReference {
            v: 1,
            user_id: actor.user_id,
            workspace_id: actor.current_workspace_id,
            instance_id: instance.instance_id.clone(),
            tool_id: tool.tool_id.clone(),
            des_id: tool.des_id.clone(),
            arguments: arguments.clone(),
            resource_uri: original.to_owned(),
        });
        replacements.insert(original.to_owned(), reference.encode()?);
    }
    for block in blocks.iter_mut() {
        let path = match block["type"].as_str() {
            Some("resource") => "/resource/uri",
            Some("resource_link") => "/uri",
            _ => continue,
        };
        if let Some(replacement) = block
            .pointer(path)
            .and_then(Value::as_str)
            .and_then(|uri| replacements.get(uri))
        {
            *block
                .pointer_mut(path)
                .ok_or("Invalid native resource URI")? = json!(replacement);
        }
    }
    Ok(())
}

pub(super) fn read_result(
    result: &Value,
    original_uri: &str,
    requested_uri: &str,
) -> Result<Value, &'static str> {
    if result.get("isError").and_then(Value::as_bool) == Some(true) {
        return Err("Resource read failed");
    }
    let blocks = result
        .get("content")
        .and_then(Value::as_array)
        .ok_or("Resource not returned by tool")?;
    let mut contents = Vec::new();
    for block in blocks.iter().filter(|block| {
        block["type"] == "resource"
            && block.pointer("/resource/uri").and_then(Value::as_str) == Some(original_uri)
    }) {
        let mut resource = block["resource"].clone();
        resource["uri"] = json!(requested_uri);
        contents.push(resource);
    }
    if contents.is_empty() {
        return Err("Resource not returned by tool");
    }
    Ok(json!({"contents":contents}))
}

pub(super) async fn read(
    request: virtual_ui::McpCallRequest<'_>,
) -> Result<VirtualToolOutcome, ApiError> {
    let Some(uri) = request.arguments.get("uri").and_then(Value::as_str) else {
        return Ok(VirtualToolOutcome::invalid("Invalid resource URI"));
    };
    let reference = match ResourceReference::decode(uri) {
        Ok(reference) => reference,
        Err(message) => return Ok(VirtualToolOutcome::invalid(message)),
    };
    if let Err(message) =
        authorize_reference(&reference, request.actor, request.catalog, request.scope)
    {
        return Ok(VirtualToolOutcome::invalid(message));
    }
    let arguments = json!({"tool_id":reference.tool_id,"arguments":reference.arguments});
    let outcome = virtual_ui::call_inner(
        virtual_ui::McpCallRequest {
            arguments: &arguments,
            ..request
        },
        Some(uri),
    )
    .await?;
    match outcome {
        VirtualToolOutcome::Success(result) => {
            match read_result(&result, &reference.resource_uri, uri) {
                Ok(result) => Ok(VirtualToolOutcome::Success(result)),
                Err(message) => Ok(VirtualToolOutcome::invalid(message)),
            }
        }
        error => Ok(error),
    }
}

/// Shared currentness/visibility/opt-in authority for protocol reads and Host imports.
fn authorize_reference<'a>(
    reference: &extension_contracts::ResourceReference,
    actor: &domain::ActorContext,
    catalog: &'a domain::McpCatalogSnapshot,
    scope: &virtual_ui::VirtualMcpScope,
) -> Result<(&'a domain::McpInstanceRecord, &'a domain::McpToolRecord), &'static str> {
    if reference.user_id != actor.user_id || reference.workspace_id != actor.current_workspace_id {
        return Err("Resource not visible");
    }
    let Some((instance, tool)) =
        virtual_ui::visible_tool(catalog, scope, &reference.tool_id, false)
    else {
        return Err("Resource not visible");
    };
    if instance.instance_id != reference.instance_id
        || instance.workspace_id != reference.workspace_id
        || tool.workspace_id != reference.workspace_id
        || tool.des_id != reference.des_id
        || !matches!(
            tool.execution_target,
            domain::McpToolExecutionTarget::InterfaceWrapper { .. }
        )
    {
        return Err("Resource reference is no longer current");
    }
    if !matches!(media::mapping(&tool.output_mapping), Ok(Some(mapping)) if mapping.resource_read) {
        return Err("Tool does not support resource reads");
    }
    Ok((instance, tool))
}

/// The parent mutation lock stays owned by its canonical operation. Fail before attempting
/// another serialized operation on that installation; parallel reads retain their normal path.
fn reentrant_serialized_source(
    caller_installation: &str,
    source_installation: &str,
    source_requires_lock: bool,
) -> bool {
    caller_installation == source_installation && source_requires_lock
}

pub(crate) async fn replay_for_import(
    generation: &crate::generation_dispatch::GenerationLease,
    binding: &runtime_core::runtime_backend::FileImportBinding,
    actor: &domain::ActorContext,
    user: domain::UserRecord,
    uri: &str,
) -> Result<
    control_plane::file_management::ImportedResourceFile,
    extension_contracts::FileImportError,
> {
    use extension_contracts::FileImportError;
    let state = &generation.state;
    let dispatch = &generation.owner;
    let reference = ResourceReference::decode(uri)
        .map_err(|_| FileImportError::new("file_import_resource_invalid"))?;
    let mut catalog = control_plane::mcp_management::McpManagementService::new(state.store.clone())
        .read_catalog_for_actor(actor)
        .await
        .map_err(|_| FileImportError::new("file_import_resource_not_visible"))?;
    let ids = binding
        .caller
        .selected_mcp_instance_ids
        .clone()
        .unwrap_or_else(|| {
            catalog
                .instances
                .iter()
                .map(|i| i.instance_id.clone())
                .collect()
        });
    let interfaces =
        crate::routes::mcp_management::mcp_interface_catalog_entries(state.as_ref(), actor)
            .await
            .map_err(|_| FileImportError::new("file_import_resource_read_failed"))?;
    let interface_catalog = virtual_ui::McpInterfaceCatalogSnapshot::new(interfaces);
    // The locator selects a view within the caller's trusted scope; it cannot grant one.
    // Direct console callers may have multiple visible instances, so project the source
    // instance rather than whichever instance happens to appear first in the catalog.
    let source_instances = ids
        .iter()
        .filter(|id| **id == reference.instance_id)
        .cloned()
        .collect::<Vec<_>>();
    crate::managed_service_mcp::append(
        &state.store,
        state.console_surface_registry.managed_services(),
        actor,
        &mut catalog,
        &source_instances,
        &interface_catalog,
    )
    .await
    .map_err(|_| FileImportError::new("file_import_resource_not_visible"))?;
    let scope = virtual_ui::VirtualMcpScope::from_instance_ids(source_instances);
    let (_, tool) = authorize_reference(&reference, actor, &catalog, &scope).map_err(|reason| {
        FileImportError::new(match reason {
            "Resource reference is no longer current" => "file_import_resource_stale",
            "Tool does not support resource reads" => "file_import_resource_read_not_enabled",
            _ => "file_import_resource_not_visible",
        })
    })?;
    let domain::McpToolExecutionTarget::InterfaceWrapper { interface_id } = &tool.execution_target
    else {
        return Err(FileImportError::new("file_import_resource_invalid"));
    };
    if state
        .console_surface_registry
        .managed_services()
        .iter()
        .any(|registration| {
            registration.declaration.operations.iter().any(|operation| {
                operation.interface_id == *interface_id
                    && reentrant_serialized_source(
                        binding.subject.installation_id().as_str(),
                        &registration.installation_id.to_string(),
                        operation.requires_mutation_lock(),
                    )
            })
        })
    {
        return Err(FileImportError::new("file_import_reentrant_source"));
    }
    let dependencies = virtual_ui::RuntimeInternalToolInvokerDependencies::new(
        state.store.clone(),
        state.infrastructure.cache_store(),
        state.provider_secret_master_key.clone(),
    )
    .with_managed_services(state.console_surface_registry.managed_services().to_vec());
    let arguments = json!({"tool_id":reference.tool_id,"arguments":reference.arguments});
    let headers = axum::http::HeaderMap::new();
    let context =
        crate::middleware::require_session::RequestContext::server_delegation(user, actor.clone());
    let result = crate::resource_import::with_replay(
        crate::middleware::require_session::with_server_delegated_request_context(
            context,
            virtual_ui::call_inner(
                virtual_ui::McpCallRequest {
                    dependencies: &dependencies,
                    interface_catalog: &interface_catalog,
                    interface_dispatch: dispatch.as_ref(),
                    headers: &headers,
                    actor,
                    catalog: &catalog,
                    scope: &scope,
                    arguments: &arguments,
                    assistant_client: None,
                },
                Some(uri),
            ),
        ),
    )
    .await
    .map_err(|_| FileImportError::new("file_import_resource_read_failed"))?;
    match result {
        VirtualToolOutcome::Success(result) => imported_file(&result, &reference.resource_uri),
        _ => Err(FileImportError::new("file_import_resource_read_failed")),
    }
}

fn imported_file(
    result: &Value,
    original_uri: &str,
) -> Result<
    control_plane::file_management::ImportedResourceFile,
    extension_contracts::FileImportError,
> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use extension_contracts::FileImportError;
    if result.get("isError").and_then(Value::as_bool) == Some(true) {
        return Err(FileImportError::new("file_import_resource_read_failed"));
    }
    let blocks = result
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| FileImportError::new("file_import_resource_missing"))?;
    let mut matches = blocks.iter().filter(|block| {
        block["type"] == "resource"
            && block.pointer("/resource/uri").and_then(Value::as_str) == Some(original_uri)
    });
    let resource = &matches
        .next()
        .ok_or_else(|| FileImportError::new("file_import_resource_missing"))?["resource"];
    if matches.next().is_some() {
        return Err(FileImportError::new("file_import_resource_ambiguous"));
    }
    if resource.get("text").is_some() {
        return Err(FileImportError::new("file_import_resource_not_binary"));
    }
    let blob = resource
        .get("blob")
        .and_then(Value::as_str)
        .ok_or_else(|| FileImportError::new("file_import_resource_not_binary"))?;
    let bytes = STANDARD
        .decode(blob)
        .map_err(|_| FileImportError::new("file_import_resource_invalid_blob"))?;
    let mime_type = resource
        .get("mimeType")
        .map(|mime| {
            mime.as_str()
                .filter(|mime| media::valid_mime(mime, None))
                .map(str::to_owned)
                .ok_or_else(|| FileImportError::new("file_import_resource_invalid_mime"))
        })
        .transpose()?;
    let names = blocks
        .iter()
        .filter(|b| b["type"] == "resource_link" && b["uri"].as_str() == Some(original_uri))
        .filter_map(|b| b["name"].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    if names.len() > 1 {
        return Err(FileImportError::new(
            "file_import_resource_ambiguous_filename",
        ));
    }
    let filename = match names.first() {
        Some(name) => (*name).to_owned(),
        None => {
            let url = url::Url::parse(original_uri)
                .map_err(|_| FileImportError::new("file_import_resource_filename_missing"))?;
            let name = url
                .path_segments()
                .and_then(|mut segments| segments.rfind(|s| !s.is_empty()))
                .ok_or_else(|| FileImportError::new("file_import_resource_filename_missing"))?;
            percent_encoding::percent_decode_str(name)
                .decode_utf8()
                .map_err(|_| FileImportError::new("file_import_resource_filename_invalid"))?
                .into_owned()
        }
    };
    if filename.is_empty() || filename.chars().any(char::is_control) {
        return Err(FileImportError::new(
            "file_import_resource_filename_invalid",
        ));
    }
    Ok(control_plane::file_management::ImportedResourceFile {
        bytes,
        filename,
        mime_type,
    })
}

#[cfg(test)]
#[path = "_tests/resources.rs"]
mod tests;
