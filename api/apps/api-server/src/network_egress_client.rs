use anyhow::{Context, Result};
use control_plane::{
    network_egress_pool::NetworkEgressPoolService,
    network_egress_route::NetworkEgressRouteService,
    network_egress_secret::ProviderRegistryNetworkEgressSecretResolver,
    ports::{NetworkEgressRepository, NetworkEgressSecretResolver, PluginRepository},
};
use reqwest::{Client, Proxy};
use storage_durable::MainDurableStore;
use uuid::Uuid;

use crate::provider_runtime::ApiProviderRuntime;

/// Resolves a configured Network Center route to one short-lived HTTP client. `None` is the
/// only direct-path result: it means no enabled route matched the closed consumer selector.
#[derive(Clone)]
pub struct NetworkEgressHttpClientResolver {
    store: MainDurableStore,
    runtime: ApiProviderRuntime,
    provider_secret_master_key: String,
    node_id: String,
}

pub struct NetworkEgressHttpClientLease {
    client: Client,
    runtime: ApiProviderRuntime,
    installation: domain::LocalPluginInstallationRecord,
}

impl NetworkEgressHttpClientResolver {
    pub fn new(
        store: MainDurableStore,
        runtime: ApiProviderRuntime,
        provider_secret_master_key: impl Into<String>,
        node_id: impl Into<String>,
    ) -> Self {
        Self {
            store,
            runtime,
            provider_secret_master_key: provider_secret_master_key.into(),
            node_id: node_id.into(),
        }
    }

    pub async fn resolve(
        &self,
        workspace_id: Uuid,
        selector: domain::NetworkEgressConsumerSelector,
    ) -> Result<Option<NetworkEgressHttpClientLease>> {
        let route = NetworkEgressRouteService::new(self.store.clone())
            .resolve_enabled(workspace_id, &selector)
            .await?;
        let Some(route) = route else {
            return Ok(None);
        };

        let selected = NetworkEgressPoolService::new(self.store.clone())
            .select_healthy_first(route.pool_id)
            .await
            .context("configured network egress pool has no usable member")?;
        let provider =
            NetworkEgressRepository::get_network_egress_provider(&self.store, selected.provider_id)
                .await?
                .context("configured network egress provider is unavailable")?;
        let installation = PluginRepository::get_local_installation(
            &self.store,
            &self.node_id,
            provider.installation_id,
        )
        .await?
        .context("configured network egress provider is unavailable on this node")?;
        let secret = ProviderRegistryNetworkEgressSecretResolver::new(
            self.store.clone(),
            self.provider_secret_master_key.clone(),
        )
        .resolve_for_runner(&provider)
        .await?
        .context("configured network egress provider secret is unavailable")?;
        let forward_proxy = self
            .runtime
            .acquire_network_egress_http_forward_proxy(
                &installation,
                secret,
                &selected.provider_egress_key,
            )
            .await
            .context("configured network egress provider could not acquire a proxy lease")?;
        let client = Client::builder()
            .proxy(
                Proxy::all(&forward_proxy.http_proxy_url)
                    .context("network egress provider returned an invalid HTTP proxy URL")?,
            )
            .build()
            .context("failed to construct routed HTTP client")?;

        Ok(Some(NetworkEgressHttpClientLease {
            client,
            runtime: self.runtime.clone(),
            installation,
        }))
    }
}

impl NetworkEgressHttpClientLease {
    pub fn client(&self) -> &Client {
        &self.client
    }

    pub async fn release(self) -> Result<()> {
        self.runtime
            .release_network_egress_http_forward_proxy(&self.installation)
            .await
            .context("configured network egress provider did not release its proxy lease")
    }
}
