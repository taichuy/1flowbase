pub use api_host_kernel::network_egress_client::*;

#[cfg(test)]
use anyhow::Result;
#[cfg(test)]
use api_host_kernel::network_egress_client::static_proxy_execution_scope_for_test_support as static_proxy_execution_scope;
#[cfg(test)]
use uuid::Uuid;
#[cfg(test)]
#[path = "_tests/network_proxy/mod.rs"]
mod network_proxy;

#[cfg(test)]
use crate::provider_runtime::ApiProviderRuntime;
