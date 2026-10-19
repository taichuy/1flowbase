//! Host-owned admission and configuration for managed external identity verifiers.
mod registration;
pub(crate) use registration::registry_for;

use crate::{
    app_state::ApiState, error_response::ApiError, extension_bus::ManagedWorkspaceSnapshot,
};
use anyhow::Context;
use control_plane::{errors::ControlPlaneError as Error, ports::AuthenticationSecretCodec};
use extension_contracts::{
    AuthenticationProviderDescriptor, AuthenticationRequest, AuthenticationResponse,
    AUTHENTICATION_POINT,
};
use runtime_core::runtime_backend::RuntimeExecutionPrincipal;
use uuid::Uuid;

pub(crate) struct AuthenticationProvider {
    pub descriptor: AuthenticationProviderDescriptor,
    pub installation_id: Uuid,
    pub fingerprint: String,
}

pub(crate) fn callback_url() -> Result<String, ApiError> {
    let origin = std::env::var("API_AUTH_PUBLIC_ORIGIN")
        .map_err(|_| Error::InvalidInput("authentication_public_origin"))?;
    let url = url::Url::parse(&origin)
        .map_err(|_| Error::InvalidInput("authentication_public_origin"))?;
    if !matches!(url.scheme(), "https" | "http")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
        || (url.scheme() == "http"
            && !matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")))
    {
        return Err(Error::InvalidInput("authentication_public_origin").into());
    }
    Ok(format!(
        "{}/api/public/auth/external/callback",
        url.origin().ascii_serialization()
    ))
}

fn fingerprint(
    snapshot: &ManagedWorkspaceSnapshot,
    contribution: &extension_contracts::ContributionId,
) -> Result<String, ApiError> {
    let binding = snapshot
        .bindings
        .get(contribution)
        .ok_or(Error::PluginUnavailable)?;
    let identity = binding.handle.identity();
    Ok(format!(
        "{}:{}:{}",
        identity.artifact_fingerprint().as_str(),
        identity.binding_fingerprint().as_str(),
        binding.handle.generation()
    ))
}

pub(crate) async fn provider(
    state: &ApiState,
    auth_type: &str,
) -> Result<AuthenticationProvider, ApiError> {
    let composition = state.provider_runtime.managed_composition()?;
    let snapshot = composition
        .snapshot(domain::SYSTEM_SCOPE_ID)
        .await
        .ok_or(Error::PluginUnavailable)?;
    let declarations = registration::active_declarations(
        &state.store,
        &state.api_node_id,
        &state.provider_install_root,
    )
    .await?;
    let (installation_id, descriptor) = declarations
        .into_iter()
        .find(|(_, descriptor)| descriptor.auth_type == auth_type)
        .ok_or(Error::NotFound("auth_provider"))?;
    let binding = snapshot
        .bindings
        .get(&descriptor.contribution_id)
        .ok_or(Error::PluginUnavailable)?;
    if binding.descriptor.point_id.as_str() != AUTHENTICATION_POINT
        || binding.handle.identity().installation_id().as_str() != installation_id.to_string()
    {
        return Err(Error::PluginUnavailable.into());
    }
    let fingerprint = fingerprint(&snapshot, &descriptor.contribution_id)?;
    Ok(AuthenticationProvider {
        descriptor,
        installation_id,
        fingerprint,
    })
}

impl AuthenticationProvider {
    /// Final authority barrier after every verifier/eligibility worker call. The
    /// Host holds this lease until the identity/session business effect finishes.
    pub(crate) async fn authorize_completion(
        &self,
        state: &ApiState,
        entry: &domain::LoginEntryRecord,
    ) -> Result<Box<dyn control_plane::ports::ContributionAuthorityLease>, ApiError> {
        let current = provider(state, &entry.auth_type).await?;
        if current.installation_id != self.installation_id
            || current.fingerprint != self.fingerprint
            || current.descriptor.contribution_id != self.descriptor.contribution_id
        {
            return Err(Error::PermissionDenied("authentication_provider_changed").into());
        }
        let composition = state.provider_runtime.managed_composition()?;
        let snapshot = composition
            .snapshot(domain::SYSTEM_SCOPE_ID)
            .await
            .ok_or(Error::PluginUnavailable)?;
        if fingerprint(&snapshot, &self.descriptor.contribution_id)? != self.fingerprint {
            return Err(Error::PermissionDenied("authentication_provider_changed").into());
        }
        composition
            .authorize_snapshot_binding(
                &snapshot,
                domain::SYSTEM_SCOPE_ID,
                &self.descriptor.contribution_id,
            )
            .await
            .map_err(|_| Error::PermissionDenied("authentication_provider_changed").into())
    }

    pub(crate) async fn invoke(
        &self,
        state: &ApiState,
        entry: &domain::LoginEntryRecord,
        request: AuthenticationRequest,
    ) -> Result<AuthenticationResponse, ApiError> {
        // Re-resolve exact bytes and live authority. A callback cannot migrate to another install/generation.
        let current = provider(state, &entry.auth_type).await?;
        if !entry.enabled
            || current.installation_id != self.installation_id
            || current.fingerprint != self.fingerprint
            || current.descriptor.contribution_id != self.descriptor.contribution_id
        {
            return Err(Error::PermissionDenied("authentication_provider_changed").into());
        }
        let composition = state.provider_runtime.managed_composition()?;
        let snapshot = composition
            .snapshot(domain::SYSTEM_SCOPE_ID)
            .await
            .ok_or(Error::PluginUnavailable)?;
        if fingerprint(&snapshot, &self.descriptor.contribution_id)? != self.fingerprint {
            return Err(Error::PermissionDenied("authentication_provider_changed").into());
        }
        let mut config = entry
            .options
            .get("extension_config")
            .and_then(serde_json::Value::as_object)
            .cloned()
            .unwrap_or_default();
        if let Some(envelope) = entry.options.get("authentication_secret_envelope") {
            let secrets = storage_durable_postgres::AuthenticationSecretCipher::new(
                &state.provider_secret_master_key,
            )
            .open(entry.connection_id, envelope)?;
            config.extend(
                secrets
                    .as_object()
                    .context("authentication secret configuration invalid")?
                    .clone(),
            );
        }
        config.insert(
            "callback_url".into(),
            serde_json::Value::String(callback_url()?),
        );
        let response = composition
            .execute_snapshot(
                snapshot,
                domain::SYSTEM_SCOPE_ID,
                &self.descriptor.contribution_id,
                RuntimeExecutionPrincipal {
                    caller: None,
                    workspace_id: domain::SYSTEM_SCOPE_ID.to_string(),
                    actor_id: None,
                    deadline_unix_ms: i64::MAX,
                },
                serde_json::Value::Object(config),
                serde_json::to_value(request)?,
            )
            .await
            .map_err(|_| Error::PermissionDenied("external_authentication_failed"))?;
        serde_json::from_value(response)
            .map_err(|_| Error::PermissionDenied("external_authentication_response").into())
    }
}
