use super::*;

/// Scope the client's reconnect identity to its authenticated application/key.
/// A missing client identity gets a fresh request identity (or the socket's
/// frozen fallback); different clients never share a provider socket by default.
pub(super) fn bind_responses_session_context(
    context: &mut Option<ProtocolContextEnvelope>,
    principal: &interface_runtime::ApplicationPrincipal,
    headers: &HeaderMap,
) -> Result<(),OpenAiRouteError> {
    use sha2::{Digest,Sha256};
    let mut parts=vec![principal.application_id().to_string(),principal.api_key_id().to_string(),principal.workspace_id().to_string()];
    for name in ["session-id","thread-id"] {
        let mut values=headers.get_all(name).iter();
        let value=values.next().map(|value|value.to_str()).transpose()
            .map_err(|_|openai_invalid_request(name,"session header must be valid text"))?;
        if values.next().is_some() || value.is_some_and(|v|v.is_empty() || v.len()>256) {
            return Err(openai_invalid_request(name,"session header must appear once and contain 1–256 bytes"));
        }
        parts.push(value.map(ToOwned::to_owned).unwrap_or_else(|| {
            if name=="session-id" {Uuid::now_v7().to_string()} else {String::new()}
        }));
    }
    let identity=format!("{:x}",Sha256::digest(serde_json::to_vec(&parts).expect("session identity serializes")));
    let envelope=context.get_or_insert_with(||ProtocolContextEnvelope {source_protocol:"openai_responses".into(),..Default::default()});
    envelope.headers.insert("session-id".into(),vec![identity]);
    Ok(())
}
