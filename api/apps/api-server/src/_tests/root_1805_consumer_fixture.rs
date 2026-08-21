//! Narrow Root #1805 consumer authenticity target.
//!
//! Invoke with:
//! `cargo test -p api-server --lib --features root-1805-consumer-fixture root_1805_`
//!
//! It intentionally includes only the authenticated API state support plus the persisted
//! Route -> Pool -> Provider fixture. Keeping this target separate prevents unrelated API-server
//! unit modules from making the Root QA evidence un-runnable on a constrained fresh build.

mod network_egress_fixture;
#[path = "support/mod.rs"]
pub(crate) mod support;

use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    sync::Arc,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use control_plane::ports::ProviderRuntimePort;
use domain::{
    PluginAvailabilityStatus, PluginDesiredState, PluginInstallationRecord, PluginRuntimeStatus,
    PluginVerificationStatus,
};
use plugin_framework::provider_contract::ProviderInvocationInput;
use plugin_runner::{
    capability_host::CapabilityHost, data_source_host::DataSourceHost, provider_host::ProviderHost,
};
use serde_json::{Map, json};
use time::OffsetDateTime;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::provider_runtime::{ApiProviderRuntime, ApiRuntimeServices};

use self::network_egress_fixture::seed_network_egress_resolver;

/// The orchestration runtime asks this bridge for a client at node execution time. It never owns
/// the proxy URL, the lease id, cleanup material, or the egress Provider secret.
#[derive(Clone)]
struct ResolverBackedHttpNodeInvoker {
    runtime: ApiProviderRuntime,
    workspace_id: Uuid,
}

#[async_trait::async_trait]
impl orchestration_runtime::execution_engine::ProviderInvoker for ResolverBackedHttpNodeInvoker {
    async fn invoke_llm(
        &self,
        _runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        _input: ProviderInvocationInput,
    ) -> anyhow::Result<orchestration_runtime::execution_engine::ProviderInvocationOutput> {
        anyhow::bail!("the Network Center HTTP fixture has no LLM node")
    }

    async fn acquire_http_node_client(
        &self,
        timeout: Duration,
        verify_ssl: bool,
    ) -> anyhow::Result<Option<orchestration_runtime::execution_engine::HttpRequestClientLease>>
    {
        ProviderRuntimePort::acquire_http_node_client(
            &self.runtime,
            self.workspace_id,
            timeout,
            verify_ssl,
        )
        .await
    }
}

fn network_egress_runtime_services() -> Arc<ApiRuntimeServices> {
    Arc::new(
        ApiRuntimeServices::new_without_model_provider_extension_graph_for_tests(
            Arc::new(RwLock::new(ProviderHost::default())),
            Arc::new(RwLock::new(CapabilityHost::default())),
            Arc::new(RwLock::new(DataSourceHost::default())),
        ),
    )
}

struct TempProviderPackage {
    root: PathBuf,
}

impl TempProviderPackage {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("api-root-1805-provider-{nonce}"));
        fs::create_dir_all(&root).expect("provider fixture directory must be created");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write(&self, relative_path: &str, content: &str) {
        let path = self.root.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("provider fixture parent must be created");
        }
        fs::write(path, content).expect("provider fixture file must be written");
    }
}

impl Drop for TempProviderPackage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn write_network_egress_handoff_provider_package(
    package: &TempProviderPackage,
    proxy_target: &str,
    wire_capture_path: Option<&Path>,
) {
    package.write(
        "manifest.yaml",
        r#"manifest_version: 1
plugin_id: fixture_provider@0.1.0
version: 0.1.0
publisher_namespace: 1flowbase
vendor: 1flowbase
display_name: Fixture Provider
description: Fixture provider
source_kind: uploaded
trust_level: checksum_only
consumption_kind: runtime_extension
execution_mode: process_per_call
slot_codes:
  - model_provider
binding_targets:
  - workspace
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.provider/v2
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: none
  secrets: provider_instance_only
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/fixture_provider
  capabilities:
    - network_egress_handoff/v1
  limits:
    timeout_ms: 30000
node_contributions: []
"#,
    );
    package.write(
        "provider/fixture_provider.yaml",
        r#"provider_code: fixture_provider
display_name: Fixture Provider
protocol: openai_compatible
model_discovery: static
config_schema:
  - key: base_url
    type: string
    required: true
  - key: api_key
    type: secret
    required: true
"#,
    );
    package.write(
        "i18n/en_US.json",
        r#"{ "plugin": { "label": "Fixture Provider" } }"#,
    );
    let wire_capture = wire_capture_path.map_or_else(String::new, |path| {
        format!("printf '%s' \"$payload\" > '{}'\n", path.display())
    });
    package.write(
        "bin/fixture_provider",
        &format!(
            r#"#!/usr/bin/env bash
set -euo pipefail
payload="$(cat)"
{wire_capture}proxy_url="$(printf '%s' "$payload" | sed -n 's/.*"http_proxy_url":"\([^"]*\)".*/\1/p')"
if printf '%s' "$payload" | grep -F '"cleanup_token"' >/dev/null; then exit 1; fi
if test -n "$proxy_url"; then
  curl --fail --silent --show-error --proxy "$proxy_url" '{proxy_target}' >/dev/null
else
  curl --fail --silent --show-error --noproxy '*' '{proxy_target}' >/dev/null
fi
printf '%s\n' '{{"type":"text_delta","delta":"proxied"}}'
printf '%s\n' '{{"type":"result","result":{{"final_content":"proxied","finish_reason":"stop"}}}}'
"#,
        ),
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let path = package.path().join("bin/fixture_provider");
        let mut permissions = fs::metadata(&path)
            .expect("provider fixture executable metadata must be readable")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("provider fixture must be executable");
    }
}

fn spawn_request_proxy(expected_request_count: usize) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("fixture proxy must bind");
    let proxy_url = format!("http://{}", listener.local_addr().expect("proxy address"));
    let request = thread::spawn(move || {
        (0..expected_request_count)
            .map(|_| {
                let (mut stream, _) = listener.accept().expect("fixture proxy must accept");
                let mut bytes = [0_u8; 4096];
                let read = stream
                    .read(&mut bytes)
                    .expect("fixture proxy must read request");
                stream
                    .write_all(
                        b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok",
                    )
                    .expect("fixture proxy must respond");
                String::from_utf8_lossy(&bytes[..read]).to_string()
            })
            .collect()
    });
    (proxy_url, request)
}

fn fixture_installation(package: &TempProviderPackage) -> domain::LocalPluginInstallationRecord {
    let now = OffsetDateTime::now_utc();
    let installation = PluginInstallationRecord {
        id: Uuid::now_v7(),
        scope_id: domain::SYSTEM_SCOPE_ID,
        category: domain::ExtensionCategory::RuntimeExtensions,
        organization: "test".to_string(),
        provider_code: "fixture_provider".to_string(),
        plugin_id: "fixture_provider@0.1.0".to_string(),
        plugin_version: "0.1.0".to_string(),
        contract_version: "1flowbase.provider/v2".to_string(),
        protocol: "openai_compatible".to_string(),
        display_name: "Fixture Provider".to_string(),
        source_kind: "uploaded".to_string(),
        trust_level: "checksum_only".to_string(),
        verification_status: PluginVerificationStatus::Valid,
        desired_state: PluginDesiredState::ActiveRequested,
        expected_checksum: None,
        signature_status: domain::ExtensionSignatureStatus::Missing,
        signature_algorithm: None,
        signing_key_id: None,
        legacy_manifest_compatibility: None,
        metadata_json: json!({}),
        is_system_reserved: false,
        created_by: Uuid::now_v7(),
        updated_by: None,
        created_at: now,
        updated_at: now,
    };
    domain::LocalPluginInstallationRecord {
        artifact: domain::PluginArtifactInstanceRecord {
            node_id: "test-node".to_string(),
            installation_id: installation.id,
            local_version: Some("0.1.0".to_string()),
            local_checksum: None,
            local_path: Some(package.path().display().to_string()),
            package_path: None,
            manifest_fingerprint: None,
            artifact_status: domain::PluginArtifactInstanceStatus::Ready,
            runtime_status: PluginRuntimeStatus::Active,
            availability_status: PluginAvailabilityStatus::Available,
            checked_at: now,
            last_error: None,
            is_current: false,
        },
        installation,
    }
}

fn root_1805_http_node(target: &str) -> orchestration_runtime::compiled_plan::CompiledNode {
    orchestration_runtime::compiled_plan::CompiledNode {
        node_id: "network-egress-http".to_string(),
        node_type: "http_request".to_string(),
        alias: "Network Egress HTTP".to_string(),
        container_id: None,
        dependency_node_ids: Vec::new(),
        downstream_node_ids: Vec::new(),
        bindings: BTreeMap::new(),
        outputs: Vec::new(),
        config: json!({
            "method": "GET",
            "url": target,
            "body_type": "none",
            "timeout_ms": 5_000,
            "verify_ssl": true,
        }),
        plugin_runtime: None,
        llm_runtime: None,
        code_runtime: None,
    }
}

#[tokio::test]
async fn root_1805_model_provider_route_pool_provider_lease_reaches_fake_proxy_without_host_secrets()
 {
    // The Host probes the acquired loopback endpoint before handing it to the model worker.
    let (proxy_url, proxy_requests) = spawn_request_proxy(2);
    let proxy_target = "http://model-provider-origin.invalid/route-pool-provider";
    let package = TempProviderPackage::new();
    let wire_capture = package.path().join("provider-wire.json");
    write_network_egress_handoff_provider_package(&package, proxy_target, Some(&wire_capture));
    let (state, _) = support::test_api_state_with_database_url().await;
    let instance_id = Uuid::now_v7();
    let base_runtime = ApiProviderRuntime::new(network_egress_runtime_services());
    let (resolver, _egress_package) = seed_network_egress_resolver(
        &state,
        &proxy_url,
        domain::NetworkEgressConsumerSelector::ModelProviderInstance { instance_id },
        base_runtime.clone(),
    )
    .await;
    let runtime = base_runtime.with_network_egress(Arc::new(resolver));
    let output = ProviderRuntimePort::invoke_stream_with_network_egress(
        &runtime,
        &fixture_installation(&package),
        ProviderInvocationInput {
            provider_instance_id: instance_id.to_string(),
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            model: "fixture_chat".to_string(),
            provider_config: json!({"api_key": "model-provider-secret"}),
            ..ProviderInvocationInput::default()
        },
        None,
        state.bootstrap_workspace_id,
        domain::NetworkEgressConsumerSelector::ModelProviderInstance { instance_id },
    )
    .await
    .expect("the persisted model route must reach the fake proxy through the Host resolver");

    assert_eq!(output.result.final_content.as_deref(), Some("proxied"));
    let proxy_requests = proxy_requests
        .join()
        .expect("fake proxy must receive Host probe and model traffic");
    assert!(proxy_requests.iter().any(|request| {
        request.starts_with("GET http://model-provider-origin.invalid/route-pool-provider HTTP/1.1")
    }));
    let wire = fs::read_to_string(&wire_capture).expect("model worker must record its input wire");
    assert!(wire.contains("network_egress"));
    for forbidden in [
        "cleanup_token",
        "host-private",
        "fixture-lease",
        "egress-provider-secret",
        "fixture_egress",
    ] {
        assert!(!wire.contains(forbidden), "model wire leaked {forbidden}");
    }
}

#[tokio::test]
async fn root_1805_model_provider_no_route_keeps_direct_behavior_without_handoff() {
    let (origin, origin_request) = spawn_request_proxy(1);
    let package = TempProviderPackage::new();
    let wire_capture = package.path().join("provider-wire.json");
    write_network_egress_handoff_provider_package(
        &package,
        &format!("{origin}/model-direct"),
        Some(&wire_capture),
    );
    let (state, _) = support::test_api_state_with_database_url().await;
    let base_runtime = ApiProviderRuntime::new(network_egress_runtime_services());
    let resolver = crate::network_egress_client::NetworkEgressHttpClientResolver::new(
        state.store.clone(),
        base_runtime.clone(),
        state.provider_secret_master_key.clone(),
        state.api_node_id.clone(),
    );
    let runtime = base_runtime.with_network_egress(Arc::new(resolver));
    let instance_id = Uuid::now_v7();
    let output = ProviderRuntimePort::invoke_stream_with_network_egress(
        &runtime,
        &fixture_installation(&package),
        ProviderInvocationInput {
            provider_instance_id: instance_id.to_string(),
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            model: "fixture_chat".to_string(),
            provider_config: json!({"api_key": "model-provider-secret"}),
            ..ProviderInvocationInput::default()
        },
        None,
        state.bootstrap_workspace_id,
        domain::NetworkEgressConsumerSelector::ModelProviderInstance { instance_id },
    )
    .await
    .expect("no model route must retain ordinary provider invocation behavior");

    assert_eq!(output.result.final_content.as_deref(), Some("proxied"));
    assert!(
        origin_request
            .join()
            .expect("direct origin must receive the request")
            .into_iter()
            .any(|request| request.starts_with("GET /model-direct HTTP/1.1"))
    );
    let wire = fs::read_to_string(&wire_capture).expect("model worker must record its input wire");
    assert!(!wire.contains("network_egress"));
    assert!(!wire.contains("http_proxy_url"));
    assert!(!wire.contains("cleanup_token"));
    assert!(!wire.contains("egress-provider-secret"));
}

#[tokio::test]
async fn root_1805_http_node_route_pool_provider_lease_reaches_fake_proxy_without_host_secrets() {
    // HTTP-node acquisition probes before orchestration consumes the leased client.
    let (proxy_url, proxy_requests) = spawn_request_proxy(2);
    let (state, _) = support::test_api_state_with_database_url().await;
    let base_runtime = ApiProviderRuntime::new(network_egress_runtime_services());
    let (resolver, _egress_package) = seed_network_egress_resolver(
        &state,
        &proxy_url,
        domain::NetworkEgressConsumerSelector::HttpNodeDefault,
        base_runtime.clone(),
    )
    .await;
    let invoker = ResolverBackedHttpNodeInvoker {
        runtime: base_runtime.with_network_egress(Arc::new(resolver)),
        workspace_id: state.bootstrap_workspace_id,
    };
    let execution =
        orchestration_runtime::execution_engine::execute_http_request_node_with_provider_invoker(
            &root_1805_http_node("http://http-node-origin.invalid/route-pool-provider"),
            &Map::new(),
            &Map::new(),
            None,
            Some(&invoker),
        )
        .await
        .expect("HTTP execution must preserve its normal output envelope");

    assert_eq!(execution.error_payload, None);
    assert_eq!(execution.output_payload["status_code"], json!(200));
    let observable = serde_json::to_string(&json!({
        "output_payload": execution.output_payload,
        "error_payload": execution.error_payload,
        "metrics_payload": execution.metrics_payload,
        "debug_payload": execution.debug_payload,
    }))
    .expect("HTTP output envelope must serialize");
    for forbidden in [
        "cleanup_token",
        "host-private",
        "fixture-lease",
        "egress-provider-secret",
    ] {
        assert!(
            !observable.contains(forbidden),
            "HTTP output leaked {forbidden}"
        );
    }
    let proxy_requests = proxy_requests
        .join()
        .expect("fake proxy must receive Host probe and HTTP-node traffic");
    assert!(proxy_requests.iter().any(|request| {
        request.starts_with("GET http://http-node-origin.invalid/route-pool-provider HTTP/1.1")
    }));
}

#[tokio::test]
async fn root_1805_http_node_no_route_keeps_direct_behavior_without_host_lease() {
    let (origin, origin_request) = spawn_request_proxy(1);
    let (state, _) = support::test_api_state_with_database_url().await;
    let base_runtime = ApiProviderRuntime::new(network_egress_runtime_services());
    let resolver = crate::network_egress_client::NetworkEgressHttpClientResolver::new(
        state.store.clone(),
        base_runtime.clone(),
        state.provider_secret_master_key.clone(),
        state.api_node_id.clone(),
    );
    let invoker = ResolverBackedHttpNodeInvoker {
        runtime: base_runtime.with_network_egress(Arc::new(resolver)),
        workspace_id: state.bootstrap_workspace_id,
    };
    let execution =
        orchestration_runtime::execution_engine::execute_http_request_node_with_provider_invoker(
            &root_1805_http_node(&format!("{origin}/http-node-direct")),
            &Map::new(),
            &Map::new(),
            None,
            Some(&invoker),
        )
        .await
        .expect("HTTP execution must retain its normal result envelope");

    assert_eq!(execution.error_payload, None);
    assert_eq!(execution.output_payload["status_code"], json!(200));
    assert!(
        origin_request
            .join()
            .expect("direct origin must receive the request")
            .into_iter()
            .any(|request| request.starts_with("GET /http-node-direct HTTP/1.1"))
    );
    let observable = serde_json::to_string(&json!({
        "output_payload": execution.output_payload,
        "error_payload": execution.error_payload,
        "metrics_payload": execution.metrics_payload,
        "debug_payload": execution.debug_payload,
    }))
    .expect("HTTP output envelope must serialize");
    for forbidden in [
        "cleanup_token",
        "host-private",
        "fixture-lease",
        "egress-provider-secret",
    ] {
        assert!(
            !observable.contains(forbidden),
            "HTTP output leaked {forbidden}"
        );
    }
}
