//! Provider wire traversal. Only known protocol objects carry structural keys;
//! their arbitrary data (schemas, arguments, metadata, etc.) never inherits them.
use std::collections::{BTreeMap, HashMap};

use extension_contracts::provider_contract::{NativePromptBlock, ProviderInvocationInput};
use serde_json::Value;

use super::engine::{placeholder_regex, StageMaskState};
use super::kernel::canonical_json_digest_and_size;

#[derive(Clone, Copy)]
enum Scope {
    User,
    Body,
    Item,
    Tool,
    Format,
}

pub(super) fn visit_user_content(value: &mut Value, mut visit: impl FnMut(&mut String)) {
    visit_json(value, Scope::User, &mut visit);
}

fn visit_json(value: &mut Value, scope: Scope, visit: &mut impl FnMut(&mut String)) {
    match value {
        Value::String(text) => visit(text),
        Value::Array(items) => items
            .iter_mut()
            .for_each(|item| visit_json(item, scope, visit)),
        Value::Object(map) => visit_object(map.iter_mut(), scope, visit),
        _ => {}
    }
}

fn visit_object<'a>(
    entries: impl Iterator<Item = (&'a String, &'a mut Value)>,
    scope: Scope,
    visit: &mut impl FnMut(&mut String),
) {
    for (key, value) in entries {
        let next = match (scope, key.as_str()) {
            (Scope::User, _) => Scope::User,
            (
                _,
                "id"
                | "type"
                | "role"
                | "previous_response_id"
                | "response_id"
                | "call_id"
                | "tool_call_id"
                | "encrypted_content"
                | "signature"
                | "mime_type"
                | "media_type"
                | "model",
            ) => continue,
            (Scope::Body, "conversation" | "stream_options") => continue,
            (Scope::Item | Scope::Tool, "arguments") => {
                visit_wire_arguments(value, visit);
                continue;
            }
            (Scope::Body, "input" | "messages" | "system")
            | (Scope::Item, "content" | "tool_calls" | "summary" | "source") => Scope::Item,
            (Scope::Body, "tools" | "tool_choice") | (Scope::Item | Scope::Tool, "function") => {
                Scope::Tool
            }
            (Scope::Body, "response_format" | "text")
            | (Scope::Format, "format" | "json_schema") => Scope::Format,
            // These are callable names and connection coordinates, not descriptions.
            (
                Scope::Tool,
                "name"
                | "server_label"
                | "server_url"
                | "server_description_id"
                | "allowed_tools"
                | "connector_id"
                | "authorization"
                | "headers",
            )
            | (Scope::Item | Scope::Format, "name") => continue,
            _ => Scope::User,
        };
        visit_json(value, next, visit);
    }
}

fn visit_wire_arguments(value: &mut Value, visit: &mut impl FnMut(&mut String)) {
    if let Value::String(encoded) = value {
        if let Ok(mut decoded) = serde_json::from_str::<Value>(encoded) {
            let original = decoded.clone();
            visit_json(&mut decoded, Scope::User, visit);
            if decoded != original {
                *encoded = decoded.to_string();
            }
            return;
        }
    }
    visit_json(value, Scope::User, visit);
}

fn visit_string_map(
    values: &mut BTreeMap<String, Vec<String>>,
    is_header: bool,
    visit: &mut impl FnMut(&mut String),
) {
    for (key, values) in values {
        if is_header
            && matches!(
                key.to_ascii_lowercase().as_str(),
                "authorization"
                    | "proxy-authorization"
                    | "x-api-key"
                    | "cookie"
                    | "content-type"
                    | "accept"
                    | "anthropic-version"
                    | "anthropic-beta"
                    | "openai-beta"
            )
        {
            continue;
        }
        values.iter_mut().for_each(&mut *visit);
    }
}

pub(super) fn visit_invocation_content(
    input: &mut ProviderInvocationInput,
    mut visit: impl FnMut(&mut String),
) {
    // Exhaustive destructuring makes future additions explicitly choose a scope.
    let ProviderInvocationInput {
        operation: _,
        contract_version: _,
        profile: _,
        provider_instance_id: _,
        provider_code: _,
        protocol: _,
        model: _,
        previous_response_id: _,
        provider_config: _,
        required_capabilities: _,
        client_transport: _,
        trace_context: _,
        messages,
        system,
        request_context,
        tools,
        mcp_bindings,
        response_format,
        model_parameters,
        client_protocol_envelope,
        native_transport,
        run_context,
    } = input;
    // Routing/profile/capabilities, the host-supplied provider configuration (including
    // credentials), and trace correlation are control data, not model content.
    for message in messages {
        visit(&mut message.content);
        // name and tool_call_id identify the sender/call on the wire.
        for value in [&mut message.content_blocks, &mut message.tool_calls]
            .into_iter()
            .flatten()
        {
            visit_json(value, Scope::Item, &mut visit);
        }
    }
    for NativePromptBlock::Text { text, .. } in system {
        visit(text);
    }
    if let Some(reference) = &mut request_context.end_user_reference {
        visit(reference);
    }
    for tool in tools.iter_mut().chain(mcp_bindings) {
        visit_json(tool, Scope::Tool, &mut visit);
    }
    if let Some(format) = response_format {
        visit_json(format, Scope::Format, &mut visit);
    }
    visit_object(model_parameters.iter_mut(), Scope::Body, &mut visit);
    if let Some(envelope) = client_protocol_envelope {
        visit_object(envelope.body.iter_mut(), Scope::Body, &mut visit);
        if let Some(body) = envelope
            .source_request
            .as_mut()
            .and_then(|req| req.body.as_mut())
        {
            visit_json(body, Scope::Body, &mut visit);
        }
        visit_string_map(&mut envelope.query, false, &mut visit);
        visit_string_map(&mut envelope.headers, true, &mut visit);
    }
    if let Some(native) = native_transport {
        visit_json(&mut native.wire_body, Scope::Body, &mut visit);
    }
    for (key, value) in run_context {
        if matches!(
            key.as_str(),
            "native_inference_recovery"
                | "__host_verified_native_continuation"
                | "visible_internal_llm_media_tools"
                | "client_tool_progress_names"
        ) {
            continue; // sealed recovery carriers and tool dispatch identifiers
        }
        visit_json(value, Scope::User, &mut visit);
    }
}

pub(super) fn refresh_native_digest(input: &mut ProviderInvocationInput) -> anyhow::Result<()> {
    if let Some(transport) = &mut input.native_transport {
        let (digest, size) = canonical_json_digest_and_size(&transport.wire_body)?;
        transport.digest = digest;
        transport.size_bytes = size as u64;
    }
    Ok(())
}

/// Index final masked strings in one pass, only retaining the first snippet for a hit.
pub(super) struct FinalAuditSnippets(HashMap<String, Option<String>>);

impl FinalAuditSnippets {
    pub(super) fn new<'a>(states: impl Iterator<Item = &'a StageMaskState>) -> Self {
        Self(
            states
                .flat_map(|state| state.matches.keys().cloned().map(|key| (key, None)))
                .collect(),
        )
    }

    pub(super) fn observe(&mut self, final_text: &str) {
        if self.0.is_empty() || !final_text.contains("{{") {
            return;
        }
        for hit in placeholder_regex().find_iter(final_text) {
            let Some(slot @ None) = self.0.get_mut(hit.as_str()) else {
                continue;
            };
            let start = final_text[..hit.start()]
                .char_indices()
                .rev()
                .nth(23)
                .map_or(0, |(i, _)| i);
            let end = final_text[hit.end()..]
                .char_indices()
                .nth(24)
                .map_or(final_text.len(), |(i, _)| hit.end() + i);
            let left = if start > 0 { "…" } else { "" };
            let right = if end < final_text.len() { "…" } else { "" };
            *slot = Some(format!("{left}{}{right}", &final_text[start..end]));
        }
    }

    pub(super) fn populate(&self, state: &mut StageMaskState) {
        for entry in state.matches.values_mut() {
            if let Some(Some(snippet)) = self.0.get(&entry.placeholder) {
                entry.masked_snippet.clone_from(snippet);
            }
        }
    }
}
