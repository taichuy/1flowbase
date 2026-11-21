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
//! No SSH network execution, MCP/UI evidence or QA verdict is claimed by this asset.

#[path = "plugin_resource_archive_lifecycle/_tests/fixture.rs"]
mod fixture;
#[path = "plugin_resource_archive_lifecycle/_tests/inputs.rs"]
mod inputs;
#[path = "plugin_resource_archive_lifecycle/_tests/lifecycle.rs"]
mod lifecycle;
