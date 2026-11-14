use axum::{http::StatusCode, Json};
use control_plane::frontstage as source;
use serde::Serialize;
use utoipa::ToSchema;
#[derive(Debug, Serialize, ToSchema)]
pub struct SourcePositionResponse {
    pub line: u32,
    pub column: u32,
}

impl From<source::SourcePosition> for SourcePositionResponse {
    fn from(value: source::SourcePosition) -> Self {
        Self {
            line: value.line,
            column: value.column,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageSourceEditErrorDetails {
    pub edit_index: Option<usize>,
    pub current_source_revision: Option<String>,
    pub candidates: Vec<SourcePositionResponse>,
    pub candidates_truncated: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageSourceEditErrorResponse {
    pub status: u16,
    pub code: String,
    pub message: String,
    pub details: Option<FrontstageSourceEditErrorDetails>,
}

pub(crate) fn source_edit_error_response(
    error: &source::FrontstageSourceEditError,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let status = if error.code == "frontstage_block_source_revision" {
        StatusCode::CONFLICT
    } else {
        StatusCode::BAD_REQUEST
    };
    (
        status,
        Json(FrontstageSourceEditErrorResponse {
            status: status.as_u16(),
            code: error.code.to_owned(),
            message: error.to_string(),
            details: Some(FrontstageSourceEditErrorDetails {
                edit_index: error.edit_index,
                current_source_revision: error.current_source_revision.clone(),
                candidates: error.candidates.iter().cloned().map(Into::into).collect(),
                candidates_truncated: error.candidates_truncated,
            }),
        }),
    )
        .into_response()
}
