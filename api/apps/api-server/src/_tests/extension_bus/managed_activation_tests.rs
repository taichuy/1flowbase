//! Root 2007 AC-001/002, AUTH-02/03/04/06/07. Real archive, durable installation,
//! managed RuntimeExtensionHost workers and exact contribution grants. No fake registry.
use crate::provider_runtime::{ApiProviderRuntime, ApiRuntimeArtifactResolver, ApiRuntimeServices};
use async_trait::async_trait;
use control_plane::{plugin_management::*, ports::AuthRepository};
use plugin_framework::extension_bus::*;
use runtime_core::runtime_backend::{
    RuntimeArtifactReference, RuntimeBackendError, RuntimeExecutionPrincipal,
};
use runtime_extension_host::{RuntimeArtifactResolver, RuntimeExtensionHost};
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::sync::Notify;
use uuid::Uuid;

struct ResolvingBarrier {
    inner: ApiRuntimeArtifactResolver,
    armed: AtomicBool,
    entered: Notify,
    release: Notify,
}
#[async_trait]
impl RuntimeArtifactResolver for ResolvingBarrier {
    async fn resolve(
        &self,
        artifact: &RuntimeArtifactReference,
    ) -> Result<PathBuf, RuntimeBackendError> {
        if self.armed.swap(false, Ordering::SeqCst) {
            self.entered.notify_one();
            self.release.notified().await;
        }
        self.inner.resolve(artifact).await
    }
}

fn package_bytes(root: &Path) -> Vec<u8> {
    let mut archive = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    ));
    for relative in ["manifest.yaml", "bin/worker.py"] {
        archive
            .append_path_with_name(root.join(relative), relative)
            .unwrap();
    }
    archive.into_inner().unwrap().finish().unwrap()
}

fn principal(workspace: Uuid, actor: Uuid) -> RuntimeExecutionPrincipal {
    RuntimeExecutionPrincipal {
        workspace_id: workspace.to_string(),
        actor_id: Some(actor.to_string()),
        deadline_unix_ms: ((time::OffsetDateTime::now_utc() + time::Duration::seconds(30))
            .unix_timestamp_nanos()
            / 1_000_000) as i64,
    }
}
fn grant(contribution: &str) -> GrantContributionPermission {
    let data = contribution == "data";
    GrantContributionPermission {
        contribution_id: format!("acme.composition-a.{contribution}"),
        permission: if data {
            "plugin_data.owned.write"
        } else {
            "hook.model_definitions.create.before"
        }
        .into(),
        resource_scope: if data {
            domain::ContributionResourceScope::OwnedCollection {
                collection_code: "processed_models".into(),
            }
        } else {
            domain::ContributionResourceScope::Workspace
        },
        permission_contract_id: if data { "plugin-data" } else { "managed-hook" }.into(),
        permission_contract_version: "1".into(),
    }
}

#[tokio::test]
async fn root_2007_ac_001_002_package_activation() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let actor_id: Uuid = sqlx::query_scalar("select id from users where account='root'")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let actor = state
        .store
        .load_actor_context_for_user(actor_id)
        .await
        .unwrap();
    let assembly = crate::extension_bus::assemble_extension_graph_input(
        crate::api_workspace_root().unwrap(),
        crate::extension_bus::DEFAULT_PLUGIN_SET_PATH,
        vec![],
    )
    .unwrap();
    let resolver = Arc::new(ResolvingBarrier {
        inner: ApiRuntimeArtifactResolver::new(
            state.store.clone(),
            &state.api_node_id,
            &state.provider_install_root,
        ),
        armed: AtomicBool::new(false),
        entered: Notify::new(),
        release: Notify::new(),
    });
    let host = Arc::new(
        RuntimeExtensionHost::new_with_artifact_resolver(
            time::OffsetDateTime::now_utc(),
            resolver.clone(),
        )
        .unwrap(),
    );
    host.mark_ready().unwrap();
    let services = Arc::new(
        ApiRuntimeServices::new_with_runtime_backend(
            host,
            Arc::new(assembly.compile_graph().unwrap()),
        )
        .unwrap()
        .with_managed_composition(
            state.store.clone(),
            state.api_node_id.clone(),
            assembly.module_descriptors().to_vec(),
        ),
    );
    let composition = services.managed_composition().unwrap();
    let management = Arc::new(
        PluginManagementService::new(
            state.store.clone(),
            ApiProviderRuntime::new(services.clone()),
            state.official_plugin_source.clone(),
            &state.provider_install_root,
        )
        .with_node_id(&state.api_node_id),
    );
    let package = crate::api_workspace_root()
        .unwrap()
        .join("plugins/fixtures/acme.composition-a");
    let installed = management
        .install_uploaded_plugin(InstallUploadedPluginCommand {
            actor_user_id: actor_id,
            file_name: "acme.composition-a.1flowbasepkg".into(),
            package_bytes: package_bytes(&package),
        })
        .await
        .unwrap();
    let installation_id = installed.installation.id;
    assert_eq!(
        installed.installation.desired_state,
        domain::PluginDesiredState::Disabled
    );
    assert!(installed
        .local_artifact
        .package_path
        .as_ref()
        .is_some_and(|path| Path::new(path).is_file()));
    management
        .assign_plugin(AssignPluginCommand {
            actor_user_id: actor_id,
            installation_id,
        })
        .await
        .unwrap();
    let authority = PluginContributionAuthorityService::new(
        state.store.clone(),
        HostContributionGrantPolicy::root_composition(),
    );
    assert!(authority
        .query(&actor, installation_id)
        .await
        .unwrap()
        .authorizations
        .is_empty());
    assert!(management
        .enable_plugin(EnablePluginCommand {
            actor_user_id: actor_id,
            installation_id
        })
        .await
        .is_err());
    assert!(composition
        .snapshot(actor.current_workspace_id)
        .await
        .is_none());
    authority
        .grant(&actor, installation_id, grant("first"))
        .await
        .unwrap();
    authority
        .grant(&actor, installation_id, grant("data"))
        .await
        .unwrap();
    // Both executables request the same permission; granting first still cannot admit second.
    assert!(management
        .enable_plugin(EnablePluginCommand {
            actor_user_id: actor_id,
            installation_id
        })
        .await
        .is_err());
    assert!(composition
        .snapshot(actor.current_workspace_id)
        .await
        .is_none());
    authority
        .grant(&actor, installation_id, grant("second"))
        .await
        .unwrap();
    management
        .enable_plugin(EnablePluginCommand {
            actor_user_id: actor_id,
            installation_id,
        })
        .await
        .unwrap();
    let first_snapshot = composition
        .snapshot(actor.current_workspace_id)
        .await
        .unwrap();
    assert_eq!(first_snapshot.bindings.len(), 3);
    assert_eq!(first_snapshot.authority.contributions.len(), 3);
    for name in ["first", "second"] {
        let id = ContributionId::new(format!("acme.composition-a.{name}")).unwrap();
        let output = composition.execute(actor.current_workspace_id, &id, principal(actor.current_workspace_id, actor_id), json!({}), json!({"contribution_id":"forged", "workspace_id":"foreign", "granted_permissions":["admin"]})).await.unwrap();
        assert_eq!(output["worker"], name);
    }
    let first = ContributionId::new("acme.composition-a.first").unwrap();
    assert!(composition
        .execute(
            actor.current_workspace_id,
            &first,
            principal(Uuid::now_v7(), actor_id),
            json!({}),
            json!({})
        )
        .await
        .is_err());
    assert!(composition
        .execute(
            Uuid::now_v7(),
            &first,
            principal(actor.current_workspace_id, actor_id),
            json!({}),
            json!({})
        )
        .await
        .is_err());
    let granted = authority.query(&actor, installation_id).await.unwrap();
    let first_id = granted
        .authorizations
        .iter()
        .find(|grant| grant.contribution_id == first.as_str())
        .unwrap()
        .id;
    // Freeze a genuinely prepared candidate at the real resolver seam, then revoke before
    // its final publication. No timing inference or fake graph publication is involved.
    resolver.armed.store(true, Ordering::SeqCst);
    let candidate_owner = composition.clone();
    let candidate =
        tokio::spawn(async move { candidate_owner.rebuild_installation(installation_id).await });
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        resolver.entered.notified(),
    )
    .await
    .unwrap();
    authority
        .revoke(
            &actor,
            installation_id,
            RevokeContributionPermission {
                authorization_id: first_id,
                expected_revision: granted.revision,
            },
        )
        .await
        .unwrap();
    resolver.release.notify_one();
    assert!(candidate.await.unwrap().is_err());
    assert!(Arc::ptr_eq(
        &first_snapshot,
        &composition
            .snapshot(actor.current_workspace_id)
            .await
            .unwrap()
    ));
    assert!(composition
        .execute(
            actor.current_workspace_id,
            &first,
            principal(actor.current_workspace_id, actor_id),
            json!({}),
            json!({})
        )
        .await
        .is_err());
    // Existing immutable graph can use a newly granted exact permission, never a stale grant.
    let regranted = authority
        .grant(&actor, installation_id, grant("first"))
        .await
        .unwrap();
    let marker_root = std::env::temp_dir().join(format!("root-2007-admission-{}", Uuid::now_v7()));
    std::fs::create_dir(&marker_root).unwrap();
    let marker = marker_root.join("worker");
    let running_owner = composition.clone();
    let running_first = first.clone();
    let workspace_id = actor.current_workspace_id;
    let running_marker = marker.clone();
    let running = tokio::spawn(async move {
        running_owner
            .execute(
                workspace_id,
                &running_first,
                principal(workspace_id, actor_id),
                json!({}),
                json!({"fixture_barrier":running_marker}),
            )
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while !marker.with_extension("started").is_file() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    // The worker is admitted and deliberately slow. Revoke must commit before its completion.
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        authority.revoke(
            &actor,
            installation_id,
            RevokeContributionPermission {
                authorization_id: first_id,
                expected_revision: regranted.revision,
            },
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(!running.is_finished());
    assert!(composition
        .execute(
            workspace_id,
            &first,
            principal(workspace_id, actor_id),
            json!({}),
            json!({})
        )
        .await
        .is_err());
    std::fs::write(marker.with_extension("release"), "release").unwrap();
    assert_eq!(running.await.unwrap().unwrap()["worker"], "first");
    std::fs::remove_dir_all(marker_root).unwrap();
    authority
        .grant(&actor, installation_id, grant("first"))
        .await
        .unwrap();
    // A real execution-binding failure preserves the complete old graph and bindings.
    let worker =
        Path::new(installed.local_artifact.local_path.as_ref().unwrap()).join("bin/worker.py");
    let saved = worker.with_extension("saved");
    std::fs::rename(&worker, &saved).unwrap();
    let failure = composition.rebuild_installation(installation_id).await;
    std::fs::rename(&saved, &worker).unwrap();
    assert!(failure.is_err());
    assert!(Arc::ptr_eq(
        &first_snapshot,
        &composition.snapshot(workspace_id).await.unwrap()
    ));
    composition
        .rebuild_installation(installation_id)
        .await
        .unwrap();
    let rebuilt = composition.snapshot(workspace_id).await.unwrap();
    assert_ne!(
        first_snapshot.graph.fingerprint(),
        rebuilt.graph.fingerprint()
    );
}
