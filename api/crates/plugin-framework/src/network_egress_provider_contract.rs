use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use url::Url;

pub use extension_contracts::{EgressAvailability, EgressDescriptor};

use crate::error::{FrameworkResult, PluginFrameworkError};

/// The only stdio operations exposed by a network egress provider runtime.
/// Health checks and worker lifecycle remain host-to-process concerns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "operation",
    content = "input",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum NetworkEgressProviderStdioRequest {
    SyncEgresses(SyncEgressesInput),
    AcquireHttpForwardProxy(AcquireHttpForwardProxyInput),
    ReleaseHttpForwardProxy(ReleaseHttpForwardProxyInput),
}

impl NetworkEgressProviderStdioRequest {
    pub fn validate(&self) -> FrameworkResult<()> {
        match self {
            Self::SyncEgresses(_) => Ok(()),
            Self::AcquireHttpForwardProxy(input) => {
                validate_non_empty(&input.provider_egress_key, "provider_egress_key")
            }
            Self::ReleaseHttpForwardProxy(input) => {
                validate_non_empty(&input.lease_id, "lease_id")?;
                validate_non_empty(&input.cleanup_token, "cleanup_token")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncEgressesInput {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcquireHttpForwardProxyInput {
    pub provider_egress_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseHttpForwardProxyInput {
    pub lease_id: String,
    /// An opaque lease capability rather than provider configuration or a credential.
    pub cleanup_token: String,
}

/// A typed success envelope. An operation name is always paired with its only
/// valid result shape, so third-party runtimes cannot return another operation's payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "operation",
    content = "result",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum NetworkEgressProviderStdioResponse {
    SyncEgresses(SyncEgressesResult),
    AcquireHttpForwardProxy(ForwardProxyLease),
    ReleaseHttpForwardProxy(CleanupReceipt),
}

impl NetworkEgressProviderStdioResponse {
    pub fn validate(&self) -> FrameworkResult<()> {
        match self {
            Self::SyncEgresses(result) => result.validate(),
            Self::AcquireHttpForwardProxy(lease) => lease.validate(),
            Self::ReleaseHttpForwardProxy(receipt) => {
                validate_non_empty(&receipt.lease_id, "lease_id")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncEgressesResult {
    pub egresses: Vec<EgressDescriptor>,
}

impl SyncEgressesResult {
    fn validate(&self) -> FrameworkResult<()> {
        let mut provider_egress_keys = BTreeSet::new();
        let mut previous_key: Option<&str> = None;

        for descriptor in &self.egresses {
            validate_egress_descriptor(descriptor)?;
            if !provider_egress_keys.insert(descriptor.provider_egress_key.as_str()) {
                return Err(PluginFrameworkError::invalid_provider_contract(
                    "sync_egresses result contains duplicate provider_egress_key",
                ));
            }
            if previous_key.is_some_and(|key| key >= descriptor.provider_egress_key.as_str()) {
                return Err(PluginFrameworkError::invalid_provider_contract(
                    "sync_egresses result must be sorted by provider_egress_key",
                ));
            }
            previous_key = Some(&descriptor.provider_egress_key);
        }

        Ok(())
    }
}

fn validate_egress_descriptor(descriptor: &EgressDescriptor) -> FrameworkResult<()> {
    validate_non_empty(&descriptor.provider_egress_key, "provider_egress_key")?;
    validate_non_empty(&descriptor.display_name, "display_name")?;
    if let Some(region) = &descriptor.region {
        validate_non_empty(region, "region")?;
    }
    if let Some(tags) = &descriptor.tags {
        for tag in tags {
            validate_non_empty(tag, "tags[]")?;
        }
    }
    Ok(())
}

/// An HTTP forward-proxy lease with no provider configuration or credentials.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForwardProxyLease {
    pub lease_id: String,
    pub http_proxy_url: String,
    pub cleanup_token: String,
    /// Nonzero Unix-epoch milliseconds; the Runner compares this value with its own clock.
    pub expires_at: u64,
}

impl ForwardProxyLease {
    fn validate(&self) -> FrameworkResult<()> {
        validate_non_empty(&self.lease_id, "lease_id")?;
        validate_http_proxy_url(&self.http_proxy_url)?;
        if self.expires_at == 0 {
            return Err(PluginFrameworkError::invalid_provider_contract(
                "expires_at must be a nonzero Unix-epoch milliseconds integer",
            ));
        }
        validate_non_empty(&self.cleanup_token, "cleanup_token")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CleanupReceipt {
    pub lease_id: String,
}

fn validate_non_empty(value: &str, field: &str) -> FrameworkResult<()> {
    if value.trim().is_empty() {
        return Err(PluginFrameworkError::invalid_provider_contract(format!(
            "{field} must not be empty"
        )));
    }
    Ok(())
}

fn validate_http_proxy_url(value: &str) -> FrameworkResult<()> {
    let url = Url::parse(value).map_err(|_| {
        PluginFrameworkError::invalid_provider_contract(
            "http_proxy_url must be a valid http://host:port URL",
        )
    })?;
    if url.scheme() != "http"
        || url.host_str().is_none()
        || url.port().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(PluginFrameworkError::invalid_provider_contract(
            "http_proxy_url must be a credential-free http://host:port URL",
        ));
    }
    Ok(())
}
