//! Request-local usage is projected from the immutable admitted callback boundary.
#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
use control_plane::application_public_api::native::NativeUsage;
#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
use serde_json::Value;

pub(crate) use application_public_protocol::usage::ResponseUsageBaseline;

pub(super) fn to_chat_response_for_round(
    run: control_plane::application_public_api::native::NativeRunResult,
    model: String,
    completion_id: String,
    baseline: &ResponseUsageBaseline,
) -> Result<super::OpenAiChatCompletionResponse, super::OpenAiRouteError> {
    let usage = baseline.project(run.usage.as_ref());
    let mut response = super::to_openai_response(run, model, completion_id)?;
    response.usage = usage;
    Ok(response)
}

#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
#[path = "_tests/usage_boundary.rs"]
mod tests;
