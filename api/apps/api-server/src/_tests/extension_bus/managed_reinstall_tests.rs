//! Root #2007 IR-01/02/03, AC-001/002/008/009, AUTH-02/05/06/08/09/10.
//! Official archive intake, real SDK event execution, durable Create and authenticated governance.
//! No behavior test is skipped when the prebuilt MANAGED_EVENT_WORKER_FIXTURE is absent.
use super::managed_event_authority_tests::{grant, manifest, package};
use super::managed_snapshot_tests::{create, RuntimeFixture};
use crate::_tests::support::{login_and_capture_cookie, test_api_state_with_database_url};
use crate::provider_runtime::ApiProviderRuntime;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use control_plane::{
    errors::ControlPlaneError, lifecycle_outbox_dispatcher::LifecycleFactDeliveryPort,
    plugin_management::*, ports::AuthRepository,
};
use control_plane_contracts::ports::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tower::ServiceExt;
use uuid::Uuid;

struct Fixture {
    state: Arc<crate::app_state::ApiState>,
    runtime: RuntimeFixture,
    actor: domain::ActorContext,
    management: Arc<crate::app_state::ApiPluginManagementService>,
}
impl Fixture {
    async fn new(max_connections: u32) -> Self {
        let worker = PathBuf::from(
            std::env::var_os("MANAGED_EVENT_WORKER_FIXTURE")
                .expect("CI must prebuild the real managed_event_worker SDK example"),
        );
        assert!(
            worker.is_absolute() && worker.is_file(),
            "real SDK executable required"
        );
        let (initial, database_url) = test_api_state_with_database_url().await;
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(max_connections)
            .connect(&database_url)
            .await
            .unwrap();
        let mut state = (*initial).clone();
        state.store = storage_durable_postgres::PgControlPlaneStore::new(pool);
        let runtime = RuntimeFixture::new(&state);
        state.store = runtime.store.clone();
        let actor_id: Uuid = sqlx::query_scalar("select id from users where account='root'")
            .fetch_one(runtime.store.pool())
            .await
            .unwrap();
        let actor = AuthRepository::load_actor_context_for_user(&runtime.store, actor_id)
            .await
            .unwrap();
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
            state: Arc::new(state),
            runtime,
            actor,
            management,
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
    async fn install(&self, bytes: &[u8]) -> domain::PluginInstallationRecord {
        // Includes task/audit writes after the same-connection installation lease is released.
        tokio::time::timeout(
            Duration::from_secs(20),
            self.management
                .install_uploaded_plugin(upload(self.actor.user_id, bytes)),
        )
        .await
        .expect("official install/restore must finish even with pool max_connections=1")
        .unwrap()
        .installation
    }
    async fn activate(&self, id: Uuid) -> Uuid {
        self.management
            .assign_plugin(AssignPluginCommand {
                actor_user_id: self.actor.user_id,
                installation_id: id,
            })
            .await
            .unwrap();
        for permission in ["event.subscribe", "event.publish"] {
            self.authority()
                .grant(&self.actor, id, grant("a", permission))
                .await
                .unwrap();
        }
        self.management
            .enable_plugin(EnablePluginCommand {
                actor_user_id: self.actor.user_id,
                installation_id: id,
            })
            .await
            .unwrap();
        // A publishes processed events; the production publication owner requires a real target.
        // Keep B active across A's version switch so both actual A workers can publish.
        let subscriber = self.install(&package(&manifest("b"))).await.id;
        self.management
            .assign_plugin(AssignPluginCommand {
                actor_user_id: self.actor.user_id,
                installation_id: subscriber,
            })
            .await
            .unwrap();
        self.authority()
            .grant(&self.actor, subscriber, grant("b", "event.subscribe"))
            .await
            .unwrap();
        self.management
            .enable_plugin(EnablePluginCommand {
                actor_user_id: self.actor.user_id,
                installation_id: subscriber,
            })
            .await
            .unwrap();
        subscriber
    }
    async fn artifact(&self, id: Uuid) -> domain::PluginArtifactInstanceRecord {
        self.runtime
            .store
            .get_artifact_instance(&self.state.api_node_id, id)
            .await
            .unwrap()
            .unwrap()
    }
    async fn backlog(&self, id: Uuid) -> LifecycleOutboxRecord {
        create(
            &self.runtime.store,
            self.actor.user_id,
            self.actor.current_workspace_id,
        )
        .await;
        let worker = Uuid::now_v7();
        let records = self
            .runtime
            .store
            .claim_lifecycle_facts(worker, 32, time::Duration::minutes(5))
            .await
            .unwrap();
        let old = records
            .iter()
            .find(|r| managed_handler_installation(&r.handler_version) == Some(id))
            .unwrap()
            .clone();
        // Execute A's actual SDK process and persist its derived event, deliberately lose its ACK.
        for record in &records {
            self.runtime.delivery.deliver(record).await.unwrap();
            if record.subscriber_id != old.subscriber_id {
                self.runtime
                    .store
                    .mark_lifecycle_fact_delivered(
                        record.event_id,
                        &record.subscriber_id,
                        worker,
                        record.claim_id.unwrap(),
                    )
                    .await
                    .unwrap();
            }
        }
        let path =
            PathBuf::from(self.artifact(id).await.local_path.unwrap()).join("bin/worker.trace");
        let trace = std::fs::read_to_string(path).unwrap();
        let frame: extension_contracts::ManagedEventHostFrame =
            serde_json::from_str(trace.lines().last().unwrap()).unwrap();
        assert_eq!(frame.graph_fingerprint, old.graph_fingerprint);
        assert_eq!(
            frame
                .execution_identity
                .subject()
                .installation_id()
                .as_str(),
            id.to_string()
        );
        assert_eq!(frame.handler, "publish");
        let published: i64 = sqlx::query_scalar("select count(*) from lifecycle_outbox where contract_id='acme.composition-a.processed'")
            .fetch_one(self.runtime.store.pool()).await.unwrap();
        assert!(published > 0);
        old
    }
    async fn disable(&self, id: Uuid) {
        self.management
            .disable_plugin(DisablePluginCommand {
                actor_user_id: self.actor.user_id,
                installation_id: id,
            })
            .await
            .unwrap();
    }
    async fn stable_state(&self, id: Uuid) -> Value {
        // Read-only evidence excludes timestamps/task audit rows that an attempted install owns.
        let installation: Value = sqlx::query_scalar("select jsonb_build_object('id',id,'plugin_id',plugin_id,'desired_state',desired_state,'metadata_json',metadata_json,'expected_checksum',expected_checksum) from extension_installations where id=$1")
            .bind(id).fetch_one(self.runtime.store.pool()).await.unwrap();
        let authority = self.authority().query(&self.actor, id).await.unwrap();
        let deliveries: Value = sqlx::query_scalar("select coalesce(jsonb_agg(to_jsonb(d) order by event_id,subscriber_id),'[]'::jsonb) from lifecycle_outbox_deliveries d")
            .fetch_one(self.runtime.store.pool()).await.unwrap();
        let facts: Value = sqlx::query_scalar("select coalesce(jsonb_agg(to_jsonb(f) order by event_id),'[]'::jsonb) from lifecycle_outbox f")
            .fetch_one(self.runtime.store.pool()).await.unwrap();
        json!({"installation":installation,"authority":authority,"deliveries":deliveries,"facts":facts})
    }
}
fn upload(actor_user_id: Uuid, bytes: &[u8]) -> InstallUploadedPluginCommand {
    InstallUploadedPluginCommand {
        actor_user_id,
        file_name: "managed-reinstall.1flowbasepkg".into(),
        package_bytes: bytes.to_vec(),
    }
}
fn changed_manifest() -> Value {
    let mut changed = manifest("a");
    // Both sides move together: official parser/intake must accept this valid changed package.
    changed["managed"]["module"]["contributions"][0]["contribution_id"] =
        "acme.composition-a.events2".into();
    changed["managed"]["execution_bindings"][0]["contribution_id"] =
        "acme.composition-a.events2".into();
    changed
}
fn conflict(error: anyhow::Error, expected: &str) {
    match error.downcast_ref::<ControlPlaneError>() {
        Some(ControlPlaneError::Conflict(code)) => assert_eq!(*code, expected, "{error:#}"),
        _ => panic!("expected Conflict({expected}), received {error:#}"),
    }
}
fn tree(root: &Path) -> BTreeMap<PathBuf, (u32, Vec<u8>)> {
    use std::os::unix::fs::PermissionsExt;
    fn collect(root: &Path, path: &Path, result: &mut BTreeMap<PathBuf, (u32, Vec<u8>)>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let metadata = entry.metadata().unwrap();
            let bytes = if metadata.is_dir() {
                Vec::new()
            } else {
                std::fs::read(&path).unwrap()
            };
            result.insert(
                path.strip_prefix(root).unwrap().to_owned(),
                (metadata.permissions().mode(), bytes),
            );
            if metadata.is_dir() {
                collect(root, &path, result);
            }
        }
    }
    let mut result = BTreeMap::new();
    collect(root, root, &mut result);
    result
}
fn exact(record: &LifecycleOutboxRecord) -> ResumeManagedLifecycleDelivery {
    ResumeManagedLifecycleDelivery {
        event_id: record.event_id,
        subscriber_id: record.subscriber_id.clone(),
        expected: ManagedFrozenExecutionTarget {
            graph_fingerprint: record.graph_fingerprint.clone(),
            handler_id: record.handler_id.clone(),
            handler_version: record.handler_version.clone(),
        },
    }
}
async fn request(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    method: &str,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|e| panic!("{method} {path}: {status}, invalid JSON {e}"))
    };
    (status, value)
}

#[tokio::test]
async fn root_2007_ir_f01_changed_reinstall_preserves_history() {
    let fixture = Fixture::new(8).await;
    let bytes = package(&manifest("a"));
    let installed = fixture.install(&bytes).await;
    let id = installed.id;
    let subscriber = fixture.activate(id).await;
    let old = fixture.backlog(id).await;
    // B depends on A's event point: stop new B bindings first, retaining its durable backlog.
    fixture.disable(subscriber).await;
    fixture.disable(id).await;
    let before = fixture.stable_state(id).await;
    let artifact = fixture.artifact(id).await;
    let directory = PathBuf::from(artifact.local_path.as_ref().unwrap());
    let archive = PathBuf::from(artifact.package_path.as_ref().unwrap());
    let original_tree = tree(&directory);
    let original_archive = std::fs::read(&archive).unwrap();
    assert_eq!(original_archive, bytes);
    let replacement = package(&changed_manifest());
    assert_ne!(replacement, bytes);
    conflict(
        fixture
            .management
            .install_uploaded_plugin(upload(fixture.actor.user_id, &replacement))
            .await
            .unwrap_err(),
        "managed_archive_identity_conflict",
    );
    // A legal v1 single-slot input cannot erase an existing managed identity either.
    let mut legacy = manifest("a");
    legacy.as_object_mut().unwrap().remove("managed");
    legacy["manifest_version"] = 1.into();
    legacy["schema_version"] = "1flowbase.plugin.manifest/v1".into();
    legacy["contract_version"] = "1flowbase.data_source/v1".into();
    legacy["slot_codes"] = json!(["data_source"]);
    let legacy_bytes = package(&legacy);
    conflict(
        fixture
            .management
            .install_uploaded_plugin(upload(fixture.actor.user_id, &legacy_bytes))
            .await
            .unwrap_err(),
        "managed_archive_identity_conflict",
    );
    assert_eq!(fixture.stable_state(id).await, before);
    assert_eq!(fixture.artifact(id).await, artifact);
    assert_eq!(tree(&directory), original_tree);
    assert_eq!(std::fs::read(&archive).unwrap(), original_archive);
    let state = fixture.state.clone();
    let actor = fixture.actor.clone();
    fixture.runtime.host.stop().await.unwrap();
    drop(fixture);
    // No retained Arc/host/registry survives reconstruction. Disabled installation builds no binding.
    let restarted = RuntimeFixture::new(&state);
    restarted
        .composition
        .rebuild_installation(id)
        .await
        .unwrap();
    if let Some(snapshot) = restarted
        .composition
        .snapshot(actor.current_workspace_id)
        .await
    {
        assert!(snapshot
            .bindings
            .values()
            .all(|b| b.handle.identity().installation_id().as_str() != id.to_string()));
    }
    let mut state = (*state).clone();
    state.store = restarted.store.clone();
    state.provider_runtime = restarted.services.clone();
    let app = crate::app_with_state(Arc::new(state));
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let base = format!("/api/console/settings/extension-center/installed/{id}");
    let (status, view) = request(
        &app,
        &cookie,
        &csrf,
        "GET",
        &format!("{base}/managed-execution"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{view}");
    let row = view["data"]["deliveries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| {
            r["event_id"] == old.event_id.to_string() && r["subscriber_id"] == old.subscriber_id
        })
        .unwrap();
    assert_eq!(row["status"], "paused");
    assert_eq!(row["pause_reason"], "installation_inactive");
    assert_eq!(
        row["target"],
        serde_json::to_value(&exact(&old).expected).unwrap()
    );
    let persisted = restarted
        .store
        .managed_lifecycle_delivery(id, actor.current_workspace_id, &exact(&old))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(persisted.status, LifecycleOutboxStatus::Paused);
    assert_eq!(persisted.handler_version, old.handler_version);
    let (status, error) = request(
        &app,
        &cookie,
        &csrf,
        "POST",
        &format!("{base}/lifecycle-deliveries/resume"),
        serde_json::to_value(exact(&old)).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{error}");
    assert_eq!(error["code"], "managed_frozen_graph_unavailable", "{error}");
    let guard = restarted
        .composition
        .guard_managed_artifact_removal(&[id])
        .await;
    conflict(
        guard.err().expect("durable backlog must prevent removal"),
        "managed_artifact_has_durable_deliveries",
    );
    let (status, error) = request(&app, &cookie, &csrf, "DELETE", &base, Value::Null).await;
    assert_eq!(status, StatusCode::CONFLICT, "{error}");
    assert_eq!(
        error["code"], "managed_artifact_has_durable_deliveries",
        "{error}"
    );
    assert!(restarted
        .store
        .managed_installation_has_backlog(id, None, None)
        .await
        .unwrap());
    assert!(restarted
        .store
        .get_installation(id)
        .await
        .unwrap()
        .is_some());
    assert_eq!(tree(&directory), original_tree);
    assert_eq!(std::fs::read(&archive).unwrap(), original_archive);
    restarted.host.stop().await.unwrap();
}

#[tokio::test]
async fn root_2007_ir_f01_identical_archive_restores_artifact() {
    let fixture = Fixture::new(1).await;
    let bytes = package(&manifest("a"));
    let installed = fixture.install(&bytes).await;
    let id = installed.id;
    let subscriber = fixture.activate(id).await;
    let old = fixture.backlog(id).await;
    // B depends on A's event point: stop new B bindings first, retaining its durable backlog.
    fixture.disable(subscriber).await;
    fixture.disable(id).await;
    let before = fixture.stable_state(id).await;
    let artifact = fixture.artifact(id).await;
    let directory = PathBuf::from(artifact.local_path.as_ref().unwrap());
    let archive = PathBuf::from(artifact.package_path.as_ref().unwrap());
    let executable = std::fs::read(directory.join("bin/worker.py")).unwrap();
    let manifest_bytes = std::fs::read(directory.join("manifest.yaml")).unwrap();
    // Exact original bytes are a legitimate restore despite paused durable backlog.
    assert_eq!(fixture.install(&bytes).await.id, id);
    assert_eq!(fixture.stable_state(id).await, before);
    std::fs::remove_dir_all(&directory).unwrap();
    std::fs::remove_file(&archive).unwrap();
    let restored = fixture.install(&bytes).await;
    assert_eq!(restored.id, id);
    assert_eq!(restored.desired_state, domain::PluginDesiredState::Disabled);
    assert_eq!(restored.expected_checksum, installed.expected_checksum);
    assert_eq!(fixture.stable_state(id).await, before);
    assert_eq!(std::fs::read(&archive).unwrap(), bytes);
    assert_eq!(
        std::fs::read(directory.join("bin/worker.py")).unwrap(),
        executable
    );
    assert_eq!(
        std::fs::read(directory.join("manifest.yaml")).unwrap(),
        manifest_bytes
    );
    use std::os::unix::fs::PermissionsExt;
    assert_ne!(
        std::fs::metadata(directory.join("bin/worker.py"))
            .unwrap()
            .permissions()
            .mode()
            & 0o111,
        0
    );
    assert!(fixture
        .runtime
        .store
        .managed_lifecycle_delivery(id, fixture.actor.current_workspace_id, &exact(&old))
        .await
        .unwrap()
        .is_some());
    assert_eq!(
        fixture.artifact(id).await.local_checksum,
        installed.expected_checksum
    );
    let state = ManagedExecutionService::new(
        fixture.runtime.store.clone(),
        fixture.runtime.composition.governance(),
    )
    .query(&fixture.actor, id)
    .await
    .unwrap();
    assert!(state
        .deliveries
        .iter()
        .any(|r| r.event_id == old.event_id && r.subscriber_id == old.subscriber_id));
    // A changed gzip timestamp keeps all decompressed content identical but changes archive identity.
    let mut repacked = bytes.clone();
    assert_eq!(&repacked[..3], &[0x1f, 0x8b, 8]);
    repacked[4] ^= 1;
    conflict(
        fixture
            .management
            .install_uploaded_plugin(upload(fixture.actor.user_id, &repacked))
            .await
            .unwrap_err(),
        "managed_archive_identity_conflict",
    );
    assert_eq!(fixture.stable_state(id).await, before);
    assert_eq!(std::fs::read(&archive).unwrap(), bytes);
    // Explicit old-data compatibility fixture ONLY: no authoritative historical checksum.
    // SQL does not simulate installation; the subsequent operation is again official upload intake.
    sqlx::query("update extension_installations set expected_checksum=null where id=$1")
        .bind(id)
        .execute(fixture.runtime.store.pool())
        .await
        .unwrap();
    let historical = fixture.stable_state(id).await;
    let original_tree = tree(&directory);
    conflict(
        fixture
            .management
            .install_uploaded_plugin(upload(fixture.actor.user_id, &bytes))
            .await
            .unwrap_err(),
        "managed_archive_identity_conflict",
    );
    assert_eq!(fixture.stable_state(id).await, historical);
    assert_eq!(tree(&directory), original_tree);
    assert_eq!(std::fs::read(&archive).unwrap(), bytes);
    fixture.runtime.host.stop().await.unwrap();
}

#[tokio::test]
async fn root_2007_ir_f01_upgrade_and_concurrent_identity() {
    let fixture = Fixture::new(8).await;
    let original = package(&manifest("a"));
    let source = fixture.install(&original).await.id;
    fixture.activate(source).await;
    let old = fixture.backlog(source).await;
    let source_authority = fixture
        .authority()
        .query(&fixture.actor, source)
        .await
        .unwrap();
    let mut candidate = manifest("a");
    candidate["version"] = "1.1.0".into();
    candidate["managed"]["module"]["module_version"] = "1.1.0".into();
    let target = fixture.install(&package(&candidate)).await.id;
    assert_ne!(source, target);
    let switch = || SwitchPluginVersionCommand {
        actor_user_id: fixture.actor.user_id,
        provider_code: "acme.composition-a".into(),
        target_installation_id: target,
    };
    assert!(
        fixture.management.switch_version(switch()).await.is_err(),
        "source authorization cannot authorize candidate"
    );
    for permission in ["event.subscribe", "event.publish"] {
        fixture
            .authority()
            .grant(&fixture.actor, target, grant("a", permission))
            .await
            .unwrap();
    }
    let candidate_authority = fixture
        .authority()
        .query(&fixture.actor, target)
        .await
        .unwrap();
    assert!(candidate_authority
        .authorizations
        .iter()
        .all(|a| source_authority.authorizations.iter().all(|b| a.id != b.id)));
    let task = fixture.management.switch_version(switch()).await.unwrap();
    assert_eq!(task.status, domain::PluginTaskStatus::Succeeded);
    assert_eq!(
        fixture
            .runtime
            .store
            .list_assignments(fixture.actor.current_workspace_id)
            .await
            .unwrap()
            .into_iter()
            .find(|a| a.provider_code == "acme.composition-a")
            .unwrap()
            .installation_id,
        target
    );
    let current = fixture.backlog(target).await;
    assert_ne!(current.graph_fingerprint, old.graph_fingerprint);
    assert_ne!(current.handler_version, old.handler_version);
    assert_eq!(
        managed_handler_installation(&old.handler_version),
        Some(source)
    );
    assert_eq!(
        managed_handler_installation(&current.handler_version),
        Some(target)
    );
    assert!(fixture
        .runtime
        .store
        .managed_lifecycle_delivery(source, fixture.actor.current_workspace_id, &exact(&old))
        .await
        .unwrap()
        .is_some());
    assert!(fixture
        .runtime
        .store
        .managed_lifecycle_delivery(target, fixture.actor.current_workspace_id, &exact(&old))
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        fixture
            .authority()
            .query(&fixture.actor, source)
            .await
            .unwrap(),
        source_authority
    );
    // Current revocation remains authoritative and is independent of source's frozen target.
    let subscribed = candidate_authority
        .authorizations
        .iter()
        .find(|a| a.permission == "event.subscribe")
        .unwrap();
    fixture
        .authority()
        .revoke(
            &fixture.actor,
            target,
            RevokeContributionPermission {
                authorization_id: subscribed.id,
                expected_revision: candidate_authority.revision,
            },
        )
        .await
        .unwrap();
    let denied = fixture
        .runtime
        .delivery
        .deliver(&current)
        .await
        .unwrap_err();
    assert_eq!(
        denied.downcast_ref::<LifecycleDeliveryBlocked>().unwrap().0,
        LifecycleDeliveryPauseReason::AuthorityRevoked
    );
    assert_eq!(
        fixture
            .authority()
            .query(&fixture.actor, source)
            .await
            .unwrap(),
        source_authority
    );

    // Both first-install contenders reach the same real PostgreSQL admission barrier before release.
    // This pool has eight connections; pool1 above never holds an external lease while acquiring.
    let mut left = manifest("a");
    left["version"] = "2.0.0".into();
    left["managed"]["module"]["module_version"] = "2.0.0".into();
    let mut right = changed_manifest();
    right["version"] = "2.0.0".into();
    right["managed"]["module"]["module_version"] = "2.0.0".into();
    let contenders = [package(&left), package(&right)];
    assert_ne!(contenders[0], contenders[1]);
    let identity = "acme.composition-a@2.0.0";
    let mut barrier = fixture.runtime.store.pool().acquire().await.unwrap();
    sqlx::query("select pg_advisory_lock(hashtextextended($1,2007))")
        .bind(identity)
        .execute(&mut *barrier)
        .await
        .unwrap();
    let tasks = contenders
        .iter()
        .map(|bytes| {
            let management = fixture.management.clone();
            let command = upload(fixture.actor.user_id, bytes);
            tokio::spawn(async move { management.install_uploaded_plugin(command).await })
        })
        .collect::<Vec<_>>();
    let reached_barrier = tokio::time::timeout(Duration::from_secs(20),async{loop{
        let waiting:i64=sqlx::query_scalar("select count(*) from pg_locks where locktype='advisory' and not granted and database=(select oid from pg_database where datname=current_database()) and classid=((hashtextextended($1,2007)>>32)&4294967295)::oid and objid=(hashtextextended($1,2007)&4294967295)::oid and objsubid=1")
            .bind(identity).fetch_one(fixture.runtime.store.pool()).await.unwrap();
        if waiting==2{break;} tokio::task::yield_now().await;
    }}).await;
    sqlx::query("select pg_advisory_unlock(hashtextextended($1,2007))")
        .bind(identity)
        .execute(&mut *barrier)
        .await
        .unwrap();
    drop(barrier);
    reached_barrier.expect("both independent uploads must wait on the exact identity lock");
    let mut winner = None;
    let mut rejected = 0;
    for (index, task) in tasks.into_iter().enumerate() {
        match tokio::time::timeout(Duration::from_secs(20), task)
            .await
            .unwrap()
            .unwrap()
        {
            Ok(result) => {
                assert!(
                    winner.is_none(),
                    "different archives cannot both claim one identity"
                );
                winner = Some((index, result));
            }
            Err(error) => {
                conflict(error, "managed_archive_identity_conflict");
                rejected += 1;
            }
        }
    }
    assert_eq!(rejected, 1);
    let (index, installed) = winner.unwrap();
    let bytes = &contenders[index];
    let digest = format!("sha256:{:x}", Sha256::digest(bytes));
    assert_eq!(
        installed.installation.expected_checksum.as_deref(),
        Some(digest.as_str())
    );
    let persistent = fixture
        .runtime
        .store
        .get_installation(installed.installation.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        persistent.metadata_json,
        installed.installation.metadata_json
    );
    assert_eq!(
        persistent.expected_checksum,
        installed.installation.expected_checksum
    );
    let artifact = fixture.artifact(persistent.id).await;
    assert_eq!(artifact.local_checksum.as_deref(), Some(digest.as_str()));
    assert_eq!(
        std::fs::read(artifact.package_path.unwrap()).unwrap(),
        *bytes
    );
    let installed_directory = PathBuf::from(artifact.local_path.as_ref().unwrap());
    let source_worker = PathBuf::from(std::env::var_os("MANAGED_EVENT_WORKER_FIXTURE").unwrap());
    assert_eq!(
        std::fs::read(installed_directory.join("bin/worker.py")).unwrap(),
        std::fs::read(source_worker).unwrap()
    );
    let unpacked: Value = serde_yaml::from_slice(
        &std::fs::read(PathBuf::from(artifact.local_path.unwrap()).join("manifest.yaml")).unwrap(),
    )
    .unwrap();
    assert_eq!(unpacked, if index == 0 { left } else { right });
    let parsed =
        plugin_framework::parse_plugin_manifest(&serde_yaml::to_string(&unpacked).unwrap())
            .unwrap();
    assert_eq!(
        persistent.metadata_json["managed"],
        serde_json::to_value(parsed.managed.unwrap()).unwrap()
    );
    let count: i64 =
        sqlx::query_scalar("select count(*) from extension_installations where plugin_id=$1")
            .bind(identity)
            .fetch_one(fixture.runtime.store.pool())
            .await
            .unwrap();
    assert_eq!(count, 1);
    fixture.runtime.host.stop().await.unwrap();
}
