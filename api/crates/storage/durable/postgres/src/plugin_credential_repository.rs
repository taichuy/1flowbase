use crate::secret_crypto::{decrypt_secret_json_with_aad, encrypt_secret_json_with_aad};
use async_trait::async_trait;
use control_plane_contracts::ports::PluginCredentialRepository;
use extension_contracts::{
    PluginCredentialBinding, PluginCredentialError, PluginCredentialRequest,
    PluginCredentialResponse,
};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

/// Inject only behind the control-plane authority facade, never directly into Runtime Host.
pub struct PgPluginCredentialRepository {
    pool: PgPool,
    master_key: String,
}
impl PgPluginCredentialRepository {
    pub fn new(pool: PgPool, master_key: impl Into<String>) -> anyhow::Result<Self> {
        let master_key = master_key.into();
        anyhow::ensure!(
            !master_key.is_empty(),
            "plugin_credential_master_key_missing"
        );
        Ok(Self { pool, master_key })
    }
}
fn storage_error(_: impl std::fmt::Display) -> PluginCredentialError {
    PluginCredentialError::new("plugin_credential_storage_unavailable")
}
fn associated_data(
    binding: &PluginCredentialBinding,
    credential_id: &str,
) -> Result<Vec<u8>, PluginCredentialError> {
    // JSON tuple encoding is unambiguous even when user IDs contain separators.
    serde_json::to_vec(&(
        "plugin_credential/v1",
        &binding.publisher_namespace,
        &binding.plugin_code,
        &binding.scope_id,
        credential_id,
    ))
    .map_err(storage_error)
}
#[async_trait]
impl PluginCredentialRepository for PgPluginCredentialRepository {
    async fn execute_plugin_credential(
        &self,
        binding: &PluginCredentialBinding,
        request: &PluginCredentialRequest,
    ) -> Result<PluginCredentialResponse, PluginCredentialError> {
        request.validate()?;
        let scope_id = Uuid::parse_str(&binding.scope_id)
            .map_err(|_| PluginCredentialError::new("plugin_credential_scope_invalid"))?;
        if binding.contribution_id.is_empty()
            || binding.publisher_namespace.is_empty()
            || binding.plugin_code.is_empty()
            || binding.plugin_version.is_empty()
            || Uuid::parse_str(&binding.installation_id).is_err()
        {
            return Err(PluginCredentialError::new(
                "plugin_credential_binding_invalid",
            ));
        }
        if binding.deadline_unix_ms
            <= (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64
        {
            return Err(PluginCredentialError::new("plugin_credential_deadline"));
        }
        let id = request.credential_id();
        let aad = associated_data(binding, id)?;
        let value = match request {
            PluginCredentialRequest::Put { value, .. } => {
                let encrypted = encrypt_secret_json_with_aad(value, &self.master_key, &aad)
                    .map_err(storage_error)?;
                sqlx::query("insert into plugin_credentials (publisher_namespace, plugin_code, scope_id, credential_id, encrypted_value) values ($1,$2,$3,$4,$5) on conflict (publisher_namespace,plugin_code,scope_id,credential_id) do update set encrypted_value=excluded.encrypted_value, updated_at=now()")
                    .bind(&binding.publisher_namespace).bind(&binding.plugin_code).bind(scope_id).bind(id).bind(encrypted).execute(&self.pool).await.map_err(storage_error)?;
                None
            }
            PluginCredentialRequest::Get { .. } => {
                let encrypted: Option<Value> = sqlx::query_scalar("select encrypted_value from plugin_credentials where publisher_namespace=$1 and plugin_code=$2 and scope_id=$3 and credential_id=$4")
                    .bind(&binding.publisher_namespace).bind(&binding.plugin_code).bind(scope_id).bind(id).fetch_optional(&self.pool).await.map_err(storage_error)?;
                encrypted
                    .map(|value| {
                        decrypt_secret_json_with_aad(&value, &self.master_key, &aad)
                            .map_err(storage_error)
                    })
                    .transpose()?
            }
            PluginCredentialRequest::Delete { .. } => {
                sqlx::query("delete from plugin_credentials where publisher_namespace=$1 and plugin_code=$2 and scope_id=$3 and credential_id=$4")
                    .bind(&binding.publisher_namespace).bind(&binding.plugin_code).bind(scope_id).bind(id).execute(&self.pool).await.map_err(storage_error)?;
                None
            }
        };
        Ok(PluginCredentialResponse { value })
    }
}

#[cfg(test)]
#[path = "_tests/plugin/plugin_credential_crypto_tests.rs"]
mod tests;
