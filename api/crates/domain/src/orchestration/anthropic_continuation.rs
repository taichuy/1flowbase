use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Full Anthropic context admitted atomically with an owned tool callback.
/// Prompt/system values are emitted by the canonical Anthropic translator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnthropicContinuation {
    pub ordered_messages: Vec<Value>,
    pub system: Value,
    pub model_tool_context: Value,
    pub model_parameters: Value,
    pub client_protocol_envelope: Value,
}

/// Retry transport attempts do not change the admitted model context. Keep
/// the original envelope in the receipt, but ignore this SDK transport counter
/// when identifying a duplicate. Every other header/body field remains bound.
pub fn callback_resume_payloads_match(left: &Value, right: &Value) -> bool {
    fn semantic_payload(payload: &Value) -> Value {
        let mut normalized = payload.clone();
        if let Some(headers) = normalized
            .pointer_mut("/anthropic_continuation/client_protocol_envelope/headers")
            .and_then(Value::as_object_mut)
        {
            headers.retain(|name, _| !name.eq_ignore_ascii_case("x-stainless-retry-count"));
        }
        // Typed envelopes omit empty header maps. A retry counter added to
        // an otherwise headerless request must normalize back to that shape.
        if let Some(envelope) = normalized
            .pointer_mut("/anthropic_continuation/client_protocol_envelope")
            .and_then(Value::as_object_mut)
        {
            if envelope
                .get("headers")
                .and_then(Value::as_object)
                .is_some_and(serde_json::Map::is_empty)
            {
                envelope.remove("headers");
            }
        }
        normalized
    }
    left == right || semantic_payload(left) == semantic_payload(right)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn retry_counter_is_transport_only_while_message_and_beta_remain_bound() {
        let original = json!({"tool_results":[{"tool_call_id":"a","content":"file"}],"anthropic_continuation":{
            "ordered_messages":[{"role":"user","content":"instruction"}],
            "client_protocol_envelope":{"headers":{"anthropic-beta":["context-1m-2025-08-07"],"x-stainless-retry-count":["0"]}}
        }});
        let mut retry = original.clone();
        retry["anthropic_continuation"]["client_protocol_envelope"]["headers"]
            ["x-stainless-retry-count"] = json!(["1"]);
        assert!(callback_resume_payloads_match(&original, &retry));
        retry["anthropic_continuation"]["client_protocol_envelope"]["headers"]
            .as_object_mut()
            .unwrap()
            .remove("x-stainless-retry-count");
        retry["anthropic_continuation"]["client_protocol_envelope"]["headers"]
            ["X-Stainless-Retry-Count"] = json!(["2"]);
        assert!(callback_resume_payloads_match(&original, &retry));
        let mut changed = retry.clone();
        changed["anthropic_continuation"]["ordered_messages"][0]["content"] =
            json!("different instruction");
        assert!(!callback_resume_payloads_match(&original, &changed));
        let mut changed = retry;
        changed["anthropic_continuation"]["client_protocol_envelope"]["headers"]
            ["anthropic-beta"] = json!(["different-beta"]);
        assert!(!callback_resume_payloads_match(&original, &changed));
        assert_eq!(
            original["anthropic_continuation"]["client_protocol_envelope"]["headers"]
                ["x-stainless-retry-count"],
            json!(["0"])
        );
    }
    #[test]
    fn sdk_counter_only_headers_match_an_original_headerless_envelope() {
        let original = json!({"tool_results":[],"anthropic_continuation":{
            "ordered_messages":[{"role":"user","content":"current instruction"}],
            "client_protocol_envelope":{"source_protocol":"anthropic_messages"}
        }});
        let mut retry = original.clone();
        retry["anthropic_continuation"]["client_protocol_envelope"]["headers"] =
            json!({"x-stainless-retry-count":["1"]});
        assert!(callback_resume_payloads_match(&original, &retry));
        assert!(callback_resume_payloads_match(&retry, &original));
        let mut changed = retry.clone();
        changed["anthropic_continuation"]["client_protocol_envelope"]["headers"]
            ["anthropic-beta"] = json!(["semantic-beta"]);
        assert!(!callback_resume_payloads_match(&original, &changed));
        let mut changed = retry.clone();
        changed["anthropic_continuation"]["ordered_messages"][0]["content"] =
            json!("different instruction");
        assert!(!callback_resume_payloads_match(&original, &changed));
        assert!(
            original["anthropic_continuation"]["client_protocol_envelope"]
                .get("headers")
                .is_none()
        );
        assert_eq!(
            retry["anthropic_continuation"]["client_protocol_envelope"]["headers"]
                ["x-stainless-retry-count"],
            json!(["1"])
        );
    }
}
