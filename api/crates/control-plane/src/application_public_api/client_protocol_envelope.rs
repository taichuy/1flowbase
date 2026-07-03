use plugin_framework::provider_contract::ClientProtocolEnvelope;
use std::collections::BTreeMap;

pub const ANTHROPIC_BETA_HEADER_NAME: &str = "anthropic-beta";
pub const ANTHROPIC_CONTEXT_1M_BETA_HEADER_VALUE: &str = "context-1m-2025-08-07";
const ANTHROPIC_MESSAGES_SOURCE_PROTOCOL: &str = "anthropic_messages";
const ANTHROPIC_MESSAGES_POLICY: &str = "anthropic_messages_v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientProtocolIngressPolicy {
    AnthropicMessages,
    DefaultDeny,
}

pub fn capture_client_protocol_envelope<I, N, V>(
    policy: ClientProtocolIngressPolicy,
    headers: I,
) -> Option<ClientProtocolEnvelope>
where
    I: IntoIterator<Item = (N, V)>,
    N: AsRef<str>,
    V: AsRef<str>,
{
    let policy_spec = match policy {
        ClientProtocolIngressPolicy::AnthropicMessages => anthropic_messages_policy(),
        ClientProtocolIngressPolicy::DefaultDeny => return None,
    };
    let mut captured = BTreeMap::new();

    for (name, value) in headers {
        let header_name = name.as_ref().trim().to_ascii_lowercase();
        if header_name.is_empty()
            || blocked_header(&header_name)
            || !policy_spec.allowed_headers.contains(&header_name.as_str())
        {
            continue;
        }
        let header_value = value.as_ref().trim();
        if header_value.is_empty() {
            continue;
        }
        captured.insert(header_name, header_value.to_string());
    }

    (!captured.is_empty()).then(|| ClientProtocolEnvelope {
        source_protocol: policy_spec.source_protocol.to_string(),
        policy: policy_spec.policy.to_string(),
        headers: captured,
    })
}

pub fn anthropic_messages_envelope_with_beta(beta: &'static str) -> ClientProtocolEnvelope {
    ClientProtocolEnvelope {
        source_protocol: ANTHROPIC_MESSAGES_SOURCE_PROTOCOL.to_string(),
        policy: ANTHROPIC_MESSAGES_POLICY.to_string(),
        headers: BTreeMap::from([(ANTHROPIC_BETA_HEADER_NAME.to_string(), beta.to_string())]),
    }
}

pub fn merge_anthropic_messages_envelopes(
    captured: Option<ClientProtocolEnvelope>,
    generated: Option<ClientProtocolEnvelope>,
) -> Option<ClientProtocolEnvelope> {
    match (captured, generated) {
        (None, None) => None,
        (Some(envelope), None) | (None, Some(envelope)) => Some(envelope),
        (Some(mut captured), Some(generated)) => {
            for (name, value) in generated.headers {
                if name == ANTHROPIC_BETA_HEADER_NAME {
                    merge_anthropic_beta_header(&mut captured.headers, &value);
                } else {
                    captured.headers.entry(name).or_insert(value);
                }
            }
            Some(captured)
        }
    }
}

fn merge_anthropic_beta_header(headers: &mut BTreeMap<String, String>, value: &str) {
    let new_betas = split_anthropic_beta_header(value);
    if new_betas.is_empty() {
        return;
    }
    let mut betas = headers
        .get(ANTHROPIC_BETA_HEADER_NAME)
        .map(|value| split_anthropic_beta_header(value))
        .unwrap_or_default();

    for beta in new_betas {
        if !betas
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&beta))
        {
            betas.push(beta);
        }
    }
    headers.insert(ANTHROPIC_BETA_HEADER_NAME.to_string(), betas.join(","));
}

fn split_anthropic_beta_header(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

struct ClientProtocolPolicySpec {
    source_protocol: &'static str,
    policy: &'static str,
    allowed_headers: &'static [&'static str],
}

fn anthropic_messages_policy() -> ClientProtocolPolicySpec {
    ClientProtocolPolicySpec {
        source_protocol: ANTHROPIC_MESSAGES_SOURCE_PROTOCOL,
        policy: ANTHROPIC_MESSAGES_POLICY,
        allowed_headers: &[
            "anthropic-version",
            ANTHROPIC_BETA_HEADER_NAME,
            "x-claude-code-session-id",
            "anthropic-client-name",
            "anthropic-client-version",
            "x-client-name",
            "x-client-version",
            "user-agent",
        ],
    }
}

fn blocked_header(name: &str) -> bool {
    matches!(
        name,
        "authorization"
            | "proxy-authorization"
            | "x-api-key"
            | "api-key"
            | "cookie"
            | "set-cookie"
            | "x-csrf-token"
            | "x-xsrf-token"
            | "csrf-token"
            | "host"
            | "content-length"
            | "connection"
            | "transfer-encoding"
            | "accept-encoding"
            | "te"
            | "trailer"
            | "upgrade"
            | "keep-alive"
    )
}
