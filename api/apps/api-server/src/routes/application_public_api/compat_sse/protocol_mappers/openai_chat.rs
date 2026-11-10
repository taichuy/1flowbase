use super::*;

// No additional projection state; preserve API call sites and test selectors.
#[repr(transparent)]
pub(crate) struct OpenAiChatStreamMapper(
    application_public_protocol::protocol_mappers::OpenAiChatStreamMapper,
);

impl OpenAiChatStreamMapper {
    pub(in crate::routes::application_public_api::compat_sse) fn new(
        model: String,
        chat_completion_id: String,
    ) -> Self {
        Self(
            application_public_protocol::protocol_mappers::OpenAiChatStreamMapper::new(
                model,
                chat_completion_id,
            ),
        )
    }

    pub(in crate::routes::application_public_api::compat_sse) fn with_usage_baseline(
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

#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
pub(in crate::routes::application_public_api::compat_sse) use application_public_protocol::protocol_mappers::{
    openai_delta_chunk_payload, openai_finish_chunk_payload, openai_tool_call_chunk_payload,
};
