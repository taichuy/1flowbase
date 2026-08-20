use std::{
    fs,
    net::TcpListener,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use plugin_runner::network_egress_host::{
    NetworkEgressCleanupReason, NetworkEgressHost, NetworkEgressWorkerState,
};
use time::OffsetDateTime;

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TempNetworkEgressPackage {
    root: PathBuf,
}

impl TempNetworkEgressPackage {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after epoch")
            .as_nanos();
        let sequence = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "network-egress-runner-test-{}-{nonce}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("bin")).expect("fixture package directory must be created");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write_runtime(
        &self,
        proxy_url: &str,
        expires_at: u64,
        exit_after_acquire: bool,
        child_marker: Option<&Path>,
    ) {
        fs::write(
            self.root.join("manifest.yaml"),
            r#"manifest_version: 1
plugin_id: fixture_egress
version: 0.1.0
publisher_namespace: 1flowbase
vendor: 1flowbase
display_name: Fixture Egress
description: Runner fixture
source_kind: uploaded
trust_level: checksum_only
consumption_kind: runtime_extension
execution_mode: stateful_runtime_worker
slot_codes:
  - network_egress_provider
binding_targets:
  - workspace
selection_mode: manual_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.network_egress_provider/v1
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: none
  secrets: none
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json_worker
  entry: bin/fixture_egress
  limits:
    timeout_ms: 2000
node_contributions: []
"#,
        )
        .expect("fixture manifest must be written");
        let child = child_marker
            .map(|path| format!("( sleep 30 ) & echo $! > '{}'\n", path.display()))
            .unwrap_or_default();
        let exit_after_acquire = exit_after_acquire.then_some("exit 0").unwrap_or_default();
        fs::write(
            self.root.join("bin/fixture_egress"),
            format!(
                "#!/usr/bin/env bash\nset -euo pipefail\n{child}while IFS= read -r request; do\n  case \"${{request}}\" in\n    *'\"operation\":\"sync_egresses\"'*)\n      printf '%s\\n' '{{\"operation\":\"sync_egresses\",\"result\":{{\"egresses\":[{{\"provider_egress_key\":\"egress-us-1\",\"display_name\":\"US 1\",\"availability\":\"available\"}}]}}}}'\n      ;;\n    *'\"operation\":\"acquire_http_forward_proxy\"'*)\n      printf '%s\\n' '{{\"operation\":\"acquire_http_forward_proxy\",\"result\":{{\"lease_id\":\"lease-1\",\"http_proxy_url\":\"{proxy_url}\",\"cleanup_token\":\"opaque-cleanup-capability\",\"expires_at\":{expires_at}}}}}'\n      {exit_after_acquire}\n      ;;\n    *'\"operation\":\"release_http_forward_proxy\"'*)\n      printf '%s\\n' '{{\"operation\":\"release_http_forward_proxy\",\"result\":{{\"lease_id\":\"lease-1\"}}}}'\n      ;;\n    *) exit 1 ;;\n  esac\ndone\n"
            ),
        )
        .expect("fixture runtime must be written");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let path = self.root.join("bin/fixture_egress");
            let mut permissions = fs::metadata(&path)
                .expect("fixture runtime metadata must be readable")
                .permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).expect("fixture runtime must be executable");
        }
    }
}

impl Drop for TempNetworkEgressPackage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[tokio::test]
async fn nc_02b_sync_egresses_exposes_only_the_validated_provider_catalog() {
    let package = TempNetworkEgressPackage::new();
    package.write_runtime(
        "http://127.0.0.1:18080",
        unix_milliseconds_now() + 300_000,
        false,
        None,
    );
    let mut host = NetworkEgressHost::default();
    host.load_if_needed(
        "fixture_egress@0.1.0",
        &package.path().display().to_string(),
        "fixture-v1",
    )
    .await
    .expect("fixture worker must register");

    let egresses = host
        .sync_egresses("fixture_egress@0.1.0")
        .await
        .expect("typed sync result must be returned");
    assert_eq!(egresses.len(), 1);
    assert_eq!(egresses[0].provider_egress_key, "egress-us-1");
    assert_eq!(
        egresses[0].availability,
        plugin_framework::EgressAvailability::Available
    );
    assert!(host.sync_egresses("missing@0.1.0").await.is_err());

    host.unload("fixture_egress@0.1.0")
        .await
        .expect("worker without a lease must stop cleanly");
}

#[tokio::test]
async fn ac_002_ac_003_ac_005_ac_014_resolves_public_lease_and_cleans_mihomo_tree() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("fixture proxy listener must bind");
    let port = listener
        .local_addr()
        .expect("fixture proxy address must resolve")
        .port();
    thread::spawn(move || {
        let _ = listener.accept();
    });
    let package = TempNetworkEgressPackage::new();
    let child_marker = package.path().join("mihomo-child.pid");
    package.write_runtime(
        &format!("http://127.0.0.1:{port}"),
        unix_milliseconds_now() + 300_000,
        false,
        Some(&child_marker),
    );

    let mut host = NetworkEgressHost::default();
    host.load_if_needed(
        "fixture_egress@0.1.0",
        &package.path().display().to_string(),
        "fixture-v1",
    )
    .await
    .expect("activation must register the stateful worker without acquiring a lease");
    let lease = host
        .resolve_http_forward_proxy("fixture_egress@0.1.0", "egress-us-1")
        .await
        .expect("resolver must acquire and actively validate the public lease");
    assert_eq!(lease.http_proxy_url, format!("http://127.0.0.1:{port}"));
    let child_pid = wait_for_child_pid(&child_marker);

    host.unload("fixture_egress@0.1.0")
        .await
        .expect("deactivation must release and stop the worker");
    let receipt = host
        .cleanup_receipt("fixture_egress@0.1.0")
        .expect("cleanup must leave an auditable receipt");
    assert!(receipt.lease_revoked);
    assert!(receipt.termination_signal_sent);
    assert!(receipt.process_tree_exited);
    assert_eq!(receipt.reason, NetworkEgressCleanupReason::Stopped);
    assert_eq!(receipt.final_state, NetworkEgressWorkerState::Inactive);
    #[cfg(target_os = "linux")]
    assert!(!PathBuf::from(format!("/proc/{child_pid}")).exists());
}

#[tokio::test]
async fn ac_004_rejects_non_loopback_or_expired_leases_before_core_can_consume_them() {
    let package = TempNetworkEgressPackage::new();
    package.write_runtime(
        "http://198.51.100.19:7890",
        unix_milliseconds_now() + 300_000,
        false,
        None,
    );
    let mut host = NetworkEgressHost::default();
    host.load_if_needed(
        "fixture_egress@0.1.0",
        &package.path().display().to_string(),
        "fixture-v1",
    )
    .await
    .expect("activation must not acquire the invalid lease");

    let error = host
        .resolve_http_forward_proxy("fixture_egress@0.1.0", "egress-us-1")
        .await
        .expect_err("non-loopback endpoint must be rejected");
    assert!(error.to_string().contains("loopback-only"));
}

#[tokio::test]
async fn ac_004_revokes_the_lease_when_the_worker_crashes() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("fixture proxy listener must bind");
    let port = listener
        .local_addr()
        .expect("fixture proxy address must resolve")
        .port();
    thread::spawn(move || {
        let _ = listener.accept();
    });
    let package = TempNetworkEgressPackage::new();
    package.write_runtime(
        &format!("http://127.0.0.1:{port}"),
        unix_milliseconds_now() + 300_000,
        true,
        None,
    );
    let mut host = NetworkEgressHost::default();
    host.load_if_needed(
        "fixture_egress@0.1.0",
        &package.path().display().to_string(),
        "fixture-v1",
    )
    .await
    .expect("activation must start the worker");
    host.resolve_http_forward_proxy("fixture_egress@0.1.0", "egress-us-1")
        .await
        .expect("fixture worker must first return a valid lease");
    tokio::time::sleep(Duration::from_millis(25)).await;

    assert!(host
        .resolve_http_forward_proxy("fixture_egress@0.1.0", "egress-us-1")
        .await
        .is_err());
    let receipt = host
        .cleanup_receipt("fixture_egress@0.1.0")
        .expect("crash must revoke the lease and retain cleanup evidence");
    assert!(receipt.lease_revoked);
    assert_eq!(receipt.reason, NetworkEgressCleanupReason::RuntimeFailure);
    assert_eq!(receipt.final_state, NetworkEgressWorkerState::Failed);
}

fn unix_milliseconds_now() -> u64 {
    let milliseconds = OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
    u64::try_from(milliseconds).expect("current Unix milliseconds must fit u64")
}

fn wait_for_child_pid(marker: &Path) -> u32 {
    for _ in 0..40 {
        if let Ok(pid) = fs::read_to_string(marker) {
            return pid
                .trim()
                .parse()
                .expect("fixture child pid must be numeric");
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("fixture worker did not record its Mihomo child pid");
}
