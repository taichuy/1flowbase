use super::*;
#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
use uuid::Uuid;

// No additional projection state; preserve API call sites and test selectors.
#[repr(transparent)]
pub(crate) struct AnthropicStreamMapper(
    application_public_protocol::protocol_mappers::AnthropicStreamMapper,
);

impl AnthropicStreamMapper {
    pub(in crate::routes::application_public_api::compat_sse) fn new(model: String) -> Self {
        Self(application_public_protocol::protocol_mappers::AnthropicStreamMapper::new(model))
    }

    #[cfg_attr(test, api_test_selection::group(public_api))]
    #[cfg(test)]
    fn anthropic_failed_events(
        &mut self,
        initial_run: &NativeRunResult,
        payload: &Value,
    ) -> Vec<Result<Event, Infallible>> {
        self.0.anthropic_failed_events(initial_run, payload)
    }

    #[cfg_attr(test, api_test_selection::group(public_api))]
    #[cfg(test)]
    pub(super) fn anthropic_stop_events(
        &mut self,
        usage: Option<&NativeUsage>,
    ) -> Vec<Result<Event, Infallible>> {
        self.0.anthropic_stop_events(usage)
    }

    #[cfg_attr(test, api_test_selection::group(public_api))]
    #[cfg(test)]
    pub(in crate::routes::application_public_api::compat_sse) fn anthropic_tool_use_events(
        &mut self,
        payload: &Value,
        usage: Option<&NativeUsage>,
    ) -> Option<Vec<Result<Event, Infallible>>> {
        self.0.anthropic_tool_use_events(payload, usage)
    }

    pub(in crate::routes::application_public_api::compat_sse) fn runtime_event_to_sse(
        &mut self,
        initial_run: &NativeRunResult,
        event: impl Into<CompatibleRuntimeEventView>,
    ) -> Vec<Result<Event, Infallible>> {
        self.0.runtime_event_to_sse(initial_run, event.into())
    }
}

#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
pub(in crate::routes::application_public_api::compat_sse) use application_public_protocol::protocol_mappers::anthropic_delta_payload;

#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
#[path = "_tests/anthropic_errors.rs"]
mod error_tests;
