//! A unary protocol result is generated once inside the typed handler, then
//! consumed unchanged by the HTTP adapter and document observation.
use super::*;
use crate::event_center::api_projection::{self as ep, ApiEventOperationContext, ApiEventOutput};
use crate::routes::application_public_api::{anthropic, openai};
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug)]
enum ProjectionError {
    Native(NativeApiError),
    OpenAi(openai::OpenAiRouteError),
    Anthropic(anthropic::AnthropicRouteError),
    Missing,
}
pub(crate) type Projection = Box<dyn FnOnce(&NativeRunResult) -> ProtocolResult + Send + Sync>;
pub(crate) struct ProtocolResult(Result<Value, ProjectionError>);
#[derive(Debug)]
pub(crate) struct CompatibilityProtocolTargetError {
    run: Option<Box<NativeRunResult>>,
    error: ProjectionError,
}
impl From<NativeApiError> for CompatibilityProtocolTargetError {
    fn from(error: NativeApiError) -> Self {
        Self {
            run: None,
            error: ProjectionError::Native(error),
        }
    }
}
impl CompatibilityProtocolTargetError {
    pub(crate) fn run(&self) -> Option<&NativeRunResult> {
        self.run.as_deref()
    }
    pub(crate) fn into_openai(self) -> openai::OpenAiRouteError {
        match self.error {
            ProjectionError::OpenAi(error) => error,
            ProjectionError::Native(error)
            | ProjectionError::Anthropic(anthropic::AnthropicRouteError::Native(error)) => {
                error.into()
            }
            ProjectionError::Anthropic(_) | ProjectionError::Missing => internal_error().into(),
        }
    }
    pub(crate) fn into_anthropic(self) -> anthropic::AnthropicRouteError {
        match self.error {
            ProjectionError::Anthropic(error) => error,
            ProjectionError::Native(error)
            | ProjectionError::OpenAi(openai::OpenAiRouteError::Native(error)) => error.into(),
            ProjectionError::OpenAi(_) | ProjectionError::Missing => internal_error().into(),
        }
    }
    fn summary(&self) -> (u16, &str, usize) {
        match &self.error {
            ProjectionError::Native(error)|ProjectionError::OpenAi(openai::OpenAiRouteError::Native(error))|ProjectionError::Anthropic(anthropic::AnthropicRouteError::Native(error))=>(error.status.as_u16(),error.code,error.message.len()),
            ProjectionError::OpenAi(openai::OpenAiRouteError::Compat(error))=>(400,&error.code,error.message.len()),
            ProjectionError::Anthropic(anthropic::AnthropicRouteError::Compat(error))=>(400,&error.error_type,error.message.len()),
            ProjectionError::OpenAi(openai::OpenAiRouteError::RequiredAction)|ProjectionError::Anthropic(anthropic::AnthropicRouteError::RequiredAction)=>(409,"required_action_not_supported","waiting states are not supported by compatible endpoints; use the Native API to inspect and resume required_action runs".len()),
            ProjectionError::Missing=>(500,"protocol_result_projection_unavailable","protocol result projection is unavailable".len()),
        }
    }
}
impl InterfaceContract for CompatibilityProtocolTargetError {
    const CONTRACT_ID: &'static str = "application-compatibility-protocol-error";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<Value> {
        CompatibilityBlockingTargetError::managed_projection_schema()
    }
    fn project_for_managed_hook(&self) -> Option<Value> {
        use crate::extension_bus::managed_projection as mp;
        let (status, code, message_bytes) = self.summary();
        Some(
            json!({"0":{"status":status,"code":mp::text(code)?,"message":{"byte_count":message_bytes}}}),
        )
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

#[derive(Debug)]
pub(crate) struct CompatibilityProtocolOutput {
    run: NativeRunResult,
    result: Value,
}
impl CompatibilityProtocolOutput {
    pub(super) fn project(
        run: NativeRunResult,
        projection: Option<Projection>,
    ) -> Result<Self, CompatibilityProtocolTargetError> {
        let result = match projection {
            Some(project) => project(&run).0,
            None => Err(ProjectionError::Missing),
        };
        match result {
            Ok(result) => Ok(Self { run, result }),
            Err(error) => Err(CompatibilityProtocolTargetError {
                run: Some(Box::new(run)),
                error,
            }),
        }
    }
    pub(crate) fn into_parts(self) -> (NativeRunResult, Value) {
        (self.run, self.result)
    }
}

impl InterfaceContract for CompatibilityProtocolOutput {
    // Distinct Rust carriers need distinct contract identities. The safe managed
    // fields remain the legacy run projection; the prepared HTTP JSON stays private.
    const CONTRACT_ID: &'static str = "application-compatibility-protocol-output";
    const CONTRACT_VERSION: &'static str = "1";
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
        Ok(self.result.clone())
    }
}

#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
#[path = "_tests/http_result.rs"]
mod tests;
