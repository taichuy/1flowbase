//! Root 2007 P09A AC-008/009 AUTH-05/09/10: real SDK workers and durable authority/switch owners.
use super::managed_activation_tests::{hook_input, hook_invocation, principal};
use super::managed_snapshot_tests::RuntimeFixture;
use crate::provider_runtime::ApiProviderRuntime;
use control_plane::{plugin_management::*, ports::AuthRepository};
use control_plane_contracts::ports::*;
use extension_contracts::*;
use runtime_core::runtime_backend::{CapabilityRuntimePort, RuntimeManagedHookRequest};
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

struct Fixture {
    state: Arc<crate::app_state::ApiState>,
    runtime: RuntimeFixture,
    management: Arc<crate::app_state::ApiPluginManagementService>,
    actor: domain::ActorContext,
}
impl Fixture {
    async fn new() -> Self {
        let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
        let actor_id: Uuid = sqlx::query_scalar("select id from users where account='root'")
            .fetch_one(state.store.pool())
            .await
            .unwrap();
        let actor = AuthRepository::load_actor_context_for_user(&state.store, actor_id)
            .await
            .unwrap();
        let runtime = RuntimeFixture::new(&state);
        let management = Arc::new(
            PluginManagementService::new(
                runtime.store.clone(),
                ApiProviderRuntime::new(runtime.services.clone()),
                state.official_plugin_source.clone(),
                &state.provider_install_root,
            )
            .with_node_id(&state.api_node_id),
        );
        Self {
            state,
            runtime,
            management,
            actor,
        }
    }
    fn authority(
        &self,
    ) -> PluginContributionAuthorityService<storage_durable_postgres::MainDurableStore> {
        PluginContributionAuthorityService::new(
            self.runtime.store.clone(),
            HostContributionGrantPolicy::root_composition(),
        )
        .with_node_id(&self.state.api_node_id)
    }
    async fn install(&self, name: &str, version: &str, handler: &str) -> (Uuid, PathBuf) {
        // Mutate the author document; PluginManifestV1 is a validated read model, not a serializer.
        let mut value: serde_json::Value = serde_yaml::from_str(include_str!(
            "../../../../../plugins/fixtures/acme.composition-a/manifest.yaml"
        ))
        .unwrap();
        let module = format!("acme.composition-{name}");
        value["plugin_id"] = module.clone().into();
        value["version"] = version.into();
        value["data_models"] = serde_json::json!([]);
        value["managed"]["module"]["module_id"] = module.clone().into();
        value["managed"]["module"]["module_version"] = version.into();
        let mut contribution = value["managed"]["module"]["contributions"][0].clone();
        contribution["contribution_id"] = format!("{module}.first").into();
        contribution["contributor_module_id"] = module.clone().into();
        value["managed"]["module"]["contributions"] = serde_json::json!([contribution]);
        let mut binding = value["managed"]["execution_bindings"][0].clone();
        binding["contribution_id"] = format!("{module}.first").into();
        binding["handler"] = handler.into();
        value["managed"]["execution_bindings"] = serde_json::json!([binding]);
        let bytes = serde_yaml::to_string(&value).unwrap().into_bytes();
        plugin_framework::parse_plugin_manifest(std::str::from_utf8(&bytes).unwrap()).unwrap();
        let mut archive = tar::Builder::new(flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::default(),
        ));
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        archive
            .append_data(&mut header, "manifest.yaml", bytes.as_slice())
            .unwrap();
        let source = PathBuf::from(
            std::env::var_os("MANAGED_HOOK_WORKER_FIXTURE")
                .expect("CI must prebuild the actual managed_hook_worker SDK example"),
        );
        assert!(source.is_absolute() && source.is_file());
        archive
            .append_path_with_name(source, "bin/worker.py")
            .unwrap();
        let installed = self
            .management
            .install_uploaded_plugin(InstallUploadedPluginCommand {
                actor_user_id: self.actor.user_id,
                file_name: format!("{module}.1flowbasepkg"),
                package_bytes: archive.into_inner().unwrap().finish().unwrap(),
            })
            .await
            .unwrap();
        (
            installed.installation.id,
            PathBuf::from(installed.local_artifact.local_path.unwrap()).join("bin/worker.py"),
        )
    }
    async fn source(&self) -> (Uuid, PathBuf) {
        let source = self.install("a", "1.0.0", "first").await;
        self.management
            .assign_plugin(AssignPluginCommand {
                actor_user_id: self.actor.user_id,
                installation_id: source.0,
            })
            .await
            .unwrap();
        self.authority()
            .grant(&self.actor, source.0, grant("a"))
            .await
            .unwrap();
        self.management
            .enable_plugin(EnablePluginCommand {
                actor_user_id: self.actor.user_id,
                installation_id: source.0,
            })
            .await
            .unwrap();
        source
    }
    fn switch(
        &self,
        target: Uuid,
    ) -> tokio::task::JoinHandle<anyhow::Result<domain::PluginTaskRecord>> {
        let management = self.management.clone();
        let actor = self.actor.user_id;
        tokio::spawn(async move {
            management
                .switch_version(SwitchPluginVersionCommand {
                    actor_user_id: actor,
                    provider_code: "acme.composition-a".into(),
                    target_installation_id: target,
                })
                .await
        })
    }
}
fn grant(name: &str) -> GrantContributionPermission {
    GrantContributionPermission {
        contribution_id: format!("acme.composition-{name}.first"),
        permission: "hook.model_definitions.create.before".into(),
        resource_scope: domain::ContributionResourceScope::Workspace,
        permission_contract_id: "managed-hook".into(),
        permission_contract_version: "1".into(),
    }
}
fn subject(installation: Uuid, workspace: Uuid) -> ManagedContributionSubject {
    ManagedContributionSubject::new(
        ManagedInstallationId::new(installation.to_string()).unwrap(),
        ManagedWorkspaceId::new(workspace.to_string()).unwrap(),
        ContributionId::new("acme.composition-a.first").unwrap(),
    )
}
async fn started(path: &std::path::Path) {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while !path.with_extension("started").is_file() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn root_2007_ac_009_pause_revoke_retire_candidate_switch() {
    let fixture = Fixture::new().await;
    let (source, worker) = fixture.source().await;
    let (target, target_worker) = fixture.install("a", "1.1.0", "second").await;
    let workspace = fixture.actor.current_workspace_id;
    let g1 = fixture
        .runtime
        .composition
        .snapshot(workspace)
        .await
        .unwrap();
    // No implied grant inheritance and no assignment mutation on a rejected candidate.
    assert!(fixture.switch(target).await.unwrap().is_err());
    let assignments = fixture
        .runtime
        .store
        .list_assignments(workspace)
        .await
        .unwrap();
    assert_eq!(
        assignments
            .iter()
            .find(|assignment| assignment.provider_code == "acme.composition-a")
            .unwrap()
            .installation_id,
        source
    );
    let wrong_node = PluginContributionAuthorityService::new(
        fixture.runtime.store.clone(),
        HostContributionGrantPolicy::root_composition(),
    )
    .with_node_id("foreign-node");
    assert!(wrong_node
        .grant(&fixture.actor, target, grant("a"))
        .await
        .is_err());
    let mut foreign_actor = fixture.actor.clone();
    foreign_actor.current_workspace_id = Uuid::now_v7();
    assert!(fixture
        .authority()
        .grant(&foreign_actor, target, grant("a"))
        .await
        .is_err());
    let (other_family, _) = fixture.install("b", "1.0.0", "first").await;
    assert!(fixture
        .authority()
        .grant(&fixture.actor, other_family, grant("b"))
        .await
        .is_err());
    fixture
        .authority()
        .grant(&fixture.actor, target, grant("a"))
        .await
        .unwrap();
    assert!(
        fixture
            .runtime
            .store
            .lock_contribution_authority(&subject(target, workspace))
            .await
            .is_err(),
        "candidate grants cannot admit execution"
    );
    let saved = target_worker.with_extension("saved");
    std::fs::rename(&target_worker, &saved).unwrap();
    let rejected = fixture.switch(target).await.unwrap();
    std::fs::rename(&saved, &target_worker).unwrap();
    assert!(rejected.is_err());
    assert!(Arc::ptr_eq(
        &g1,
        &fixture
            .runtime
            .composition
            .snapshot(workspace)
            .await
            .unwrap()
    ));
    let contribution = ContributionId::new("acme.composition-a.first").unwrap();
    // Dropping a cancelled drain owner restores admission without retiring G1.
    let drain = fixture
        .runtime
        .host
        .drain_managed_contributions(&[g1.bindings[&contribution].handle.clone()])
        .await
        .unwrap();
    let request = || RuntimeManagedHookRequest {
        handle: g1.bindings[&contribution].handle.clone(),
        principal: principal(workspace, fixture.actor.user_id),
        invocation: hook_invocation(&g1),
        input: hook_input("identify"),
    };
    assert!(fixture
        .runtime
        .host
        .admit_managed_hook(request())
        .await
        .is_err());
    drop(drain);
    drop(
        fixture
            .runtime
            .host
            .admit_managed_hook(request())
            .await
            .unwrap(),
    );
    // A finite timeout/revoke/commit matrix holds the real worker at the same barrier.
    // Both failed drain and failed final authority check must reopen G1 admission.
    for outcome in ["timeout", "revoke", "commit"] {
        let owner = fixture.runtime.composition.clone();
        let frozen = g1.clone();
        let actor = fixture.actor.user_id;
        let running = tokio::spawn(async move {
            owner
                .execute_hook(
                    &frozen,
                    &ContributionId::new("acme.composition-a.first").unwrap(),
                    principal(workspace, actor),
                    hook_invocation(&frozen),
                    hook_input("fixture_barrier"),
                )
                .await
        });
        started(&worker).await;
        let switching = fixture.switch(target);
        // Wait for the real runtime gate, dropping admitted futures without executing extra work.
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                let request = RuntimeManagedHookRequest {
                    handle: g1.bindings[&contribution].handle.clone(),
                    principal: principal(workspace, fixture.actor.user_id),
                    invocation: hook_invocation(&g1),
                    input: hook_input("identify"),
                };
                match fixture.runtime.host.admit_managed_hook(request).await {
                    Ok(operation) => drop(operation),
                    Err(_) => break,
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(!switching.is_finished());
        assert!(Arc::ptr_eq(
            &g1,
            &fixture
                .runtime
                .composition
                .snapshot(workspace)
                .await
                .unwrap()
        ));
        assert_eq!(
            fixture
                .runtime
                .store
                .list_assignments(workspace)
                .await
                .unwrap()
                .into_iter()
                .find(|assignment| assignment.provider_code == "acme.composition-a")
                .unwrap()
                .installation_id,
            source
        );
        if outcome == "timeout" {
            let rejected = tokio::time::timeout(std::time::Duration::from_secs(8), switching)
                .await
                .unwrap()
                .unwrap();
            assert!(rejected
                .unwrap_err()
                .to_string()
                .contains("drain timed out"));
            assert!(Arc::ptr_eq(
                &g1,
                &fixture
                    .runtime
                    .composition
                    .snapshot(workspace)
                    .await
                    .unwrap()
            ));
            assert_eq!(
                fixture
                    .runtime
                    .composition
                    .execute_hook(
                        &g1,
                        &contribution,
                        principal(workspace, fixture.actor.user_id),
                        hook_invocation(&g1),
                        hook_input("identify")
                    )
                    .await
                    .unwrap(),
                ManagedHookOutcome::Deny {
                    classification: "fixture.first".into()
                }
            );
            std::fs::write(worker.with_extension("release"), "release").unwrap();
            assert_eq!(
                running.await.unwrap().unwrap(),
                ManagedHookOutcome::Continue
            );
            std::fs::remove_file(worker.with_extension("release")).unwrap();
            std::fs::remove_file(worker.with_extension("started")).unwrap();
            continue;
        }
        if outcome == "revoke" {
            let grants = fixture
                .authority()
                .query(&fixture.actor, target)
                .await
                .unwrap();
            fixture
                .authority()
                .revoke(
                    &fixture.actor,
                    target,
                    RevokeContributionPermission {
                        authorization_id: grants.authorizations[0].id,
                        expected_revision: grants.revision,
                    },
                )
                .await
                .unwrap();
        }
        if outcome == "commit" {
            // Cancelling the protocol waiter does not cancel the owned business switch.
            switching.abort();
        }
        std::fs::write(worker.with_extension("release"), "release").unwrap();
        assert_eq!(
            running.await.unwrap().unwrap(),
            ManagedHookOutcome::Continue
        );
        let result = if outcome == "commit" {
            assert!(switching.await.unwrap_err().is_cancelled());
            Ok(
                tokio::time::timeout(std::time::Duration::from_secs(5), async {
                    loop {
                        if let Some(task) = fixture
                            .runtime
                            .store
                            .list_tasks()
                            .await
                            .unwrap()
                            .into_iter()
                            .find(|task| {
                                task.installation_id == Some(target)
                                    && task.status == domain::PluginTaskStatus::Succeeded
                                    && task.task_kind == domain::PluginTaskKind::SwitchVersion
                            })
                        {
                            break task;
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap(),
            )
        } else {
            switching.await.unwrap()
        };
        std::fs::remove_file(worker.with_extension("release")).unwrap();
        std::fs::remove_file(worker.with_extension("started")).unwrap();
        if outcome == "revoke" {
            assert!(result.is_err());
            assert!(Arc::ptr_eq(
                &g1,
                &fixture
                    .runtime
                    .composition
                    .snapshot(workspace)
                    .await
                    .unwrap()
            ));
            assert_eq!(
                fixture
                    .runtime
                    .composition
                    .execute_hook(
                        &g1,
                        &contribution,
                        principal(workspace, fixture.actor.user_id),
                        hook_invocation(&g1),
                        hook_input("identify")
                    )
                    .await
                    .unwrap(),
                ManagedHookOutcome::Deny {
                    classification: "fixture.first".into()
                }
            );
            fixture
                .authority()
                .grant(&fixture.actor, target, grant("a"))
                .await
                .unwrap();
        } else {
            assert_eq!(result.unwrap().status, domain::PluginTaskStatus::Succeeded);
        }
    }
    let g2 = fixture
        .runtime
        .composition
        .snapshot(workspace)
        .await
        .unwrap();
    assert_ne!(g1.graph.fingerprint(), g2.graph.fingerprint());
    assert_eq!(
        fixture
            .runtime
            .store
            .list_assignments(workspace)
            .await
            .unwrap()
            .into_iter()
            .find(|assignment| assignment.provider_code == "acme.composition-a")
            .unwrap()
            .installation_id,
        target
    );
    assert_eq!(
        fixture
            .runtime
            .composition
            .execute_hook(
                &g2,
                &contribution,
                principal(workspace, fixture.actor.user_id),
                hook_invocation(&g2),
                hook_input("identify")
            )
            .await
            .unwrap(),
        ManagedHookOutcome::Deny {
            classification: "fixture.second".into()
        }
    );
    assert!(fixture
        .runtime
        .composition
        .execute_hook(
            &g1,
            &contribution,
            principal(workspace, fixture.actor.user_id),
            hook_invocation(&g1),
            hook_input("identify")
        )
        .await
        .is_err());
    let failed: i64 = sqlx::query_scalar(
        "select count(*) from plugin_tasks where installation_id=$1 and status='failed'",
    )
    .bind(target)
    .fetch_one(fixture.runtime.store.pool())
    .await
    .unwrap();
    assert!(
        failed >= 4,
        "each rejected switch has a durable failed task"
    );
    fixture.runtime.host.stop().await.unwrap();
}

#[tokio::test]
async fn root_2007_ac_009_pause_revoke_retire_historical_authority() {
    let fixture = Fixture::new().await;
    let (source, _) = fixture.source().await;
    let (target, _) = fixture.install("a", "1.1.0", "second").await;
    let workspace = fixture.actor.current_workspace_id;
    let before = fixture
        .authority()
        .query(&fixture.actor, source)
        .await
        .unwrap();
    // Assignment owner changes the durable pointer; the old revision must remain manageable.
    fixture
        .management
        .assign_plugin(AssignPluginCommand {
            actor_user_id: fixture.actor.user_id,
            installation_id: target,
        })
        .await
        .unwrap();
    assert!(fixture
        .runtime
        .store
        .lock_contribution_authority(&subject(source, workspace))
        .await
        .is_err());
    let history = fixture
        .authority()
        .query(&fixture.actor, source)
        .await
        .unwrap();
    assert_eq!(history, before);
    let revoked = fixture
        .authority()
        .revoke(
            &fixture.actor,
            source,
            RevokeContributionPermission {
                authorization_id: history.authorizations[0].id,
                expected_revision: history.revision,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        revoked.authorizations[0].status,
        domain::ContributionAuthorizationStatus::Revoked
    );
    let restarted = PluginContributionAuthorityService::new(
        fixture.runtime.store.clone(),
        HostContributionGrantPolicy::root_composition(),
    )
    .with_node_id(&fixture.state.api_node_id);
    assert_eq!(
        restarted.query(&fixture.actor, source).await.unwrap(),
        revoked
    );
    let mut foreign_actor = fixture.actor.clone();
    foreign_actor.current_workspace_id = Uuid::now_v7();
    assert!(restarted.query(&foreign_actor, source).await.is_err());
    let histories: i64 = sqlx::query_scalar("select count(*) from plugin_contribution_authorization_revisions where installation_id=$1 and workspace_id=$2").bind(source).bind(foreign_actor.current_workspace_id).fetch_one(fixture.runtime.store.pool()).await.unwrap();
    assert_eq!(
        histories, 0,
        "a rejected read cannot create authority history"
    );
    // Re-enable and old graph retention cannot recreate a revoked permission.
    fixture
        .management
        .assign_plugin(AssignPluginCommand {
            actor_user_id: fixture.actor.user_id,
            installation_id: source,
        })
        .await
        .unwrap();
    assert!(fixture
        .management
        .enable_plugin(EnablePluginCommand {
            actor_user_id: fixture.actor.user_id,
            installation_id: source
        })
        .await
        .is_err());
    assert_eq!(
        restarted.query(&fixture.actor, source).await.unwrap(),
        revoked
    );
    fixture.runtime.host.stop().await.unwrap();
}
