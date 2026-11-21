//! Root #2896 R5 / QA1 U4: local-only archive lifecycle acceptance ASSET, not a QA verdict.
//! Reuses managed_snapshot_tests::RuntimeFixture (real RuntimeExtensionHost + production
//! bind_managed_runtime), ApiProviderRuntime and ManagedGenerationPublisher. No successful
//! runtime stub, direct lifecycle SQL, fabricated readiness or manual generation publication.
//!
//! Linux x86_64 GNU only. Inputs: same private variables as control-plane-postgres-tests/tests/
//! plugin_resource_archive_upgrade/_tests/README.md; additionally API_DATABASE_URL must be
//! explicit loopback PostgreSQL (the API helper does NOT read DATABASE_URL). TMPDIR must be
//! private and outside both checkouts. Root supplies fresh BUSINESS build receipts/packages.
//! Missing inputs FAIL when explicitly selected; the test is ignored by default.
//!
//! Root's shared-budget commands (implementation worker must not run Cargo):
//! cargo test --offline --manifest-path api/Cargo.toml -p api-server
//!   --test api_private_contracts --no-run
//! env -u PLUGIN_SHARED_RESOURCE_CONTRACTS_JSON cargo test --offline
//!   --manifest-path api/Cargo.toml -p api-server --test api_private_contracts --
//!   --ignored --exact _tests::extension_bus::plugin_resource_archive_lifecycle::lifecycle::
//!   provider_consumer_archives_real_api_host_lifecycle --test-threads=1
//! Join the last two selector lines with no whitespace. Require ONE executed test.
//!
//! Opt-in RuntimeFixture uses startup_assembly's actual ManagedPluginDataService and
//! ManagedPluginCredentialService, sharing the seeded store/master key. Authenticated root
//! and member hosts.list cross HTTP -> worker -> HostCall -> original taichuy/ssh rows;
//! restricted rows test positive/negative member filtering without mutating original ACLs.
//! R6 extends these SAME preserved rows/credentials to ephemeral pinned-key loopback SSH.
//! Authenticated Root HTTP snapshot and package MCP list/get/call must execute real worker
//! PluginCredential HostCalls and two SSH probes each; view-only member snapshot is denied
//! before/after Root cache population. No mock Host callbacks or fabricated metrics.
//! R7 exact target (same prefix as R6):
//! provider_consumer_archives_authenticated_aggregate_browser_lifecycle
//! Requires PLUGIN_RESOURCE_BROWSER_SCRIPT (Root-authored Node file, never optional in R7).
//! Optional PLUGIN_RESOURCE_BROWSER_OUTPUT_DIR must be an owner-private directory; otherwise
//! a retained private evidence directory is created under TMPDIR. Browser timeout: 180s;
//! overall R7 lifecycle timeout: 420s. R6 keeps its separate 180s target and assertions.
//! Real ticket/WS aggregate -> production shared broker -> same managed runtime/HostCalls
//! checks multiple hosts, cadence changes, selection, ACK, member open denial and cancellation
//! with SSH socket/probe quiescence BEFORE runtime shutdown. The mandatory subprocess receives
//! only {api_base_url,visible_host_id,hidden_host_id,expected_hostname,output_dir} on JSON stdin,
//! must emit R7_BROWSER_ACCEPTANCE and exit0, and owns the business UI/session assertions.
//! Root owns execution/QA. This asset is NOT a pass report. Positive nonroot open,
//! cross-workspace/scope forgery and post-revoke runtime HostCall negatives remain outside it.

#[path = "plugin_resource_archive_lifecycle/_tests/fixture.rs"]
mod fixture;
#[path = "plugin_resource_archive_lifecycle/_tests/inputs.rs"]
mod inputs;
#[path = "plugin_resource_archive_lifecycle/_tests/lifecycle.rs"]
mod lifecycle;

#[path = "plugin_resource_archive_lifecycle/_tests/loopback_ssh.rs"]
mod loopback_ssh;

#[path = "plugin_resource_archive_lifecycle/_tests/aggregate.rs"]
mod aggregate;
#[path = "plugin_resource_archive_lifecycle/_tests/browser.rs"]
mod browser;
