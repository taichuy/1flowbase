use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use control_plane::network_egress_pool::NetworkEgressPoolSelection;
use reqwest::Client;
use serde::Deserialize;

use crate::{
    app_state::ApiState, network_egress_client::NetworkEgressHttpClientResolver,
    provider_runtime::ApiProviderRuntime,
};

const HTTP_PROBE_URLS: [&str; 2] = [
    "http://ip-api.com/json/?lang=en",
    "http://api64.ipify.org?format=json",
];
const HTTPS_PROBE_URL: &str = "https://api.ipify.org?format=json";
const CONNECTION_PROBE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct NetworkEgressConnectionProbeResult {
    pub status: domain::NetworkEgressPoolMemberProbeStatus,
    pub http_status: domain::NetworkEgressPoolMemberProbeStatus,
    pub https_status: domain::NetworkEgressPoolMemberProbeStatus,
    /// The latest real probe elapsed time. A request that cannot complete keeps the durable
    /// baseline at zero rather than leaving the operator-facing metric empty.
    pub latency_ms: i32,
    pub exit_ip: Option<String>,
    pub exit_region: Option<String>,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProbeObservation {
    exit_ip: String,
    exit_region: Option<String>,
    latency_ms: i32,
}

#[derive(Deserialize)]
struct IpApiResponse {
    status: String,
    query: Option<String>,
    #[serde(rename = "regionName")]
    region_name: Option<String>,
    region: Option<String>,
}

#[derive(Deserialize)]
struct IpifyResponse {
    ip: String,
}

/// Tests actual proxy forwarding, rather than only opening the proxy port. HTTP is probed first
/// with the same fallback strategy as Sub2API; HTTPS then verifies the CONNECT capability used by
/// model and runtime HTTP clients.
pub async fn test_network_egress_connection(
    state: &ApiState,
    selected: NetworkEgressPoolSelection,
) -> NetworkEgressConnectionProbeResult {
    let resolver = NetworkEgressHttpClientResolver::new(
        state.store.clone(),
        ApiProviderRuntime::new(state.provider_runtime.clone()),
        state.provider_secret_master_key.clone(),
        state.api_node_id.clone(),
    );
    let scope = match resolver
        .acquire_provider_egress(selected.provider_id, &selected.provider_egress_key)
        .await
    {
        Ok(Some(scope)) => scope,
        Ok(None) | Err(_) => {
            return failed_probe(
                domain::NetworkEgressPoolMemberProbeStatus::Failed,
                domain::NetworkEgressPoolMemberProbeStatus::NotTested,
                None,
                "proxy_unavailable",
            )
        }
    };

    let http_probe = probe_http_egress(scope.http_client()).await;
    let https_probe = if http_probe.is_ok() {
        probe_https_connect(scope.http_client()).await
    } else {
        Err(anyhow!("HTTP proxy forwarding failed"))
    };
    let release_error = scope.release().await.err();

    if let Some(error) = release_error {
        tracing::warn!(error = %error, "network egress connection test could not release proxy lease");
        return failed_probe(
            status_for_result(&http_probe),
            status_for_result(&https_probe),
            http_probe.ok(),
            "proxy_release_failed",
        );
    }

    match (http_probe, https_probe) {
        (Ok(http), Ok(https)) => NetworkEgressConnectionProbeResult {
            status: domain::NetworkEgressPoolMemberProbeStatus::Succeeded,
            http_status: domain::NetworkEgressPoolMemberProbeStatus::Succeeded,
            https_status: domain::NetworkEgressPoolMemberProbeStatus::Succeeded,
            latency_ms: https.latency_ms,
            exit_ip: Some(https.exit_ip),
            exit_region: http.exit_region,
            error_code: None,
        },
        (Ok(http), Err(error)) => {
            let error_code = classify_probe_error(&error, "https_connect_failed");
            tracing::warn!(error = %error, error_code, "network egress HTTPS CONNECT test failed");
            failed_probe(
                domain::NetworkEgressPoolMemberProbeStatus::Succeeded,
                domain::NetworkEgressPoolMemberProbeStatus::Failed,
                Some(http),
                error_code,
            )
        }
        (Err(error), _) => {
            let error_code = classify_probe_error(&error, "http_proxy_failed");
            tracing::warn!(error = %error, error_code, "network egress HTTP forwarding test failed");
            failed_probe(
                domain::NetworkEgressPoolMemberProbeStatus::Failed,
                domain::NetworkEgressPoolMemberProbeStatus::NotTested,
                None,
                error_code,
            )
        }
    }
}

fn failed_probe(
    http_status: domain::NetworkEgressPoolMemberProbeStatus,
    https_status: domain::NetworkEgressPoolMemberProbeStatus,
    http_observation: Option<ProbeObservation>,
    error_code: impl Into<String>,
) -> NetworkEgressConnectionProbeResult {
    NetworkEgressConnectionProbeResult {
        status: domain::NetworkEgressPoolMemberProbeStatus::Failed,
        http_status,
        https_status,
        latency_ms: http_observation
            .as_ref()
            .map(|probe| probe.latency_ms)
            .unwrap_or(0),
        exit_ip: http_observation.as_ref().map(|probe| probe.exit_ip.clone()),
        exit_region: http_observation.and_then(|probe| probe.exit_region),
        error_code: Some(error_code.into()),
    }
}

fn status_for_result(
    result: &Result<ProbeObservation>,
) -> domain::NetworkEgressPoolMemberProbeStatus {
    if result.is_ok() {
        domain::NetworkEgressPoolMemberProbeStatus::Succeeded
    } else {
        domain::NetworkEgressPoolMemberProbeStatus::Failed
    }
}

async fn probe_http_egress(client: &Client) -> Result<ProbeObservation> {
    let mut last_error = None;
    for url in HTTP_PROBE_URLS {
        match probe_ip_echo(client, url).await {
            Ok(observation) => return Ok(observation),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow!("no HTTP probe target configured")))
}

async fn probe_https_connect(client: &Client) -> Result<ProbeObservation> {
    probe_ip_echo(client, HTTPS_PROBE_URL).await
}

async fn probe_ip_echo(client: &Client, url: &str) -> Result<ProbeObservation> {
    let started = Instant::now();
    let response = client
        .get(url)
        .timeout(CONNECTION_PROBE_TIMEOUT)
        .send()
        .await?
        .error_for_status()?;
    let body = response.bytes().await?;
    let latency_ms = started.elapsed().as_millis().min(i32::MAX as u128) as i32;
    if url.contains("ip-api.com") {
        return parse_ip_api_probe(&body, latency_ms);
    }
    parse_ipify_probe(&body, latency_ms)
}

fn parse_ip_api_probe(body: &[u8], latency_ms: i32) -> Result<ProbeObservation> {
    let response: IpApiResponse = serde_json::from_slice(body)?;
    if response.status != "success" {
        return Err(anyhow!("IP echo service returned a failed status"));
    }
    let exit_ip = response
        .query
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("IP echo service did not return an address"))?;
    Ok(ProbeObservation {
        exit_ip,
        exit_region: response.region_name.or(response.region),
        latency_ms,
    })
}

fn parse_ipify_probe(body: &[u8], latency_ms: i32) -> Result<ProbeObservation> {
    let response: IpifyResponse = serde_json::from_slice(body)?;
    if response.ip.is_empty() {
        return Err(anyhow!("IP echo service did not return an address"));
    }
    Ok(ProbeObservation {
        exit_ip: response.ip,
        exit_region: None,
        latency_ms,
    })
}

fn classify_probe_error(error: &anyhow::Error, fallback: &'static str) -> &'static str {
    for cause in error.chain() {
        let Some(request_error) = cause.downcast_ref::<reqwest::Error>() else {
            continue;
        };
        if request_error.is_timeout() {
            return "proxy_timeout";
        }
        if request_error.status() == Some(reqwest::StatusCode::PROXY_AUTHENTICATION_REQUIRED) {
            return "proxy_authentication_failed";
        }
        if request_error
            .status()
            .is_some_and(|status| status.is_client_error())
        {
            return "proxy_request_rejected";
        }
    }
    fallback
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ip_api_exit_details_for_the_proxy_pool_projection() {
        let observation = parse_ip_api_probe(
            br#"{"status":"success","query":"198.51.100.10","regionName":"California"}"#,
            42,
        )
        .expect("IP API response must parse");

        assert_eq!(observation.exit_ip, "198.51.100.10");
        assert_eq!(observation.exit_region.as_deref(), Some("California"));
        assert_eq!(observation.latency_ms, 42);
    }

    #[test]
    fn preserves_http_egress_details_when_https_connect_fails() {
        let result = failed_probe(
            domain::NetworkEgressPoolMemberProbeStatus::Succeeded,
            domain::NetworkEgressPoolMemberProbeStatus::Failed,
            Some(ProbeObservation {
                exit_ip: "198.51.100.10".to_string(),
                exit_region: Some("California".to_string()),
                latency_ms: 42,
            }),
            "https_connect_failed",
        );

        assert_eq!(
            result.status,
            domain::NetworkEgressPoolMemberProbeStatus::Failed
        );
        assert_eq!(
            result.http_status,
            domain::NetworkEgressPoolMemberProbeStatus::Succeeded
        );
        assert_eq!(
            result.https_status,
            domain::NetworkEgressPoolMemberProbeStatus::Failed
        );
        assert_eq!(result.latency_ms, 42);
        assert_eq!(result.exit_region.as_deref(), Some("California"));
        assert_eq!(result.error_code.as_deref(), Some("https_connect_failed"));
    }

    #[test]
    fn stores_zero_latency_when_a_proxy_cannot_complete_a_probe() {
        let result = failed_probe(
            domain::NetworkEgressPoolMemberProbeStatus::Failed,
            domain::NetworkEgressPoolMemberProbeStatus::NotTested,
            None,
            "proxy_request_rejected",
        );

        assert_eq!(result.latency_ms, 0);
    }
}
