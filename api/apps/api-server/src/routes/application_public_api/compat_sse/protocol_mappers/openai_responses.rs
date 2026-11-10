use super::*;

// No additional projection state; preserve API call sites and test selectors.
#[repr(transparent)]
pub(crate) struct OpenAiResponseStreamMapper(
    application_public_protocol::protocol_mappers::OpenAiResponseStreamMapper,
);

impl OpenAiResponseStreamMapper {
    #[cfg_attr(test, api_test_selection::group(public_api))]
    #[cfg(test)]
    pub(in crate::routes::application_public_api::compat_sse) fn new(
        model: String,
        previous_response_id: Option<String>,
    ) -> Self {
        Self::with_mode(
            model,
            previous_response_id,
            ResponsesProjectionMode::SemanticNativeResponses,
        )
    }

    pub(in crate::routes::application_public_api) fn with_mode(
        model: String,
        previous_response_id: Option<String>,
        mode: ResponsesProjectionMode,
    ) -> Self {
        Self(
            application_public_protocol::protocol_mappers::OpenAiResponseStreamMapper::with_mode(
                model,
                previous_response_id,
                mode,
            ),
        )
    }

    pub(in crate::routes::application_public_api) fn with_usage_baseline(
        mut self,
        baseline: crate::routes::application_public_api::openai::usage::ResponseUsageBaseline,
    ) -> Self {
        self.0 = self.0.with_usage_baseline(baseline);
        self
    }

    pub(in crate::routes::application_public_api::compat_sse) fn runtime_event_to_sse(
        &mut self,
        initial_run: &NativeRunResult,
        event: impl Into<CompatibleRuntimeEventView>,
    ) -> Vec<Result<Event, Infallible>> {
        self.0.runtime_event_to_sse(initial_run, event.into())
    }
}
