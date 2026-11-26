//! Actual enable commands stop at dependency preflight, before any runtime boundary.
use super::{fixture::Fixture, inputs::*};
use anyhow::{ensure, Context, Result};
use control_plane::{plugin_management::*, ports::*};
use control_plane_contracts::ControlPlaneContractError;
use std::{fs, path::Path};

fn dependency_denial(result: Result<domain::PluginTaskRecord>, expected: &str) -> Result<()> {
    let error = result
        .err()
        .context("consumer enable unexpectedly succeeded")?;
    ensure!(
        matches!(error.downcast_ref::<ControlPlaneContractError>(),
        Some(ControlPlaneContractError::PluginDependency {
            code, provider_publisher_namespace, provider_plugin_code, resource_code, dependents,
        }) if code == expected && provider_publisher_namespace == "taichuy"
            && provider_plugin_code == "ssh" && resource_code.is_none()
            && dependents.len() == 1 && dependents[0] == "iqlater/compute-monitor@0.2.7"),
        "enable did not return the expected structured provider dependency denial"
    );
    Ok(())
}

pub async fn run(f: &Fixture, inputs: &Inputs, consumer: &ConsumerInputs) -> Result<()> {
    let monitor = f
        .upload_named(&consumer.archive, "iqlater", "compute-monitor", "0.2.7")
        .await?;
    let id = monitor.result.installation.id;
    ensure!(
        monitor.runtime_sha256 == consumer.runtime_sha256,
        "monitor archive does not contain expected fresh binary"
    );
    let local = monitor
        .result
        .local_artifact
        .local_path
        .as_ref()
        .context("monitor artifact path missing")?;
    ensure!(
        fs::read(Path::new(local).join("manifest.yaml"))? == consumer.source_manifest,
        "monitor archive manifest differs from BUSINESS"
    );
    ensure!(
        monitor.manifest.data_models.is_empty(),
        "consumer must not own a duplicate provider schema"
    );
    ensure!(
        monitor.manifest.plugin_dependencies.len() == 1
            && monitor.manifest.resource_imports.len() == 1,
        "expected real consumer dependency/import declarations"
    );
    let original = f.authority().query(&f.root_actor, id).await?;
    dependency_denial(
        f.service
            .enable_plugin(EnablePluginCommand {
                actor_user_id: f.root_actor.user_id,
                installation_id: id,
            })
            .await,
        "plugin_dependency_missing",
    )?;
    ensure!(
        f.authority().query(&f.root_actor, id).await? == original,
        "missing-dependency enable minted authority"
    );
    f.assert_not_runtime_active(id).await?;
    ensure!(
        f.store.list_managed_schema_ownership().await?.is_empty(),
        "missing dependency created consumer schema"
    );

    let provider = f.upload(&inputs.new, NEW_VERSION).await?;
    ensure!(
        provider.runtime_sha256 == inputs.runtime_sha256,
        "provider archive does not contain expected fresh BUSINESS binary"
    );
    let local = provider
        .result
        .local_artifact
        .local_path
        .as_ref()
        .context("provider artifact path missing")?;
    ensure!(
        fs::read(Path::new(local).join("manifest.yaml"))? == inputs.source_manifest,
        "provider archive manifest differs from BUSINESS"
    );
    // Real archive readiness is present, but upload is not activation. Do not write
    // ActiveRequested/Active to bypass the unavailable real runtime composition.
    dependency_denial(
        f.service
            .enable_plugin(EnablePluginCommand {
                actor_user_id: f.root_actor.user_id,
                installation_id: id,
            })
            .await,
        "plugin_dependency_disabled",
    )?;
    ensure!(
        f.authority().query(&f.root_actor, id).await? == original,
        "disabled-dependency enable minted authority"
    );
    f.assert_not_runtime_active(id).await?;
    f.assert_not_runtime_active(provider.result.installation.id)
        .await?;
    Ok(())
}
