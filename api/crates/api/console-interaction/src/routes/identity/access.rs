use std::{collections::BTreeMap, sync::Arc};

use axum::{extract::State, http::HeaderMap, Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::console_identity_interface::{invoke, ConsoleIdentityInput, ConsoleIdentityOutput};
use crate::{
    app_state::ApiState, error_response::ApiError, extension_bus::ConsoleAuthenticationCredential,
    response::ApiSuccess,
};

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct EvaluateConsoleAccessBody {
    pub codes: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct EvaluateConsoleAccessResponse {
    pub policy_fingerprint: String,
    pub allowed: BTreeMap<String, bool>,
}

#[utoipa::path(
    post,
    path = "/api/console/access/evaluate",
    summary = "Evaluate console access",
    description = "Returns access decisions only for the requested capability codes in the current workspace and active role.",
    request_body = EvaluateConsoleAccessBody,
    responses((status = 200, body = EvaluateConsoleAccessResponse), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn evaluate(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<EvaluateConsoleAccessBody>,
) -> Result<Json<ApiSuccess<EvaluateConsoleAccessResponse>>, ApiError> {
    let output = invoke(
        Arc::clone(&state),
        "http.console.identity.access.evaluate.v1",
        ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        ConsoleIdentityInput::EvaluateAccess(body),
    )
    .await?;
    let ConsoleIdentityOutput::Access(response) = output else {
        return Err(anyhow::anyhow!("console access output contract mismatch").into());
    };
    Ok(Json(ApiSuccess::new(response)))
}
