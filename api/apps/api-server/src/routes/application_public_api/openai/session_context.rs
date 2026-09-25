use super::*;

/// Scope the client's reconnect identity to its authenticated application/key.
/// A missing session-id uses a stable thread-id when provided. Headerless
/// continuations inherit the previous authenticated round's sealed identity.
pub(super) fn bind_responses_session_context(
    context: &mut Option<ProtocolContextEnvelope>,
    principal: &interface_runtime::ApplicationPrincipal,
    headers: &HeaderMap,
    inherited_identity: Option<&str>,
) -> Result<(), OpenAiRouteError> {
    let identity = responses_session_identity(principal, headers, inherited_identity)?;
    let envelope = context.get_or_insert_with(|| ProtocolContextEnvelope {
        source_protocol: "openai_responses".into(),
        ..Default::default()
    });
    envelope.headers.insert("session-id".into(), vec![identity]);
    Ok(())
}

pub(super) fn responses_session_identity(
    principal: &interface_runtime::ApplicationPrincipal,
    headers: &HeaderMap,
    inherited_identity: Option<&str>,
) -> Result<String, OpenAiRouteError> {
    use sha2::{Digest, Sha256};
    if !headers.contains_key("session-id") && !headers.contains_key("thread-id") {
        if let Some(identity) = inherited_identity {
            return Ok(identity.to_owned());
        }
    }
    let (session_id, thread_id) = client_session_parts(headers)?;
    let parts = vec![
        principal.application_id().to_string(),
        principal.api_key_id().to_string(),
        principal.workspace_id().to_string(),
        session_id,
        thread_id,
    ];
    let identity = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&parts).expect("session identity serializes"))
    );
    Ok(identity)
}

fn client_session_parts(headers: &HeaderMap) -> Result<(String, String), OpenAiRouteError> {
    let mut parts = Vec::with_capacity(2);
    for name in ["session-id", "thread-id"] {
        let mut values = headers.get_all(name).iter();
        let value = values
            .next()
            .map(|value| value.to_str())
            .transpose()
            .map_err(|_| openai_invalid_request(name, "session header must be valid text"))?;
        if values.next().is_some() || value.is_some_and(|v| v.is_empty() || v.len() > 256) {
            return Err(openai_invalid_request(
                name,
                "session header must appear once and contain 1–256 bytes",
            ));
        }
        parts.push(value.map(ToOwned::to_owned));
    }
    let thread_id = parts.pop().flatten().unwrap_or_default();
    let session_id = parts.pop().flatten().unwrap_or_else(|| {
        if thread_id.is_empty() {
            Uuid::now_v7().to_string()
        } else {
            thread_id.clone()
        }
    });
    Ok((session_id, thread_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_id_is_stable_fallback_but_headerless_requests_remain_isolated() {
        let mut headers = HeaderMap::new();
        headers.insert("thread-id", "thread-123".parse().unwrap());
        assert_eq!(
            client_session_parts(&headers).unwrap(),
            client_session_parts(&headers).unwrap()
        );
        assert_eq!(client_session_parts(&headers).unwrap().0, "thread-123");
        let empty = HeaderMap::new();
        assert_ne!(
            client_session_parts(&empty).unwrap(),
            client_session_parts(&empty).unwrap()
        );
    }
}
