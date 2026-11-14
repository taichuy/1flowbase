//! One-time CSRF-admitted stream tickets, bound to the exact authenticated session and generation.
use crate::{app_state::ApiState, error_response::ApiError};
use axum::http::HeaderMap;
use control_plane::errors::ControlPlaneError as Error;
use control_plane::ports::CacheStore;
use rand::rngs::SysRng;
use rand_core::{Rng, UnwrapErr};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Serialize, Deserialize)]
pub(super) struct Ticket {
    pub session: String,
    pub actor: uuid::Uuid,
    pub origin: String,
    pub installation: uuid::Uuid,
    pub generation: String,
    pub epoch: u64,
    pub operation: String,
    pub payload: Value,
}
pub(crate) fn generation(state: &ApiState) -> Result<String, ApiError> {
    let boot = state
        .extension_boot_snapshot
        .as_ref()
        .ok_or(Error::Conflict("managed_service_generation_mismatch"))?;
    let registry = boot
        .interface_registry()
        .ok_or(Error::Conflict("managed_service_generation_mismatch"))?
        .snapshot();
    Ok(format!(
        "{}:{}",
        boot.generation_id(),
        registry.fingerprint().as_str()
    ))
}

pub(super) fn origin(headers: &HeaderMap) -> Result<String, ApiError> {
    let mut origins = headers.get_all("origin").iter();
    let origin = origins
        .next()
        .and_then(|value| value.to_str().ok())
        .ok_or(Error::PermissionDenied("managed_service_origin"))?;
    if origins.next().is_some() {
        return Err(Error::PermissionDenied("managed_service_origin").into());
    }
    let url =
        url::Url::parse(origin).map_err(|_| Error::PermissionDenied("managed_service_origin"))?;
    // Admission requires session-bound CSRF; upgrades must match the Origin stored
    // in that one-time ticket. Host describes the final proxy hop, not necessarily
    // the browser-facing authority (for example HTTPS ingress through frp).
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(Error::PermissionDenied("managed_service_origin").into());
    }
    Ok(url.origin().ascii_serialization())
}
fn key(token: &str) -> String {
    format!(
        "managed-service-stream-ticket:{}",
        control_plane::auth::hash_api_key_token(token)
    )
}
pub(super) async fn store(cache: &dyn CacheStore, ticket: &Ticket) -> Result<String, ApiError> {
    for _ in 0..3 {
        let mut bytes = [0; 32];
        UnwrapErr(SysRng).fill_bytes(&mut bytes);
        let token = bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        if cache
            .set_if_absent_json(
                &key(&token),
                serde_json::to_value(ticket)?,
                Some(time::Duration::seconds(60)),
            )
            .await?
        {
            return Ok(token);
        }
    }
    Err(Error::Conflict("managed_service_ticket").into())
}
pub(super) struct TicketBinding<'a> {
    pub session: &'a str,
    pub actor: uuid::Uuid,
    pub origin: &'a str,
    pub installation: uuid::Uuid,
    pub generation: &'a str,
    pub operation: &'a str,
}
pub(super) async fn claim(
    cache: &dyn CacheStore,
    token: &str,
    binding: TicketBinding<'_>,
) -> Result<Ticket, ApiError> {
    let TicketBinding {
        session,
        actor,
        origin,
        installation,
        generation,
        operation,
    } = binding;
    if token.len() != 64
        || !token
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(Error::PermissionDenied("managed_service_ticket").into());
    }
    let ticket: Ticket = serde_json::from_value(
        cache
            .get_json(&key(token))
            .await?
            .ok_or(Error::PermissionDenied("managed_service_ticket"))?,
    )?;
    if ticket.session != session
        || ticket.actor != actor
        || ticket.origin != origin
        || ticket.installation != installation
        || ticket.generation != generation
        || ticket.operation != operation
        || ticket.epoch != super::stream::epoch(installation)
    {
        return Err(Error::PermissionDenied("managed_service_ticket").into());
    }
    if !cache
        .set_if_absent_json(
            &format!("{}:claimed", key(token)),
            Value::Bool(true),
            Some(time::Duration::minutes(2)),
        )
        .await?
    {
        return Err(Error::PermissionDenied("managed_service_ticket_replayed").into());
    }
    cache.delete(&key(token)).await?;
    Ok(ticket)
}
