use super::*;

/// Scope the client's reconnect identity to its authenticated application/key.
/// A missing session-id uses a stable thread-id when provided. Headerless
/// continuations inherit the previous authenticated round's sealed identity.
pub(super) fn bind_responses_session_context(
    request: &mut control_plane::application_public_api::native::NativeRunRequest,
    principal: &interface_runtime::ApplicationPrincipal,
    headers: &HeaderMap,
    inherited_identity: Option<&str>,
) -> Result<(), OpenAiRouteError> {
    let identity = responses_session_identity(principal, headers, inherited_identity)?;
    request
        .metadata
        .set_provider_session_identity(Some(identity.clone()));
    let envelope =
        request
            .client_protocol_envelope
            .get_or_insert_with(|| ProtocolContextEnvelope {
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
        base16ct::HexDisplay(&Sha256::digest(
            serde_json::to_vec(&parts).expect("session identity serializes")
        ))
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

    fn principal() -> interface_runtime::ApplicationPrincipal {
        let workspace = Uuid::now_v7();
        interface_runtime::ApplicationPrincipal::new(
            Uuid::now_v7(),
            Uuid::now_v7(),
            workspace,
            domain::ActorContext::scoped(Uuid::now_v7(), workspace, "member", []),
        )
        .unwrap()
    }

    fn request() -> control_plane::application_public_api::native::NativeRunRequest {
        serde_json::from_value(json!({"query": "hello"})).unwrap()
    }

    #[test]
    fn responses_session_identity_reaches_native_input_and_headerless_continuation() {
        use control_plane::application_public_api::{
            mapping::ApplicationApiMappingConfig, native::NativeInputMapper,
        };
        let principal = principal();
        let mut headers = HeaderMap::new();
        headers.insert("session-id", "session-A".parse().unwrap());
        let mut first = request();
        bind_responses_session_context(&mut first, &principal, &headers, None).unwrap();
        let identity = first
            .metadata
            .provider_session_identity()
            .unwrap()
            .to_owned();
        assert_eq!(
            first.client_protocol_envelope.as_ref().unwrap().headers["session-id"],
            vec![identity.clone()]
        );
        // Removing protocol context simulates an LLM node which does not mount it.
        first.client_protocol_envelope = None;
        let mapped =
            NativeInputMapper::map(&first, &ApplicationApiMappingConfig::default_native()).unwrap();
        assert_eq!(
            mapped.node_input_payload["sys"]["provider_session_identity"],
            json!(identity)
        );
        let mut continuation = request();
        bind_responses_session_context(
            &mut continuation,
            &principal,
            &HeaderMap::new(),
            Some(&identity),
        )
        .unwrap();
        assert_eq!(
            continuation.metadata.provider_session_identity(),
            Some(identity.as_str())
        );
        let mut independent = request();
        let mut other = request();
        bind_responses_session_context(&mut independent, &principal, &HeaderMap::new(), None)
            .unwrap();
        bind_responses_session_context(&mut other, &principal, &HeaderMap::new(), None).unwrap();
        assert_ne!(
            independent.metadata.provider_session_identity(),
            other.metadata.provider_session_identity()
        );
    }

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
