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

struct NetworkEgressHttpRequestLeaseReleaser(NetworkEgressExecutionScope);

struct NetworkEgressScopeRelease {
    runtime: ApiProviderRuntime,
    installation: domain::LocalPluginInstallationRecord,
}

impl NetworkEgressScopeRelease {
    async fn release(self) -> Result<()> {
        self.runtime
            .release_network_egress_http_forward_proxy(&self.installation)
            .await
            .context("configured network egress provider did not release its proxy lease")
    }

    fn release_after_cancellation(self) {
        let installation_id = self.installation.id;
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                handle.spawn(async move {
                    if self.release().await.is_err() {
                        tracing::warn!(
                            %installation_id,
                            "network egress lease release after cancellation failed"
                        );
                    }
                });
            }
            Err(_) => tracing::warn!(
                %installation_id,
                "network egress lease was dropped outside a Tokio runtime"
            ),
        }
    }
}

#[async_trait::async_trait]
impl orchestration_runtime::execution_engine::HttpRequestClientLeaseReleaser
    for NetworkEgressHttpRequestLeaseReleaser
{
    async fn release(self: Box<Self>) -> Result<()> {
        self.0.release().await
    }
}

/// Resolves a configured Network Center route to one short-lived HTTP client. `None` is the
/// only direct-path result: it means no enabled route matched the closed consumer selector.
#[derive(Clone)]
pub struct NetworkEgressHttpClientResolver {
    store: MainDurableStore,
    runtime: ApiProviderRuntime,
    provider_secret_master_key: String,
    node_id: String,
}

/// Owns one acquired lease until the host has completed its consumer operation.
/// Consumers receive only derived client/context values and never the lease capability.
pub struct NetworkEgressExecutionScope {
    client: Client,
    http_proxy_url: String,
    expires_at: u64,
    release: Option<NetworkEgressScopeRelease>,
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

    pub async fn acquire(
        &self,
        workspace_id: Uuid,
        selector: domain::NetworkEgressConsumerSelector,
    ) -> Result<Option<NetworkEgressExecutionScope>> {
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
        let release = NetworkEgressScopeRelease {
            runtime: self.runtime.clone(),
            installation,
        };
        let client = match Client::builder()
            .proxy(
                Proxy::all(&forward_proxy.http_proxy_url)
                    .context("network egress provider returned an invalid HTTP proxy URL")?,
            )
            .build()
            .context("failed to construct routed HTTP client")
        {
            Ok(client) => client,
            Err(error) => {
                if let Err(release_error) = release.release().await {
                    return Err(error.context(format!(
                        "also failed to release the acquired network egress lease: {release_error}"
                    )));
                }
                return Err(error);
            }
        };

        Ok(Some(NetworkEgressExecutionScope {
            client,
            http_proxy_url: forward_proxy.http_proxy_url,
            expires_at: forward_proxy.expires_at,
            release: Some(release),
        }))
    }
}

impl NetworkEgressExecutionScope {
    pub fn http_client(&self) -> &Client {
        &self.client
    }

    pub fn provider_invocation_context(
        &self,
    ) -> plugin_framework::provider_contract::ProviderNetworkEgressContext {
        plugin_framework::provider_contract::ProviderNetworkEgressContext {
            mode: plugin_framework::provider_contract::ProviderNetworkEgressMode::RequiredHttpProxy,
            http_proxy_url: self.http_proxy_url.clone(),
            expires_at: self.expires_at.to_string(),
            required: true,
        }
    }

    pub async fn into_http_request_client_lease(
        self,
        timeout: std::time::Duration,
        verify_ssl: bool,
    ) -> Result<orchestration_runtime::execution_engine::HttpRequestClientLease> {
        let client = match Client::builder()
            .timeout(timeout)
            .danger_accept_invalid_certs(!verify_ssl)
            .proxy(
                Proxy::all(&self.http_proxy_url)
                    .context("network egress provider returned an invalid HTTP proxy URL")?,
            )
            .build()
            .context("failed to construct routed HTTP client")
        {
            Ok(client) => client,
            Err(error) => {
                let release = self.release().await;
                return match release {
                    Ok(()) => Err(error),
                    Err(release_error) => Err(error.context(format!(
                        "also failed to release the acquired network egress lease: {release_error}"
                    ))),
                };
            }
        };
        Ok(
            orchestration_runtime::execution_engine::HttpRequestClientLease::new(
                client,
                Box::new(NetworkEgressHttpRequestLeaseReleaser(self)),
            ),
        )
    }

    pub async fn release(mut self) -> Result<()> {
        let release = self
            .release
            .take()
            .context("network egress scope was already released")?;
        release.release().await
    }
}

impl Drop for NetworkEgressExecutionScope {
    fn drop(&mut self) {
        if let Some(release) = self.release.take() {
            release.release_after_cancellation();
        }
    }
}
