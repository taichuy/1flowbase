use std::collections::{BTreeMap, BTreeSet};

use plugin_framework::provider_contract::ProtocolContextEnvelope;
use serde_json::{Map, Value};

pub const ANTHROPIC_BETA_HEADER_NAME: &str = "anthropic-beta";
pub const ANTHROPIC_CONTEXT_1M_BETA_HEADER_VALUE: &str = "context-1m-2025-08-07";
const ANTHROPIC_MESSAGES_SOURCE_PROTOCOL: &str = "anthropic_messages";
const OPENAI_CHAT_SOURCE_PROTOCOL: &str = "openai_chat";
const OPENAI_RESPONSES_SOURCE_PROTOCOL: &str = "openai_responses";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientProtocolIngressPolicy {
    AnthropicMessages,
    OpenAiChat,
    OpenAiResponses,
}

impl ClientProtocolIngressPolicy {
    fn source_protocol(self) -> &'static str {
        match self {
            Self::AnthropicMessages => ANTHROPIC_MESSAGES_SOURCE_PROTOCOL,
            Self::OpenAiChat => OPENAI_CHAT_SOURCE_PROTOCOL,
            Self::OpenAiResponses => OPENAI_RESPONSES_SOURCE_PROTOCOL,
        }
    }
}

pub fn capture_client_protocol_envelope<I, N, V>(
    policy: ClientProtocolIngressPolicy,
    headers: I,
) -> Option<ProtocolContextEnvelope>
where
    I: IntoIterator<Item = (N, V)>,
    N: AsRef<str>,
    V: AsRef<str>,
{
    let headers = headers
        .into_iter()
        .map(|(name, value)| (name.as_ref().to_string(), value.as_ref().to_string()))
        .collect::<Vec<_>>();
    let connection_headers = headers
        .iter()
        .filter(|(name, _)| name.trim().eq_ignore_ascii_case("connection"))
        .flat_map(|(_, value)| value.split(','))
        .map(|name| name.trim().to_ascii_lowercase())
        .filter(|name| !name.is_empty())
        .collect::<BTreeSet<_>>();
    let mut captured = BTreeMap::new();

    for (name, value) in headers {
        let header_name = name.trim().to_ascii_lowercase();
        if header_name.is_empty()
            || blocked_header(policy, &header_name)
            || connection_headers.contains(&header_name)
        {
            continue;
        }
        for value in residual_header_values(policy, &header_name, value) {
            captured
                .entry(header_name.clone())
                .or_insert_with(Vec::new)
                .push(value);
        }
    }

    protocol_context(policy, BTreeMap::new(), captured, BTreeMap::new())
}

pub fn anthropic_context_1m_requested<'a>(values: impl IntoIterator<Item = &'a str>) -> bool {
    values
        .into_iter()
        .flat_map(|value| split_anthropic_beta_header(value))
        .any(is_anthropic_context_1m_beta)
}

pub fn capture_client_protocol_query<I, N, V>(
    policy: ClientProtocolIngressPolicy,
    query: I,
) -> Option<ProtocolContextEnvelope>
where
    I: IntoIterator<Item = (N, V)>,
    N: AsRef<str>,
    V: AsRef<str>,
{
    let mut captured = BTreeMap::new();
    for (name, value) in query {
        let name = name.as_ref();
        if !protocol_context_field_is_safe(name) {
            continue;
        }
        captured
            .entry(name.to_string())
            .or_insert_with(Vec::new)
            .push(value.as_ref().to_string());
    }
    protocol_context(policy, captured, BTreeMap::new(), BTreeMap::new())
}

pub fn capture_client_protocol_body(
    policy: ClientProtocolIngressPolicy,
    body: &Map<String, Value>,
    typed_root_fields: &[&str],
) -> Option<ProtocolContextEnvelope> {
    let captured = body
        .iter()
        .filter(|(name, _)| {
            !typed_root_fields.contains(&name.as_str()) && protocol_context_field_is_safe(name)
        })
        .map(|(name, value)| (name.clone(), sanitized_protocol_context_value(value)))
        .collect();
    protocol_context(policy, BTreeMap::new(), BTreeMap::new(), captured)
}

pub fn merge_client_protocol_envelopes(
    policy: ClientProtocolIngressPolicy,
    first: Option<ProtocolContextEnvelope>,
    second: Option<ProtocolContextEnvelope>,
) -> Option<ProtocolContextEnvelope> {
    let mut merged = ProtocolContextEnvelope {
        source_protocol: policy.source_protocol().to_string(),
        ..ProtocolContextEnvelope::default()
    };
    for envelope in [first, second].into_iter().flatten() {
        debug_assert_eq!(envelope.source_protocol, policy.source_protocol());
        if envelope.source_protocol != policy.source_protocol() {
            continue;
        }
        extend_multi_values(&mut merged.query, envelope.query);
        extend_multi_values(&mut merged.headers, envelope.headers);
        for (name, value) in envelope.body {
            merged.body.entry(name).or_insert(value);
        }
    }
    non_empty_protocol_context(merged)
}

pub(crate) fn protocol_context_field_is_safe(name: &str) -> bool {
    let lower = name.trim().to_ascii_lowercase();
    if lower.is_empty() || lower.starts_with("__") {
        return false;
    }
    let normalized = lower.replace('_', "-");
    !matches!(
        normalized.as_str(),
        "auth"
            | "authentication"
            | "authentication-info"
            | "authorization"
            | "x-authorization"
            | "proxy-authorization"
            | "proxy-authenticate"
            | "proxy-authentication-info"
            | "www-authenticate"
            | "x-api-key"
            | "api-key"
            | "x-auth-token"
            | "auth-token"
            | "bearer-token"
            | "x-access-token"
            | "access-token"
            | "refresh-token"
            | "id-token"
            | "client-secret"
            | "api-secret"
            | "password"
            | "passwd"
            | "x-csrf-token"
            | "x-xsrf-token"
            | "csrf-token"
            | "cookie"
            | "set-cookie"
            | "host"
            | "connection"
            | "proxy-connection"
            | "keep-alive"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "content-length"
            | "client-protocol-envelope"
            | "native-model-prompt-context"
            | "native-model-request-context"
            | "native-transport"
            | "provider-transport"
            | "request-context"
            | "run-context"
            | "trace-context"
            | "compatibility-mode"
            | "sys"
            | "env"
            | "trigger"
            | "forwarded"
            | "via"
            | "x-real-ip"
            | "true-client-ip"
            | "cf-connecting-ip"
            | "cf-ray"
            | "traceparent"
            | "tracestate"
            | "baggage"
            | "x-request-id"
            | "internal"
            | "x-internal"
            | "1flowbase"
            | "x-1flowbase"
    ) && !normalized.starts_with("x-1flowbase-")
        && !normalized.starts_with("x-internal-")
        && !normalized.starts_with("internal-")
        && !normalized.starts_with("x-forwarded-")
        && !normalized.starts_with("x-envoy-")
        && !normalized.starts_with("x-amzn-")
}

fn protocol_context(
    policy: ClientProtocolIngressPolicy,
    query: BTreeMap<String, Vec<String>>,
    headers: BTreeMap<String, Vec<String>>,
    body: BTreeMap<String, Value>,
) -> Option<ProtocolContextEnvelope> {
    non_empty_protocol_context(ProtocolContextEnvelope {
        source_protocol: policy.source_protocol().to_string(),
        query,
        headers,
        body,
    })
}

fn non_empty_protocol_context(
    envelope: ProtocolContextEnvelope,
) -> Option<ProtocolContextEnvelope> {
    (!envelope.query.is_empty() || !envelope.headers.is_empty() || !envelope.body.is_empty())
        .then_some(envelope)
}

fn extend_multi_values(
    target: &mut BTreeMap<String, Vec<String>>,
    source: BTreeMap<String, Vec<String>>,
) {
    for (name, values) in source {
        target.entry(name).or_default().extend(values);
    }
}

fn split_anthropic_beta_header(value: &str) -> Vec<&str> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect()
}

fn residual_header_values(
    policy: ClientProtocolIngressPolicy,
    name: &str,
    value: String,
) -> Vec<String> {
    if policy != ClientProtocolIngressPolicy::AnthropicMessages
        || name != ANTHROPIC_BETA_HEADER_NAME
    {
        return vec![value];
    }

    let beta_tokens = split_anthropic_beta_header(&value);
    if !beta_tokens
        .iter()
        .copied()
        .any(is_anthropic_context_1m_beta)
    {
        return vec![value];
    }
    let residual = beta_tokens
        .into_iter()
        .filter(|token| !is_anthropic_context_1m_beta(token))
        .collect::<Vec<_>>()
        .join(", ");
    (!residual.is_empty())
        .then_some(residual)
        .into_iter()
        .collect()
}

fn is_anthropic_context_1m_beta(value: &str) -> bool {
    value.eq_ignore_ascii_case(ANTHROPIC_CONTEXT_1M_BETA_HEADER_VALUE)
}

fn sanitized_protocol_context_value(value: &Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(
            values
                .iter()
                .map(sanitized_protocol_context_value)
                .collect(),
        ),
        Value::Object(object) => Value::Object(
            object
                .iter()
                .filter(|(name, _)| protocol_context_field_is_safe(name))
                .map(|(name, value)| (name.clone(), sanitized_protocol_context_value(value)))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn blocked_header(policy: ClientProtocolIngressPolicy, name: &str) -> bool {
    !protocol_context_field_is_safe(name)
        || matches!(
            name,
            "content-type" | "accept" | "accept-encoding" | "accept-language" | "origin"
        )
        || matches!(
            (policy, name),
            (
                ClientProtocolIngressPolicy::AnthropicMessages,
                "x-claude-code-session-id"
            ) | (
                ClientProtocolIngressPolicy::OpenAiResponses,
                "x-codex-turn-metadata"
            )
        )
}
