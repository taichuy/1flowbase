//! A unary protocol result is generated once inside the typed handler, then
//! consumed unchanged by the HTTP adapter and document observation.
use super::*;
use crate::event_center::api_projection::{self as ep, ApiEventOperationContext, ApiEventOutput};
use crate::routes::application_public_api::{anthropic, openai};
use serde::Serialize;
use serde_json::{json, Value};

enum ProjectionError {
    OpenAi(openai::OpenAiRouteError),
    Anthropic(anthropic::AnthropicRouteError),
    Missing,
}
pub(crate) type Projection = Box<dyn FnOnce(&NativeRunResult) -> ProtocolResult + Send + Sync>;
pub(crate) struct ProtocolResult(Result<Value, ProjectionError>);
impl ProtocolResult {
    pub(crate) fn into_openai(self) -> Result<Value, openai::OpenAiRouteError> {
        match self.0 {
            Ok(value) => Ok(value),
            Err(ProjectionError::OpenAi(error)) => Err(error),
            Err(ProjectionError::Anthropic(_) | ProjectionError::Missing) => {
                Err(internal_error().into())
            }
        }
    }
    pub(crate) fn into_anthropic(self) -> Result<Value, anthropic::AnthropicRouteError> {
        match self.0 {
            Ok(value) => Ok(value),
            Err(ProjectionError::Anthropic(error)) => Err(error),
            Err(ProjectionError::OpenAi(_) | ProjectionError::Missing) => {
                Err(internal_error().into())
            }
        }
    }
}

pub(crate) fn openai<T, F>(project: F) -> Projection
where
    T: Serialize,
    F: FnOnce(&NativeRunResult) -> Result<T, openai::OpenAiRouteError> + Send + Sync + 'static,
{
    Box::new(move |run| {
        ProtocolResult(
            project(run)
                .map_err(ProjectionError::OpenAi)
                .and_then(|result| {
                    ep::to_value(&result)
                        .map_err(|_| ProjectionError::OpenAi(internal_error().into()))
                }),
        )
    })
}
pub(crate) fn anthropic<T, F>(project: F) -> Projection
where
    T: Serialize,
    F: FnOnce(&NativeRunResult) -> Result<T, anthropic::AnthropicRouteError>
        + Send
        + Sync
        + 'static,
{
    Box::new(move |run| {
        ProtocolResult(
            project(run)
                .map_err(ProjectionError::Anthropic)
                .and_then(|result| {
                    ep::to_value(&result)
                        .map_err(|_| ProjectionError::Anthropic(internal_error().into()))
                }),
        )
    })
}
fn internal_error() -> NativeApiError {
    NativeApiError::new(
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        "protocol_result_projection_unavailable",
        "protocol result projection is unavailable",
    )
}

pub(crate) struct CompatibilityProtocolOutput {
    run: NativeRunResult,
    result: ProtocolResult,
}
impl CompatibilityProtocolOutput {
    pub(super) fn project(run: NativeRunResult, projection: Option<Projection>) -> Self {
        let result = match projection {
            Some(project) => project(&run),
            None => ProtocolResult(Err(ProjectionError::Missing)),
        };
        Self { run, result }
    }
    pub(crate) fn into_parts(self) -> (NativeRunResult, ProtocolResult) {
        (self.run, self.result)
    }
}
impl InterfaceContract for CompatibilityProtocolOutput {
    // The prepared wire document is private adapter state. Preserve the existing
    // canonical/managed contract identity and its exact legacy projection.
    const CONTRACT_ID: &'static str = CompatibilityBlockingOutput::CONTRACT_ID;
    const CONTRACT_VERSION: &'static str = CompatibilityBlockingOutput::CONTRACT_VERSION;
    fn managed_projection_schema() -> Option<Value> {
        CompatibilityBlockingOutput::managed_projection_schema()
    }
    fn project_for_managed_hook(&self) -> Option<Value> {
        CompatibilityBlockingOutput(self.run.clone()).project_for_managed_hook()
    }
}
impl ApiEventOutput for CompatibilityProtocolOutput {
    fn document_output_schema(operation: &ApiEventOperationContext) -> Option<Value> {
        Some(match operation.interface_id.as_str() {
            "compat.openai.chat-completions.blocking"
            | "compat.openai.chat-completions-root.blocking" => {
                ep::schema::<openai::OpenAiChatCompletionResponse>()
            }
            "compat.openai.responses.blocking" | "compat.openai.responses-root.blocking" => {
                ep::schema::<openai::OpenAiResponsesObject>()
            }
            "compat.openai.responses-compact.blocking" => {
                json!({"anyOf":[{"type":"array","items":true},ep::schema::<openai::OpenAiResponsesObject>()]})
            }
            "compat.anthropic.messages.blocking" => {
                ep::schema::<anthropic::AnthropicMessageResponse>()
            }
            "compat.anthropic.messages.count-tokens" => {
                ep::schema::<anthropic::AnthropicCountTokensResponse>()
            }
            _ => return None,
        })
    }
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        self.result
            .0
            .as_ref()
            .cloned()
            .map_err(|_| Arc::from("protocol_result_projection_failed"))
    }
}

#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
#[path = "_tests/http_result.rs"]
mod tests;
