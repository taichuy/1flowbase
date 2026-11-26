//! Root #2896 U1 acceptance asset: real archives -> upload service -> explicit managed
//! activation preparation -> PostgreSQL schema/credential/ACL preservation.
//!
//! This is NOT HTTP, runtime activation, hot-generation or callback evidence (U4).
//! No fake Ready/Active state is written. See the subordinate README for prerequisites.

#![cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]

#[path = "plugin_resource_archive_upgrade/_tests/dependencies.rs"]
mod dependencies;
#[path = "plugin_resource_archive_upgrade/_tests/fixture.rs"]
mod fixture;
#[path = "plugin_resource_archive_upgrade/_tests/inputs.rs"]
mod inputs;
#[path = "plugin_resource_archive_upgrade/_tests/preservation.rs"]
mod preservation;
#[path = "plugin_resource_archive_upgrade/_tests/upgrade.rs"]
mod upgrade;

#[tokio::test]
#[ignore = "private old/new archives, fresh BUSINESS binary, private TMPDIR and local disposable PostgreSQL required; see _tests/README.md"]
async fn ssh_archives_explicit_preparation_preserves_full_original_state() -> anyhow::Result<()> {
    let inputs = inputs::Inputs::load()?;
    let fixture = fixture::Fixture::new().await?;
    let result = upgrade::run(&fixture, &inputs).await;
    // Close connections before the pool-owned PostgresTestSchema guard drops its database.
    // Preserve the assertion error; never let cleanup turn a failing test into success.
    fixture.pool.close().await;
    result
}

#[tokio::test]
#[ignore = "also requires freshly built real monitor archive/binary inputs; see _tests/README.md"]
async fn monitor_archive_enable_rejects_missing_and_disabled_provider_without_authority(
) -> anyhow::Result<()> {
    let inputs = inputs::Inputs::load()?;
    let consumer = inputs::ConsumerInputs::load()?;
    let fixture = fixture::Fixture::new().await?;
    let result = dependencies::run(&fixture, &inputs, &consumer).await;
    fixture.pool.close().await;
    result
}
