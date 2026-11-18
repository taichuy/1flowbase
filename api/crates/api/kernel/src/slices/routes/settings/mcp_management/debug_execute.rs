use axum::response::Response;
use serde::Deserialize;
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;
#[derive(Debug, Deserialize, ToSchema)]
pub struct McpDebugExecuteBody {
    pub interface_id: String,
    #[serde(default)]
    pub debug_response_mode: McpDebugResponseMode,
    #[schema(value_type = Object)]
    pub mcp_arguments: Value,
    #[schema(value_type = Object)]
    pub input_mapping: Value,
    #[schema(value_type = Object)]
    pub output_mapping: Value,
}

#[derive(Debug, Default, Deserialize, ToSchema, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum McpDebugResponseMode {
    #[default]
    ToolResult,
    DebugDetails,
}

pub enum McpDebugExecuteError {
    Api(anyhow::Error),
    TargetResponse(Response),
}

impl From<anyhow::Error> for McpDebugExecuteError {
    fn from(value: anyhow::Error) -> Self {
        Self::Api(value)
    }
}

#[derive(Clone, Copy)]
pub struct McpServerBoundInputs {
    pub workspace_id: Uuid,
}
