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
                ("communications.files.get", "/files/{attachment_id}")
            }
        };
        let entry=interfaces.iter().find(|e|e.interface_id==operation || (e.method=="GET"&&e.path==format!("/api/console/communications/instances/{{instance_id}}{path_suffix}")))
            .cloned().ok_or(control_plane::errors::ControlPlaneError::NotFound("communication_mcp_interface"))?;
        let tool_id = format!("{id}_{}", tool.name);
        if !catalog.tools.iter().any(|t| t.tool_id == tool_id) {
            let mut mappings:Vec<Value>=entry.parameter_descriptors.iter().map(|p|{
                let description=tool.input_schema.pointer(&format!("/properties/{}/description",p.name)).and_then(Value::as_str).unwrap_or_else(|| p.description.as_deref().unwrap_or(""));
                json!({"interface_param":p.name,"mcp_param":p.name,"description":description,"required":p.required,"hidden":p.name=="instance_id","default_value":if p.name=="instance_id"{json!(channel)}else{Value::Null}})
            }).collect();
            // mcp_get reveals des_id; mcp_call must supply it before running a discovered tool.
            mappings.push(json!({"interface_param":"des_id","mcp_param":"des_id","description":"Description identifier returned by mcp_get","required":true,"source":{"kind":"mcp_call","path":"des_id"}}));
            service.create_tool_for_actor(actor,CreateMcpToolCommand{actor_user_id:actor.user_id,tool_id:tool_id.clone(),des_id:None,name:tool.name.clone(),short_description:tool.description.clone(),full_description:tool.description.clone(),interface_entry:entry.clone(),input_mapping:json!({"interface_parameters":entry.parameter_descriptors,"mappings":mappings}),output_mapping:json!({}),max_inline_chars:None,response_fields:None,status:McpToolStatus::Enabled}).await?;
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
