use super::{
    fixture::{Fixture, Preserved},
    inputs::Inputs,
    loopback_ssh::LoopbackSsh,
};
use anyhow::{Context, Result, ensure};
use axum::http::StatusCode;
use control_plane::{plugin_management::*, ports::*};
use control_plane_contracts::ControlPlaneContractError;
use domain::*;
use plugin_framework::extension_bus::ContributionId;

fn dependency<T>(result: Result<T>, expected: &str, resource: Option<&str>) -> Result<()> {
    let error = result
        .err()
        .context("dependency-protected operation unexpectedly succeeded")?;
    ensure!(
        matches!(error.downcast_ref::<ControlPlaneContractError>(), Some(ControlPlaneContractError::PluginDependency {
        code, provider_publisher_namespace, provider_plugin_code, resource_code, dependents,
    }) if code == expected && provider_publisher_namespace == "taichuy" && provider_plugin_code == "ssh"
        && resource_code.as_deref() == resource && dependents.len() == 1 && dependents[0] == "iqlater/compute-monitor@0.2.7"),
        "wrong structured dependency identity/code"
    );
    Ok(())
}
fn succeeded(task: PluginTaskRecord) -> Result<()> {
    ensure!(
        task.status == PluginTaskStatus::Succeeded,
        "real lifecycle task did not succeed"
    );
    Ok(())
}
async fn run(
    f: &Fixture,
    inputs: &Inputs,
    ssh: &LoopbackSsh,
    browser: Option<&super::browser::BrowserConfig>,
) -> Result<()> {
    let boot = f.generation()?;
    let (monitor, manifest, _) = f.upload(&inputs.monitor).await?;
    let monitor_id = monitor.installation.id;
    ensure!(
        manifest.data_models.is_empty()
            && manifest.plugin_dependencies.len() == 1
            && manifest.resource_imports.len() == 1,
        "consumer duplicates provider schema or omits import/dependency"
    );
    let unapproved = f.authority().query(&f.actor, monitor_id).await?;
    ensure!(
        unapproved.authorizations.is_empty() && f.generation()? == boot,
        "consumer upload conferred authority or changed serving generation"
    );
    let enable_consumer = || {
        f.management.enable_plugin(EnablePluginCommand {
            actor_user_id: f.actor.user_id,
            installation_id: monitor_id,
        })
    };
    dependency(enable_consumer().await, "plugin_dependency_missing", None)?;
    let (old, old_manifest, old_binary) = f.upload(&inputs.old).await?;
    let old_id = old.installation.id;
    dependency(enable_consumer().await, "plugin_dependency_disabled", None)?;
    ensure!(
        f.generation()? == boot,
        "failed prerequisite changed serving generation"
    );
    succeeded(
        f.management
            .enable_plugin(EnablePluginCommand {
                actor_user_id: f.actor.user_id,
                installation_id: old_id,
            })
            .await
            .context("genuine archived SSH 0.1.21 real API-host enable failed")?,
    )?;
    f.active(old_id).await?;
    ensure!(
        f.generation()? != boot,
        "old enable did not publish real host generation"
    );
    let preserved = Preserved::seed(f, old_id, &old_manifest, ssh, browser.is_none()).await?;
    let old_generation = f.generation()?;
    dependency(
        enable_consumer().await,
        "plugin_resource_export_missing",
        Some("ssh-hosts-v1"),
    )?;
    f.inactive(monitor_id).await?;
    let (new, manifest, new_binary) = f.upload(&inputs.new).await?;
    let new_id = new.installation.id;
    ensure!(
        new_id != old_id && new_binary != old_binary,
        "fresh upgrade must retain old installation and replace executable"
    );
    ensure!(
        manifest.plugin_dependencies.is_empty()
            && manifest
                .resource_exports
                .iter()
                .any(|e| e.resource_code == "ssh-hosts-v1"),
        "SSH must independently export owned resources"
    );
    ensure!(
        f.generation()? == old_generation,
        "archive upload changed serving generation"
    );
    succeeded(
        f.management
            .switch_version(SwitchPluginVersionCommand {
                actor_user_id: f.actor.user_id,
                provider_code: "ssh".into(),
                target_installation_id: new_id,
            })
            .await
            .context("real SSH 0.1.21 -> 0.1.25 family switch failed")?,
    )?;
    f.active(new_id).await?;
    // Retained snapshots may still own old runtime handles; assert selection, not fake quiescence.
    let retired = f
        .runtime
        .store
        .get_installation(old_id)
        .await?
        .context("historical installation lost")?;
    ensure!(
        retired.desired_state == PluginDesiredState::Disabled,
        "old family version still selected"
    );
    ensure!(
        f.generation()? != old_generation,
        "version switch did not hot-publish generation"
    );
    preserved.check(f, new_id).await?;

    let provider_generation = f.generation()?;
    let activation = ManagedPluginActivationService::new(
        f.runtime.store.clone(),
        f.state.api_node_id.clone(),
        f.state.provider_install_root.clone().into(),
    );
    let denial = activation
        .prepare(&f.member, monitor_id)
        .await
        .err()
        .context("nonroot self-approved consumer")?;
    ensure!(
        matches!(
            denial.downcast_ref::<ControlPlaneContractError>(),
            Some(ControlPlaneContractError::PermissionDenied(
                "managed_contribution_authorization_required"
            ))
        ),
        "wrong nonroot authority denial"
    );
    ensure!(
        f.authority().query(&f.actor, monitor_id).await? == unapproved,
        "nonroot attempt minted grants"
    );
    let ownership = f.runtime.store.list_managed_schema_ownership().await?;
    succeeded(
        enable_consumer()
            .await
            .context("real monitor 0.2.7 explicit Root enable failed")?,
    )?;
    f.active(monitor_id).await?;
    f.active(new_id).await?;
    let active_generation = f.generation()?;
    ensure!(
        active_generation != provider_generation,
        "consumer enable did not publish host generation"
    );
    let approved = f.authority().query(&f.actor, monitor_id).await?;
    ensure!(
        !approved.authorizations.is_empty(),
        "explicit Root enable did not approve declared grants"
    );
    activation.prepare(&f.actor, monitor_id).await?;
    activation.prepare(&f.member, monitor_id).await?;
    ensure!(
        f.authority().query(&f.actor, monitor_id).await? == approved,
        "existing approvals are not idempotent/reusable"
    );
    ensure!(
        f.runtime.store.list_managed_schema_ownership().await? == ownership,
        "consumer enable duplicated owned schema"
    );
    let snapshot = f
        .runtime
        .composition
        .snapshot(SYSTEM_SCOPE_ID)
        .await
        .context("real managed workspace snapshot missing")?;
    ensure!(
        snapshot
            .bindings
            .contains_key(&ContributionId::new("compute-monitor.hosts.list")?)
            && snapshot
                .bindings
                .contains_key(&ContributionId::new("ssh.hosts.list")?),
        "actual provider/consumer bindings not jointly published"
    );
    let current = f
        .publisher
        .current_state()
        .context("published state missing")?;
    for id in [new_id, monitor_id] {
        ensure!(
            current
                .console_surface_registry
                .managed_services()
                .iter()
                .any(|r| r.installation_id == id),
            "real host registry lacks active installation"
        );
    }
    ensure!(
        !current
            .console_surface_registry
            .managed_services()
            .iter()
            .any(|r| r.installation_id == old_id),
        "old version still registered after switch"
    );
    // Real retained HTTP ingress -> actual worker -> managed HostCall -> provider storage.
    let (status, _) = f
        .get("/api/console/managed-services/compute-monitor/hosts", false)
        .await?;
    ensure!(
        status == StatusCode::UNAUTHORIZED,
        "registered consumer route must require authentication"
    );
    let (status, _) = f
        .get(
            "/api/console/docs/operations/compute-monitor.hosts.list/openapi.json",
            true,
        )
        .await?;
    ensure!(
        status == StatusCode::OK,
        "real registered consumer API document unavailable"
    );
    preserved.check_hosts_list(f, ssh).await?;
    preserved.check_monitor_snapshot(f, ssh).await?;
    if let Some(browser) = browser {
        super::aggregate::accept(f, &preserved, ssh, browser).await?;
    }
    ensure!(
        f.generation()? == active_generation,
        "readonly HTTP consumer changed serving generation"
    );
    preserved.check(f, new_id).await?;

    dependency(
        f.management
            .disable_plugin(DisablePluginCommand {
                actor_user_id: f.actor.user_id,
                installation_id: new_id,
            })
            .await,
        "plugin_dependency_in_use",
        None,
    )?;
    dependency(
        f.management
            .delete_family(DeletePluginFamilyCommand {
                actor_user_id: f.actor.user_id,
                provider_code: "ssh".into(),
            })
            .await,
        "plugin_dependency_in_use",
        None,
    )?;
    // The genuine old archive has no compatible export: never mutate candidate metadata.
    ensure!(
        old_manifest.resource_exports.is_empty(),
        "archived rollback target unexpectedly exports new resource contract"
    );
    dependency(
        f.management
            .switch_version(SwitchPluginVersionCommand {
                actor_user_id: f.actor.user_id,
                provider_code: "ssh".into(),
                target_installation_id: old_id,
            })
            .await,
        "plugin_dependency_in_use",
        None,
    )?;
    ensure!(
        f.generation()? == active_generation,
        "rejected provider operation changed serving generation"
    );
    f.active(new_id).await?;
    f.active(monitor_id).await?;
    preserved.check(f, new_id).await?;
    // Revocation cannot be revived by a nonroot preparation/enable path.
    let grant = approved
        .authorizations
        .iter()
        .find(|g| g.status == ContributionAuthorizationStatus::Active)
        .context("active grant missing")?;
    let revoked = f
        .authority()
        .revoke(
            &f.actor,
            monitor_id,
            RevokeContributionPermission {
                authorization_id: grant.id,
                expected_revision: approved.revision,
            },
        )
        .await
        .context("revoke live consumer authority")?;
    let denial = activation
        .prepare(&f.member, monitor_id)
        .await
        .err()
        .context("nonroot revived revoked authority")?;
    ensure!(
        matches!(
            denial.downcast_ref::<ControlPlaneContractError>(),
            Some(ControlPlaneContractError::PermissionDenied(
                "managed_contribution_authorization_required"
            ))
        ),
        "wrong revoked authority denial"
    );
    ensure!(
        f.authority().query(&f.actor, monitor_id).await? == revoked,
        "nonroot revived revoked grant"
    );
    succeeded(
        f.management
            .disable_plugin(DisablePluginCommand {
                actor_user_id: f.actor.user_id,
                installation_id: monitor_id,
            })
            .await
            .context("disable consumer after explicit authority revocation")?,
    )?;
    f.inactive(monitor_id).await?;
    succeeded(
        f.management
            .disable_plugin(DisablePluginCommand {
                actor_user_id: f.actor.user_id,
                installation_id: new_id,
            })
            .await
            .context("disable provider after consumer is inactive")?,
    )?;
    f.inactive(new_id).await?;
    ensure!(
        f.generation()? != active_generation,
        "real disable did not publish generation"
    );
    preserved.check(f, new_id).await?;
    Ok(())
}
#[tokio::test]
#[ignore = "local-only: requires private genuine archives, fresh BUSINESS builds and explicit loopback test DB"]
async fn provider_consumer_archives_real_api_host_lifecycle() -> Result<()> {
    acceptance(None).await
}

#[tokio::test]
#[ignore = "R7 local-only: requires genuine archives, loopback DB/SSH and mandatory Root browser script"]
async fn provider_consumer_archives_authenticated_aggregate_browser_lifecycle() -> Result<()> {
    // Validate before creating runtimes: R7 can never silently fall back to R6.
    let browser = super::browser::BrowserConfig::load()?;
    acceptance(Some(&browser)).await
}

async fn acceptance(browser: Option<&super::browser::BrowserConfig>) -> Result<()> {
    let inputs = Inputs::load()?; // Missing private input is a failure, never an early-success skip.
    let mut ssh = LoopbackSsh::start().await?;
    let f = Fixture::new().await?;
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(if browser.is_some() { 420 } else { 180 }),
        run(&f, &inputs, &ssh, browser),
    )
    .await;
    // Real host shutdown owns all subprocesses, including after a lifecycle assertion error.
    let stopped = f.runtime.host.stop().await;
    let ssh_cleanup = ssh.stop();
    let cleanup = std::fs::remove_dir_all(&f.state.provider_install_root);
    result.context("bounded archive lifecycle acceptance timed out")??;
    stopped.context("real runtime host shutdown failed")?;
    ssh_cleanup.context("owned loopback SSH cleanup failed")?;
    cleanup.context("private installed artifact cleanup failed")?;
    Ok(())
}
