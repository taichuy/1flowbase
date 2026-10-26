//! Real MCP records are materialized from plugin declarations; discovery stays progressive.
use super::interface::Dependencies;
use crate::error_response::ApiError;
use control_plane::mcp_management::{
    CreateMcpInstanceCommand, CreateMcpToolBindingCommand, CreateMcpToolCommand,
    McpManagementService,
};
use domain::{ActorContext, McpInstanceStatus, McpToolStatus, WebMcpExposure};
use extension_contracts::NotificationMcpOperation;
use serde_json::{json, Value};
use uuid::Uuid;

pub(crate) fn instance_id(channel: Uuid) -> String {
    format!("communication_{channel}")
}
pub(crate) async fn ensure_channel_mcp(
    deps: &Dependencies,
    actor: &ActorContext,
    channel: Uuid,
) -> Result<Option<String>, ApiError> {
    let instance = deps.service.get_instance(actor, channel).await?;
    let Some(receive) = deps
        .service
        .types(actor)
        .await?
        .into_iter()
        .find(|t| {
            t.descriptor.type_code == instance.record.type_code
                && t.installation_id == instance.record.installation_id
        })
        .and_then(|t| t.descriptor.receive)
    else {
        return Ok(None);
    };
    let id = instance_id(channel);
    let service = McpManagementService::new(deps.store.clone());
    let mut catalog = service.read_catalog_for_actor(actor).await?;
    if !catalog.instances.iter().any(|i| i.instance_id == id) {
        service
            .create_instance_for_actor(
                actor,
                CreateMcpInstanceCommand {
                    actor_user_id: actor.user_id,
                    instance_id: id.clone(),
                    name: instance.record.display_name,
                    description_short: Some(
                        "Channel conversation history and received files".into(),
                    ),
                    status: McpInstanceStatus::Enabled,
                    default_entry_path: "/".into(),
                    webmcp_exposure: WebMcpExposure::Disabled,
                },
            )
            .await?;
        catalog = service.read_catalog_for_actor(actor).await?;
    }
    let interfaces =
        crate::routes::mcp_management::interface_catalog::mcp_interface_catalog_entries_with(
            &deps.catalog,
            actor,
        )
        .await?;
    for (order, tool) in receive.mcp_tools.iter().enumerate() {
        let (operation, path_suffix) = match tool.operation {
            NotificationMcpOperation::HistoryMessages => (
                "communications.messages.list",
                "/conversations/{conversation_id}/messages",
            ),
            NotificationMcpOperation::HistoryFiles => (
                "communications.files.list",
                "/conversations/{conversation_id}/files",
            ),
            NotificationMcpOperation::ReadFile => {
                ("communications.files.get", "/files/{message_id}")
            }
        };
        let entry=interfaces.iter().find(|e|e.interface_id==operation || (e.method=="GET"&&e.path==format!("/api/console/communications/instances/{{instance_id}}{path_suffix}")))
            .cloned().ok_or(control_plane::errors::ControlPlaneError::NotFound("communication_mcp_interface"))?;
        let tool_id = format!("{id}_{}", tool.name);
        if !catalog.tools.iter().any(|t| t.tool_id == tool_id) {
            service
                .create_tool_for_actor(
                    actor,
                    CreateMcpToolCommand {
                        actor_user_id: actor.user_id,
                        tool_id: tool_id.clone(),
                        des_id: None,
                        name: tool.name.clone(),
                        short_description: tool.description.clone(),
                        full_description: tool.description.clone(),
                        interface_entry: entry.clone(),
                        input_mapping: channel_input_mapping(
                            &entry.parameter_descriptors,
                            &tool.input_schema,
                            channel,
                        ),
                        output_mapping: json!({}),
                        max_inline_chars: None,
                        response_fields: None,
                        status: McpToolStatus::Enabled,
                    },
                )
                .await?;
        }
        let instance_record = catalog
            .instances
            .iter()
            .find(|i| i.instance_id == id)
            .ok_or(control_plane::errors::ControlPlaneError::NotFound(
                "mcp_instance",
            ))?;
        if !catalog
            .bindings
            .iter()
            .any(|b| b.instance_record_id == instance_record.id && b.tool_id == tool_id)
        {
            service
                .create_tool_binding_for_actor(
                    actor,
                    CreateMcpToolBindingCommand {
                        actor_user_id: actor.user_id,
                        instance_id: id.clone(),
                        group_path: "/".into(),
                        tool_id,
                        display_alias: None,
                        visible: true,
                        sort_order: order as i32,
                    },
                )
                .await?;
        }
    }
    Ok(Some(id))
}

fn channel_input_mapping(
    parameters: &[domain::mcp_management::McpParameterDescriptor],
    schema: &Value,
    channel: Uuid,
) -> Value {
    let mut interface_parameters = parameters
        .iter()
        .map(|parameter| json!(parameter))
        .collect::<Vec<_>>();
    // The canonical call-control contract declares all three controls. Only the
    // description acknowledgement is opened; the other controls retain tool defaults.
    for (name, field_type, description) in [
        (
            "des_id",
            "string",
            "Description identifier returned by mcp_get",
        ),
        (
            "max_inline_chars",
            "integer",
            "Maximum inline response characters",
        ),
        (
            "response_fields",
            "array<string>",
            "Response field JSON pointers",
        ),
    ] {
        interface_parameters.push(json!({
            "name":name,"field_type":field_type,"parameter_type":"json_body",
            "description":description,"required":false,"source":{"kind":"mcp_call"}
        }));
    }
    let mut mappings = parameters
        .iter()
        .map(|parameter| {
            let description = schema
                .pointer(&format!("/properties/{}/description", parameter.name))
                .and_then(Value::as_str)
                .or(parameter.description.as_deref())
                .unwrap_or("");
            json!({"interface_param":parameter.name,"mcp_param":parameter.name,
            "description":description,"required":parameter.required,
            "hidden":parameter.name=="instance_id",
            "default_value":if parameter.name=="instance_id" {json!(channel)} else {Value::Null}})
        })
        .collect::<Vec<_>>();
    mappings.push(json!({"interface_param":"des_id","mcp_param":"des_id",
        "description":"Description identifier returned by mcp_get","required":true,
        "source":{"kind":"mcp_call","path":"des_id"}}));
    json!({"interface_parameters":interface_parameters,"mappings":mappings})
}

#[cfg(test)]
#[path = "_tests/provisioning.rs"]
mod tests;
