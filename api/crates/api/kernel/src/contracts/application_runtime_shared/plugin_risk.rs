use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct PluginRiskOverrideBody {
    pub reason: String,
    pub acknowledged_warnings: Vec<String>,
}
