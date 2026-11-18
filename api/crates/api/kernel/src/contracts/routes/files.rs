use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UploadedFileResponse {
    pub file_table_id: String,
    pub storage_id: String,
    #[schema(value_type = Object)]
    pub record: serde_json::Value,
}
