//! Request-local usage is projected from the immutable admitted callback boundary.
use super::{openai_responses_usage, openai_usage, OpenAiResponsesUsage, OpenAiUsage};
use control_plane::application_public_api::native::NativeUsage;
use serde_json::Value;

#[derive(Clone, Default)]
pub(crate) enum ResponseUsageBaseline {
    #[default]
    NewRun,
    Recorded(Option<NativeUsage>),
    Unavailable,
}
impl ResponseUsageBaseline {
    pub(crate) fn from_callback_payload(payload: &Value) -> Self {
        match payload.get("native_usage_baseline") {
            Some(value) => match serde_json::from_value::<Option<NativeUsage>>(value.clone()) {
                Ok(Some(usage))
                    if [
                        usage.prompt_tokens,
                        usage.completion_tokens,
                        usage.total_tokens,
                        usage.reasoning_tokens,
                        usage.input_cache_hit_tokens,
                        usage.input_cache_miss_tokens,
                        usage.cache_read_tokens,
                        usage.cache_write_tokens,
                    ]
                    .iter()
                    .any(Option::is_some) =>
                {
                    Self::Recorded(Some(usage))
                }
                Ok(None) => Self::Recorded(None),
                _ => Self::Unavailable,
            },
            None => Self::Unavailable,
        }
    }
    /// Subtract the recorded boundary, never a live snapshot that can move on replay.
    fn native_round(&self, cumulative: Option<&NativeUsage>) -> Option<NativeUsage> {
        match self {
            Self::NewRun => cumulative.cloned(),
            Self::Unavailable => None,
            Self::Recorded(baseline) => {
                let mut turn = cumulative?.clone();
                if let Some(baseline) = baseline {
                    fn difference(value: Option<u64>, before: Option<u64>) -> Option<u64> {
                        value.and_then(|value| value.checked_sub(before.unwrap_or_default()))
                    }
                    // Regressing or disappearing counters cannot establish round usage.
                    macro_rules! subtract {
                        ($($field:ident),*) => {$(
                            let original = turn.$field;
                            turn.$field = difference(original, baseline.$field);
                            if (original.is_some() || baseline.$field.is_some())
                                && turn.$field.is_none()
                            {
                                return None;
                            }
                        )*};
                    }
                    subtract!(
                        prompt_tokens,
                        completion_tokens,
                        total_tokens,
                        reasoning_tokens,
                        input_cache_hit_tokens,
                        input_cache_miss_tokens,
                        cache_read_tokens,
                        cache_write_tokens
                    );
                }
                Some(turn)
            }
        }
    }

    pub(crate) fn project(&self, cumulative: Option<&NativeUsage>) -> Option<OpenAiUsage> {
        if matches!(self, Self::NewRun) {
            return Some(openai_usage(cumulative));
        }
        self.native_round(cumulative)
            .as_ref()
            .map(|usage| openai_usage(Some(usage)))
    }

    pub(crate) fn project_responses(
        &self,
        cumulative: Option<&NativeUsage>,
    ) -> Option<OpenAiResponsesUsage> {
        if matches!(self, Self::NewRun) {
            return Some(openai_responses_usage(cumulative));
        }
        self.native_round(cumulative)
            .as_ref()
            .map(|usage| openai_responses_usage(Some(usage)))
    }

    /// Only replace usage in snapshots that already carry accounting. Progress,
    /// items and provider deltas retain their original wire shape.
    pub(crate) fn project_responses_event(
        &self,
        event: &mut Value,
        cumulative: Option<&NativeUsage>,
    ) {
        if let Some(response) = event.get_mut("response") {
            if response.get("usage").is_some() {
                response["usage"] = serde_json::json!(self.project_responses(cumulative));
            }
        }
    }
}

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

#[cfg(test)]
#[path = "_tests/usage_boundary.rs"]
mod tests;
