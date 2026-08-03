use super::*;

pub(super) fn bounded_wire_count(length: usize) -> u32 {
    u32::try_from(length).unwrap_or(u32::MAX)
}

pub(super) fn project_protocol_context_envelope(
    envelope: &mut Option<ProtocolContextEnvelope>,
    required_capabilities: &mut BTreeSet<ProviderInvocationCapability>,
    declared_capabilities: &BTreeSet<&str>,
) -> bool {
    required_capabilities.remove(&ProviderInvocationCapability::ProtocolContext);
    let Some(protocol_context) = envelope.as_ref() else {
        return false;
    };
    if protocol_context_profile_is_declared(protocol_context, declared_capabilities) {
        return false;
    }
    *envelope = None;
    true
}

fn protocol_context_profile_is_declared(
    envelope: &ProtocolContextEnvelope,
    declared_capabilities: &BTreeSet<&str>,
) -> bool {
    if envelope.source_protocol == "anthropic_messages" && envelope.source_request.is_some() {
        return declared_capabilities
            .contains(PROVIDER_PROTOCOL_CONTEXT_RESTORE_ANTHROPIC_MESSAGES_V2_CAPABILITY);
    }
    let profiles: &[&str] = match envelope.source_protocol.as_str() {
        "anthropic_messages" => &[
            PROVIDER_PROTOCOL_CONTEXT_CONSUME_ANTHROPIC_MESSAGES_V1_CAPABILITY,
            PROVIDER_PROTOCOL_CONTEXT_RESTORE_ANTHROPIC_MESSAGES_V1_CAPABILITY,
            PROVIDER_PROTOCOL_CONTEXT_RESTORE_ANTHROPIC_MESSAGES_V2_CAPABILITY,
        ],
        "openai_chat" => &[
            PROVIDER_PROTOCOL_CONTEXT_CONSUME_OPENAI_CHAT_V1_CAPABILITY,
            PROVIDER_PROTOCOL_CONTEXT_RESTORE_OPENAI_CHAT_V1_CAPABILITY,
        ],
        "openai_responses" => &[
            PROVIDER_PROTOCOL_CONTEXT_CONSUME_OPENAI_RESPONSES_V1_CAPABILITY,
            PROVIDER_PROTOCOL_CONTEXT_RESTORE_OPENAI_RESPONSES_V1_CAPABILITY,
        ],
        _ => return false,
    };
    profiles
        .iter()
        .any(|profile| declared_capabilities.contains(profile))
}

pub fn validate_protocol_context_envelope(
    envelope: &ProtocolContextEnvelope,
) -> Result<(), String> {
    if envelope.source_protocol.trim().is_empty() {
        return Err("protocol context source_protocol must not be empty".to_string());
    }
    if envelope
        .query
        .keys()
        .chain(envelope.headers.keys())
        .chain(envelope.body.keys())
        .any(|name| !protocol_context_root_field_is_safe(name))
        || envelope
            .body
            .values()
            .any(protocol_context_value_contains_unsafe_field)
    {
        return Err(
            "protocol context contains a reserved, typed, or credential-bearing field".to_string(),
        );
    }
    if let Some(source_body) = envelope
        .source_request
        .as_ref()
        .and_then(|request| request.body.as_ref())
    {
        let source_body = source_body
            .as_object()
            .ok_or_else(|| "protocol context source request body must be an object".to_string())?;
        if source_body
            .keys()
            .any(|name| !protocol_context_field_is_safe(name))
        {
            return Err(
                "protocol context source request body contains a credential-bearing root field"
                    .to_string(),
            );
        }
    }
    Ok(())
}

fn protocol_context_root_field_is_safe(name: &str) -> bool {
    const TYPED_ROOTS: &str = "contract-version operation profile provider-instance-id provider-code protocol model previous-response-id provider-config messages system request-context required-capabilities tools mcp-bindings response-format model-parameters client-protocol-envelope native-transport trace-context run-context";
    let normalized = normalized_protocol_context_field(name);
    protocol_context_field_is_safe(&normalized)
        && !TYPED_ROOTS
            .split_ascii_whitespace()
            .any(|typed| typed == normalized.as_str())
}

fn protocol_context_value_contains_unsafe_field(value: &Value) -> bool {
    match value {
        Value::Array(values) => values
            .iter()
            .any(protocol_context_value_contains_unsafe_field),
        Value::Object(object) => object.iter().any(|(name, value)| {
            !protocol_context_field_is_safe(name)
                || protocol_context_value_contains_unsafe_field(value)
        }),
        _ => false,
    }
}

fn normalized_protocol_context_field(name: &str) -> String {
    name.trim().to_ascii_lowercase().replace('_', "-")
}

fn protocol_context_field_is_safe(name: &str) -> bool {
    const BLOCKED_FIELDS: &str = "auth authentication authentication-info authorization x-authorization proxy-authorization proxy-authenticate proxy-authentication-info www-authenticate x-api-key api-key x-auth-token auth-token bearer-token x-access-token access-token refresh-token id-token client-secret api-secret password passwd x-csrf-token x-xsrf-token csrf-token cookie set-cookie host connection proxy-connection keep-alive te trailer transfer-encoding upgrade content-length client-protocol-envelope native-model-prompt-context native-model-request-context native-transport provider-transport request-context run-context trace-context compatibility-mode sys env trigger forwarded via x-real-ip true-client-ip cf-connecting-ip cf-ray traceparent tracestate baggage x-request-id internal x-internal 1flowbase x-1flowbase";
    let normalized = normalized_protocol_context_field(name);
    if normalized.is_empty() || normalized.starts_with("--") {
        return false;
    }
    !BLOCKED_FIELDS
        .split_ascii_whitespace()
        .any(|blocked| blocked == normalized.as_str())
        && !normalized.starts_with("x-1flowbase-")
        && !normalized.starts_with("x-internal-")
        && !normalized.starts_with("internal-")
        && !normalized.starts_with("x-forwarded-")
        && !normalized.starts_with("x-envoy-")
        && !normalized.starts_with("x-amzn-")
}
