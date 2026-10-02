//! Chat completion usage is a turn projection of the immutable callback boundary.
use super::{openai_usage, OpenAiUsage};
use control_plane::application_public_api::native::NativeUsage;
use serde_json::Value;

#[derive(Clone, Default)]
pub(crate) enum ChatUsageBaseline {
    #[default]
    NewRun,
    Recorded(Option<NativeUsage>),
    Unavailable,
}
impl ChatUsageBaseline {
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
    pub(crate) fn project(&self, cumulative: Option<&NativeUsage>) -> Option<OpenAiUsage> {
        match self {
            Self::NewRun => Some(openai_usage(cumulative)),
            Self::Unavailable => None,
            Self::Recorded(baseline) => {
                let mut turn = cumulative?.clone();
                if let Some(baseline) = baseline {
                    fn difference(value: Option<u64>, before: Option<u64>) -> Option<u64> {
                        value.and_then(|value| value.checked_sub(before.unwrap_or_default()))
                    }
                    // A regressing counter is not reliable usage for this turn.
                    macro_rules! subtract {
                        ($($field:ident),*) => {$(
                            let original = turn.$field;
                            turn.$field = difference(turn.$field, baseline.$field);
                            if original.is_some() && turn.$field.is_none() { return None; }
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
                Some(openai_usage(Some(&turn)))
            }
        }
    }
}

pub(super) fn to_chat_response_for_round(
    run: control_plane::application_public_api::native::NativeRunResult,
    model: String,
    completion_id: String,
    baseline: &ChatUsageBaseline,
) -> Result<super::OpenAiChatCompletionResponse, super::OpenAiRouteError> {
    let usage = baseline.project(run.usage.as_ref());
    let mut response = super::to_openai_response(run, model, completion_id)?;
    response.usage = usage;
    Ok(response)
}
