use anyhow::Result;

use crate::ports::{
    NetworkEgressRepository, NetworkEgressSecretMaterial, NetworkEgressSecretResolver,
};

/// Resolves registry-owned secret material only for provisioning an already-selected Runner
/// provider. HTTP routes and audit paths do not receive this value.
pub struct ProviderRegistryNetworkEgressSecretResolver<R> {
    repository: R,
    master_key: String,
}

impl<R> ProviderRegistryNetworkEgressSecretResolver<R> {
    pub fn new(repository: R, master_key: String) -> Self {
        Self {
            repository,
            master_key,
        }
    }
}

#[async_trait::async_trait]
impl<R> NetworkEgressSecretResolver for ProviderRegistryNetworkEgressSecretResolver<R>
where
    R: NetworkEgressRepository,
{
    async fn resolve_for_runner(
        &self,
        provider: &domain::NetworkEgressProviderRecord,
    ) -> Result<Option<NetworkEgressSecretMaterial>> {
        self.repository
            .resolve_network_egress_provider_secret_json(
                provider.id,
                &provider.secret_ref,
                &self.master_key,
            )
            .await
            .map(|secret_json| {
                secret_json.map(|secret_json| NetworkEgressSecretMaterial {
                    secret_ref: provider.secret_ref.clone(),
                    secret_json,
                })
            })
    }
}

#[cfg(test)]
mod tests {
    use crate::ports::NetworkEgressSecretMaterial;

    #[test]
    fn secret_material_debug_is_redacted() {
        let material = NetworkEgressSecretMaterial {
            secret_ref: "secret://system/network-egress/fixture".to_string(),
            secret_json: serde_json::json!({ "token": "do-not-log" }),
        };
        let debug = format!("{material:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("do-not-log"));
    }
}
