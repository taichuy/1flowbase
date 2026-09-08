//! Root 2007 AC-008/009 AUTH-05/09/10: actual console bindings, installed SDK and PG commit owners.
use super::managed_event_authority_tests::{grant, manifest, package};
use super::managed_snapshot_tests::{create, RuntimeFixture};
use crate::_tests::support::*;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use control_plane::{
    lifecycle_outbox_dispatcher::LifecycleFactDeliveryPort, plugin_management::*,
    ports::AuthRepository,
};
use control_plane_contracts::ports::*;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

// Small pre-existing fixture histories must remain complete; production callers use the page
// and never turn it into a full-history proof.
async fn deliveries(
    runtime: &RuntimeFixture,
    installation: Uuid,
    workspace: Uuid,
) -> Vec<LifecycleOutboxRecord> {
    let page = runtime
        .store
        .managed_lifecycle_delivery_page(installation, workspace)
        .await
        .unwrap();
    assert!(!page.truncated);
    let mut records = Vec::new();
    for row in page.deliveries {
        records.push(
            runtime
                .store
                .managed_lifecycle_delivery(
                    installation,
                    workspace,
                    &ResumeManagedLifecycleDelivery {
                        event_id: row.event_id,
                        subscriber_id: row.subscriber_id,
                        expected: row.target,
                    },
                )
                .await
                .unwrap()
                .unwrap(),
        );
    }
    records
}

async fn insert_completed_history(
    runtime: &RuntimeFixture,
    record: &LifecycleOutboxRecord,
) -> Vec<Uuid> {
    let ids = (1..=MANAGED_DELIVERY_PAGE_LIMIT + 40)
        .map(|i| Uuid::from_u128(i as u128))
        .collect::<Vec<_>>();
    // Deliberately invalid payload bytes prove metadata reads do not decode unrelated history.
    sqlx::query("insert into lifecycle_outbox(event_id,transaction_id,contract_id,contract_version,canonical_payload,occurred_at,graph_fingerprint,status,delivered_at) select id,id,$2,$3,decode('ff','hex'),now(),$4,'delivered',now() from unnest($1::uuid[]) id")
        .bind(&ids).bind(&record.contract_id).bind(&record.contract_version).bind(&record.graph_fingerprint)
        .execute(runtime.store.pool()).await.unwrap();
    sqlx::query("insert into lifecycle_outbox_deliveries(event_id,subscriber_id,handler_id,handler_version,status,delivered_at) select id,$2,$3,$4,'delivered',now() from unnest($1::uuid[]) id")
        .bind(&ids).bind(&record.subscriber_id).bind(&record.handler_id).bind(&record.handler_version)
        .execute(runtime.store.pool()).await.unwrap();
    ids
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
    (
        status,
        if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        },
    )
}
async fn allow(app: &axum::Router, cookie: &str, csrf: &str, role: &str, operation: &str) {
    let (status,body)=request(app,cookie,csrf,"PUT",&format!("/api/console/settings/roles/{role}/console-policy"),json!({"groups":[{"kind":"settings_feature","group_id":"system.extension-center","enabled":true,"strategy":"custom","operations":[operation]}]})).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
}
async fn acknowledge(runtime: &RuntimeFixture, record: &LifecycleOutboxRecord, worker: Uuid) {
    runtime.delivery.deliver(record).await.unwrap();
    runtime
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
fn resume(record: &LifecycleOutboxRecord) -> Value {
    json!({"event_id":record.event_id,"subscriber_id":record.subscriber_id,"expected":{"graph_fingerprint":record.graph_fingerprint,"handler_id":record.handler_id,"handler_version":record.handler_version}})
}

#[tokio::test]
async fn root_2007_ac_009_pause_revoke_retire() {
    let worker_fixture = std::path::PathBuf::from(
        std::env::var_os("MANAGED_EVENT_WORKER_FIXTURE")
            .expect("prebuild real managed_event_worker"),
    );
    assert!(worker_fixture.is_absolute() && worker_fixture.is_file());
    let (initial, database_url) = test_api_state_with_database_url().await;
    // The commit barrier and authority operations intentionally need independent real connections.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect(&database_url)
        .await
        .unwrap();
    let mut owned = (*initial).clone();
    owned.store = storage_durable_postgres::PgControlPlaneStore::new(pool.clone());
    let runtime = RuntimeFixture::new(&owned);
    owned.store = runtime.store.clone();
    owned.provider_runtime = runtime.services.clone();
    let state = Arc::new(owned);
    let app = crate::app_with_state(state.clone());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let actor_id: Uuid = sqlx::query_scalar("select id from users where account='root'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let actor = AuthRepository::load_actor_context_for_user(&runtime.store, actor_id)
        .await
        .unwrap();
    let workspace = actor.current_workspace_id;
    let management = PluginManagementService::new(
        runtime.store.clone(),
        crate::provider_runtime::ApiProviderRuntime::new(runtime.services.clone()),
        state.official_plugin_source.clone(),
        &state.provider_install_root,
    )
    .with_node_id(&state.api_node_id);
    let authority = PluginContributionAuthorityService::new(
        runtime.store.clone(),
        HostContributionGrantPolicy::root_composition(),
    )
    .with_node_id(&state.api_node_id);
    let mut a_manifest = manifest("a");
    a_manifest["managed"]["execution_bindings"][0]["handler"] = "fixture_barrier".into();
    let a = management
        .install_uploaded_plugin(InstallUploadedPluginCommand {
            actor_user_id: actor_id,
            file_name: "managed-governance-a.1flowbasepkg".into(),
            package_bytes: package(&a_manifest),
        })
        .await
        .unwrap()
        .installation
        .id;
    management
        .assign_plugin(AssignPluginCommand {
            actor_user_id: actor_id,
            installation_id: a,
        })
        .await
        .unwrap();
    for permission in ["event.subscribe", "event.publish"] {
        authority
            .grant(&actor, a, grant("a", permission))
            .await
            .unwrap();
    }
    management
        .enable_plugin(EnablePluginCommand {
            actor_user_id: actor_id,
            installation_id: a,
        })
        .await
        .unwrap();
    let artifact = runtime
        .store
        .get_artifact_instance(&state.api_node_id, a)
        .await
        .unwrap()
        .unwrap();
    let worker = std::path::PathBuf::from(artifact.local_path.unwrap()).join("bin/worker.py");
    let base = format!("/api/console/settings/extension-center/installed/{a}");
    let view = format!("{base}/managed-execution");
    let resume_path = format!("{base}/lifecycle-deliveries/resume");
    let retire_path = format!("{base}/managed-executions/retire");
    let (_, initial_view) = request(&app, &cookie, &csrf, "GET", &view, Value::Null).await;
    assert_eq!(initial_view["data"]["installation_id"], a.to_string());
    assert_eq!(initial_view["data"]["workspace_id"], workspace.to_string());
    let (status, inventory) = request(
        &app,
        &cookie,
        &csrf,
        "GET",
        "/api/console/settings/extension-center/installed",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let installed = inventory["data"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"] == a.to_string())
        .unwrap();
    assert_eq!(installed["contract_version"], "1flowbase.extension-bus/v1");
    assert!(installed["installed_versions"]
        .as_array()
        .unwrap()
        .iter()
        .all(|version| version["deletable"] == false
            && version["delete_reasons"]
                .as_array()
                .unwrap()
                .contains(&json!("managed_execution_references_or_backlog"))));
    let target_g1 = initial_view["data"]["executions"][0]["target"].clone();
    assert!(
        managed_handler_installation(target_g1["handler_version"].as_str().unwrap()) == Some(a)
    );
    let registry = state
        .extension_boot_snapshot
        .as_ref()
        .unwrap()
        .interface_registry()
        .unwrap()
        .snapshot();
    for (operation, method, path) in [
        ("managed_execution.view", "GET", "managed-execution"),
        (
            "lifecycle_deliveries.resume",
            "POST",
            "lifecycle-deliveries/resume",
        ),
        (
            "managed_executions.retire",
            "POST",
            "managed-executions/retire",
        ),
    ] {
        let id = format!("extension_center.{operation}");
        let plan = registry
            .plan_for_interface(&interface_runtime::InterfaceId::new(id.clone()).unwrap())
            .unwrap();
        assert_eq!(
            plan.binding().projection().http_route().unwrap().path(),
            format!("/api/console/settings/extension-center/installed/:installation_id/{path}")
        );
        assert_eq!(
            plan.binding().projection().http_route().unwrap().method(),
            method
        );
        assert_eq!(
            plan.definition().handler_reference().as_str(),
            format!("{id}.handler")
        );
        // Real session gate; a syntactically valid body reaches authentication.
        let body = if method == "GET" {
            Value::Null
        } else if operation.ends_with("resume") {
            json!({"event_id":Uuid::now_v7(),"subscriber_id":"none","expected":target_g1})
        } else {
            target_g1.clone()
        };
        assert_eq!(
            request(&app, "", "", method, &format!("{base}/{path}"), body)
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        request(&app, &cookie, "", "POST", &retire_path, target_g1.clone())
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            &cookie,
            &csrf,
            "POST",
            &retire_path,
            target_g1.clone()
        )
        .await
        .0,
        StatusCode::CONFLICT,
        "current graph cannot retire"
    );
    let role = "root_2007_governance_only";
    create_role(&app, &cookie, &csrf, role).await;
    replace_role_permissions(&app, &cookie, &csrf, role, &["plugin_config.configure.all"]).await;
    let member = create_member(
        &app,
        &cookie,
        &csrf,
        "root-2007-governance-member",
        "temp-pass",
    )
    .await;
    replace_member_roles(&app, &cookie, &csrf, &member, &[role]).await;
    let (member_cookie, member_csrf) =
        login_and_capture_cookie(&app, "root-2007-governance-member", "temp-pass").await;
    for (method, path, body) in [
        ("GET", view.clone(), Value::Null),
        (
            "POST",
            resume_path.clone(),
            json!({"event_id":Uuid::now_v7(),"subscriber_id":"none","expected":target_g1}),
        ),
        ("POST", retire_path.clone(), target_g1.clone()),
    ] {
        assert_eq!(
            request(&app, &member_cookie, &member_csrf, method, &path, body)
                .await
                .0,
            StatusCode::FORBIDDEN,
            "legacy configure does not grant governance"
        );
    }
    allow(
        &app,
        &cookie,
        &csrf,
        role,
        "extension_center.managed_execution.view",
    )
    .await;
    let (status, member_view) = request(
        &app,
        &member_cookie,
        &member_csrf,
        "GET",
        &view,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(member_view["data"], initial_view["data"]);
    assert_eq!(
        request(
            &app,
            &member_cookie,
            &member_csrf,
            "POST",
            &retire_path,
            target_g1.clone()
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );

    // G1 real worker remains in flight while B activation publishes G2.
    create(&runtime.store, actor_id, workspace).await;
    let claimant = Uuid::now_v7();
    let records = runtime
        .store
        .claim_lifecycle_facts(claimant, 32, time::Duration::minutes(5))
        .await
        .unwrap();
    let a_record = records
        .iter()
        .find(|r| managed_handler_installation(&r.handler_version) == Some(a))
        .unwrap()
        .clone();
    let running = {
        let delivery = runtime.delivery.clone();
        let record = a_record.clone();
        tokio::spawn(async move { delivery.deliver(&record).await })
    };
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while !worker.with_extension("started").is_file() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let b = management
        .install_uploaded_plugin(InstallUploadedPluginCommand {
            actor_user_id: actor_id,
            file_name: "managed-governance-b.1flowbasepkg".into(),
            package_bytes: package(&manifest("b")),
        })
        .await
        .unwrap()
        .installation
        .id;
    management
        .assign_plugin(AssignPluginCommand {
            actor_user_id: actor_id,
            installation_id: b,
        })
        .await
        .unwrap();
    authority
        .grant(&actor, b, grant("b", "event.subscribe"))
        .await
        .unwrap();
    management
        .enable_plugin(EnablePluginCommand {
            actor_user_id: actor_id,
            installation_id: b,
        })
        .await
        .unwrap();
    assert_eq!(
        request(
            &app,
            &cookie,
            &csrf,
            "POST",
            &retire_path,
            target_g1.clone()
        )
        .await
        .0,
        StatusCode::CONFLICT,
        "real in-flight invocation keeps G1"
    );
    std::fs::write(worker.with_extension("release"), "release").unwrap();
    running.await.unwrap().unwrap();
    runtime
        .store
        .mark_lifecycle_fact_delivered(
            a_record.event_id,
            &a_record.subscriber_id,
            claimant,
            a_record.claim_id.unwrap(),
        )
        .await
        .unwrap();
    for record in records
        .iter()
        .filter(|r| r.subscriber_id != a_record.subscriber_id)
    {
        acknowledge(&runtime, record, claimant).await;
    }

    // A deferred PG commit blocks after real Outbox INSERT while its host-private lease remains.
    let lock_key = (Uuid::now_v7().as_u128() as u64 & 0x7fff_ffff_ffff_ffff) as i64;
    let mut blocker = pool.acquire().await.unwrap();
    sqlx::query("select pg_advisory_lock($1)")
        .bind(lock_key)
        .execute(&mut *blocker)
        .await
        .unwrap();
    sqlx::query(&format!("create function root_2007_publication_barrier() returns trigger language plpgsql as $$ begin perform pg_advisory_xact_lock({lock_key}); return NEW; end $$")).execute(&pool).await.unwrap();
    sqlx::query("create constraint trigger root_2007_publication_barrier after insert on lifecycle_outbox deferrable initially deferred for each row execute function root_2007_publication_barrier()").execute(&pool).await.unwrap();
    let committing = {
        let store = runtime.store.clone();
        tokio::spawn(async move { create(&store, actor_id, workspace).await })
    };
    tokio::time::timeout(std::time::Duration::from_secs(3),async { loop { let waiting:bool=sqlx::query_scalar("select exists(select 1 from pg_locks where locktype='advisory' and not granted and objid=$1::bigint::oid)").bind(lock_key & 0xffff_ffff).fetch_one(&pool).await.unwrap();if waiting{break;}tokio::task::yield_now().await;} }).await.unwrap();
    committing.abort();
    assert!(
        committing.await.unwrap_err().is_cancelled(),
        "dropping the Create waiter must not drop its transaction publication lease"
    );
    let (_, g2_view) = request(&app, &cookie, &csrf, "GET", &view, Value::Null).await;
    assert!(g2_view["data"]["executions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|execution| execution["current"] == true
            && execution["frozen_reference_count"].as_u64().unwrap() > 0));
    let target_g2 = g2_view["data"]["executions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["current"] == true)
        .unwrap()["target"]
        .clone();
    management
        .disable_plugin(DisablePluginCommand {
            actor_user_id: actor_id,
            installation_id: b,
        })
        .await
        .unwrap();
    assert_eq!(
        request(
            &app,
            &cookie,
            &csrf,
            "POST",
            &retire_path,
            target_g2.clone()
        )
        .await
        .0,
        StatusCode::CONFLICT,
        "uncommitted future publication keeps G2"
    );
    sqlx::query("select pg_advisory_unlock($1)")
        .bind(lock_key)
        .execute(&mut *blocker)
        .await
        .unwrap();
    drop(blocker);
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            if deliveries(&runtime, a, workspace).await.len() == 2 {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    sqlx::query("drop trigger root_2007_publication_barrier on lifecycle_outbox")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("drop function root_2007_publication_barrier()")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        request(
            &app,
            &cookie,
            &csrf,
            "POST",
            &retire_path,
            target_g2.clone()
        )
        .await
        .0,
        StatusCode::CONFLICT,
        "committed durable backlog replaces publication lease"
    );
    let grants = authority.query(&actor, a).await.unwrap();
    let subscribed = grants
        .authorizations
        .iter()
        .find(|g| g.permission == "event.subscribe")
        .unwrap();
    authority
        .revoke(
            &actor,
            a,
            RevokeContributionPermission {
                authorization_id: subscribed.id,
                expected_revision: grants.revision,
            },
        )
        .await
        .unwrap();
    let pending = deliveries(&runtime, a, workspace)
        .await
        .into_iter()
        .find(|r| r.status != LifecycleOutboxStatus::Delivered)
        .unwrap();
    assert_eq!(pending.status, LifecycleOutboxStatus::Paused);
    assert_eq!(
        pending.pause_reason,
        Some(LifecycleDeliveryPauseReason::AuthorityRevoked)
    );
    let (status, paused_view) = request(&app, &cookie, &csrf, "GET", &view, Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert!(paused_view["data"]["deliveries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["event_id"] == pending.event_id.to_string()
            && r["status"] == "paused"
            && r["pause_reason"] == "authority_revoked"));
    assert_ne!(
        request(&app, &cookie, &csrf, "POST", &resume_path, resume(&pending))
            .await
            .0,
        StatusCode::OK,
        "revoked authority cannot resume"
    );
    authority
        .grant(&actor, a, grant("a", "event.subscribe"))
        .await
        .unwrap();
    let restarted = RuntimeFixture::new(&state);
    restarted.composition.rebuild_installation(a).await.unwrap();
    let restarted_service =
        ManagedExecutionService::new(runtime.store.clone(), restarted.composition.governance());
    let exact: ResumeManagedLifecycleDelivery = serde_json::from_value(resume(&pending)).unwrap();
    assert!(
        restarted_service.resume(&actor, a, exact).await.is_err(),
        "new epoch cannot impersonate old frozen target"
    );
    let mut old_epoch = resume(&pending);
    let (_, suffix) = pending.handler_version.split_once(':').unwrap();
    old_epoch["expected"]["handler_version"] = format!("{}:{suffix}", Uuid::now_v7()).into();
    assert_eq!(
        request(&app, &cookie, &csrf, "POST", &resume_path, old_epoch)
            .await
            .0,
        StatusCode::CONFLICT
    );
    let mut cross_workspace = resume(&pending);
    cross_workspace["workspace_id"] = Uuid::now_v7().to_string().into();
    assert_eq!(
        request(&app, &cookie, &csrf, "POST", &resume_path, cross_workspace)
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let saved = worker.with_extension("saved");
    std::fs::rename(&worker, &saved).unwrap();
    let missing_artifact = request(&app, &cookie, &csrf, "POST", &resume_path, resume(&pending))
        .await
        .0;
    std::fs::rename(&saved, &worker).unwrap();
    assert_ne!(
        missing_artifact,
        StatusCode::OK,
        "resume requires the original executable"
    );
    allow(
        &app,
        &cookie,
        &csrf,
        role,
        "extension_center.lifecycle_deliveries.resume",
    )
    .await;
    assert_eq!(
        request(
            &app,
            &member_cookie,
            &member_csrf,
            "GET",
            &view,
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let history_ids = insert_completed_history(&runtime, &pending).await;
    let (_, bounded_view) = request(&app, &cookie, &csrf, "GET", &view, Value::Null).await;
    assert_eq!(bounded_view["data"]["deliveries_truncated"], true);
    assert_eq!(
        bounded_view["data"]["deliveries"].as_array().unwrap().len(),
        MANAGED_DELIVERY_PAGE_LIMIT
    );
    assert!(!bounded_view["data"]["deliveries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["event_id"] == pending.event_id.to_string()));
    assert_eq!(
        request(
            &app,
            &cookie,
            &csrf,
            "POST",
            &retire_path,
            target_g2.clone()
        )
        .await
        .0,
        StatusCode::CONFLICT,
        "pending target beyond the displayed page still prevents retirement"
    );
    assert!(
        runtime
            .store
            .managed_installation_has_backlog(a, None, None)
            .await
            .unwrap(),
        "completed history cannot hide a late pending target from the deletion guard"
    );
    assert!(
        runtime
            .store
            .managed_lifecycle_delivery(
                a,
                Uuid::now_v7(),
                &serde_json::from_value(resume(&pending)).unwrap()
            )
            .await
            .unwrap()
            .is_none(),
        "exact lookup remains workspace-scoped"
    );
    assert_eq!(
        request(
            &app,
            &member_cookie,
            &member_csrf,
            "POST",
            &resume_path,
            resume(&pending)
        )
        .await
        .0,
        StatusCode::OK,
        "only explicit exact regrant can resume"
    );
    sqlx::query("delete from lifecycle_outbox where event_id=any($1)")
        .bind(&history_ids)
        .execute(&pool)
        .await
        .unwrap();

    let claimant = Uuid::now_v7();
    for record in runtime
        .store
        .claim_lifecycle_facts(claimant, 32, time::Duration::minutes(5))
        .await
        .unwrap()
    {
        acknowledge(&runtime, &record, claimant).await;
    }
    assert!(deliveries(&runtime, a, workspace)
        .await
        .iter()
        .all(|r| r.status == LifecycleOutboxStatus::Delivered));

    // G2 can retire after its deliveries finish, even though G3 still shares A's handle.
    assert_eq!(
        request(
            &app,
            &cookie,
            &csrf,
            "POST",
            &retire_path,
            target_g2.clone()
        )
        .await
        .0,
        StatusCode::OK
    );
    assert!(
        management
            .enable_plugin(EnablePluginCommand {
                actor_user_id: actor_id,
                installation_id: b
            })
            .await
            .is_err(),
        "a new B mount cannot resurrect retired G2/A's exact target"
    );
    management
        .disable_plugin(DisablePluginCommand {
            actor_user_id: actor_id,
            installation_id: b,
        })
        .await
        .unwrap();

    // Disable pauses existing backlog; reenabling never implicitly resumes it.
    create(&runtime.store, actor_id, workspace).await;
    management
        .disable_plugin(DisablePluginCommand {
            actor_user_id: actor_id,
            installation_id: a,
        })
        .await
        .unwrap();
    let disabled = deliveries(&runtime, a, workspace)
        .await
        .into_iter()
        .find(|record| record.status != LifecycleOutboxStatus::Delivered)
        .unwrap();
    assert_eq!(disabled.status, LifecycleOutboxStatus::Paused);
    assert_eq!(
        disabled.pause_reason,
        Some(LifecycleDeliveryPauseReason::InstallationInactive)
    );
    assert_ne!(
        request(
            &app,
            &cookie,
            &csrf,
            "POST",
            &resume_path,
            resume(&disabled)
        )
        .await
        .0,
        StatusCode::OK
    );
    management
        .enable_plugin(EnablePluginCommand {
            actor_user_id: actor_id,
            installation_id: a,
        })
        .await
        .unwrap();
    assert_eq!(
        deliveries(&runtime, a, workspace)
            .await
            .into_iter()
            .find(|record| record.event_id == disabled.event_id)
            .unwrap()
            .status,
        LifecycleOutboxStatus::Paused
    );
    restarted.composition.rebuild_installation(a).await.unwrap();
    let restored = restarted.composition.snapshot(workspace).await.unwrap();
    assert_eq!(
        restored.graph.fingerprint().as_str(),
        disabled.graph_fingerprint
    );
    assert!(
        restarted_service
            .resume(
                &actor,
                a,
                serde_json::from_value(resume(&disabled)).unwrap()
            )
            .await
            .is_err(),
        "same graph from a different epoch must still reject the frozen handler"
    );
    restarted.host.stop().await.unwrap();
    assert_eq!(
        request(
            &app,
            &cookie,
            &csrf,
            "POST",
            &resume_path,
            resume(&disabled)
        )
        .await
        .0,
        StatusCode::OK
    );
    let claimant = Uuid::now_v7();
    for record in runtime
        .store
        .claim_lifecycle_facts(claimant, 32, time::Duration::minutes(5))
        .await
        .unwrap()
    {
        acknowledge(&runtime, &record, claimant).await;
    }

    // Disable preserves references and does not silently discard delivery rows.
    management
        .disable_plugin(DisablePluginCommand {
            actor_user_id: actor_id,
            installation_id: a,
        })
        .await
        .unwrap();
    assert!(control_plane_contracts::ports::ManagedArtifactRemovalGuard::guard_managed_artifact_removal(runtime.composition.as_ref(),&[a]).await.is_err());
    let service_without_guard =
        ExtensionInstallationService::new(runtime.store.clone(), &state.provider_install_root);
    assert!(service_without_guard
        .delete_local_installation(&state.api_node_id, a)
        .await
        .is_err());
    assert_ne!(
        request(&app, &cookie, &csrf, "DELETE", &base, Value::Null)
            .await
            .0,
        StatusCode::OK,
        "family delete cannot bypass retained reference guard"
    );
    allow(
        &app,
        &cookie,
        &csrf,
        role,
        "extension_center.managed_executions.retire",
    )
    .await;
    let (_, retained) = request(&app, &cookie, &csrf, "GET", &view, Value::Null).await;
    for execution in retained["data"]["executions"].as_array().unwrap() {
        assert_eq!(execution["current"], false);
        assert_eq!(
            request(
                &app,
                &member_cookie,
                &member_csrf,
                "POST",
                &retire_path,
                execution["target"].clone()
            )
            .await
            .0,
            StatusCode::OK
        );
    }
    let (_, retired) = request(&app, &cookie, &csrf, "GET", &view, Value::Null).await;
    assert_eq!(retired["data"]["executions"], json!([]));
    assert_eq!(
        retired["data"]["deliveries"].as_array().unwrap().len(),
        3,
        "retirement never deletes backlog history"
    );
    drop(control_plane_contracts::ports::ManagedArtifactRemovalGuard::guard_managed_artifact_removal(runtime.composition.as_ref(),&[a]).await.unwrap());
    assert!(
        runtime
            .composition
            .event_snapshot_for_graph(&pending)
            .await
            .unwrap()
            .is_none(),
        "retired graph references are released"
    );
    // A real old-format imported target has no provable installation owner. Keep it unchanged,
    // expose the uncertainty, and prevent deletion rather than guessing its identity.
    let old_fact: extension_contracts::AfterCommitFact<ModelDefinitionCommittedFact> =
        serde_json::from_slice(&a_record.canonical_payload).unwrap();
    let event_id = Uuid::now_v7();
    let transaction_id = Uuid::now_v7();
    let occurred_at = time::OffsetDateTime::now_utc();
    let legacy_fact = extension_contracts::AfterCommitFact::new(
        extension_contracts::LifecycleFactId::new(event_id.to_string()).unwrap(),
        extension_contracts::LifecycleTransactionId::new(transaction_id.to_string()).unwrap(),
        (occurred_at.unix_timestamp_nanos() / 1_000_000) as i64,
        old_fact.payload().clone(),
    );
    let legacy_version = a_record
        .handler_version
        .rsplit_once(':')
        .unwrap()
        .0
        .to_string();
    assert_eq!(managed_handler_installation(&legacy_version), None);
    runtime
        .store
        .record_lifecycle_fact(&RecordLifecycleFactInput {
            event_id,
            transaction_id,
            contract_id: a_record.contract_id.clone(),
            contract_version: a_record.contract_version.clone(),
            canonical_payload: serde_json::to_vec(&legacy_fact).unwrap(),
            occurred_at,
            publication: LifecyclePublicationPlan {
                graph_fingerprint: a_record.graph_fingerprint.clone(),
                subscribers: vec![LifecycleSubscriberTarget {
                    subscriber_id: a_record.subscriber_id.clone(),
                    handler_id: a_record.handler_id.clone(),
                    handler_version: legacy_version.clone(),
                }],
            },
        })
        .await
        .unwrap();
    let claimant = Uuid::now_v7();
    let legacy = runtime
        .store
        .claim_lifecycle_facts(claimant, 32, time::Duration::minutes(5))
        .await
        .unwrap()
        .into_iter()
        .find(|record| record.event_id == event_id)
        .unwrap();
    runtime
        .store
        .pause_lifecycle_fact(
            event_id,
            &legacy.subscriber_id,
            claimant,
            legacy.claim_id.unwrap(),
            LifecycleDeliveryPauseReason::FrozenHandlerUnavailable,
        )
        .await
        .unwrap();
    let (_, legacy_view) = request(&app, &cookie, &csrf, "GET", &view, Value::Null).await;
    assert!(legacy_view["data"]["deliveries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|record| record["event_id"] == event_id.to_string()
            && record["ownership"] == "unknown_legacy"
            && record["target"]["handler_version"] == legacy_version));
    assert_eq!(
        request(&app, &cookie, &csrf, "POST", &resume_path, resume(&legacy))
            .await
            .0,
        StatusCode::CONFLICT
    );
    let _history_ids = insert_completed_history(&runtime, &legacy).await;
    let (_, bounded_legacy) = request(&app, &cookie, &csrf, "GET", &view, Value::Null).await;
    assert_eq!(bounded_legacy["data"]["deliveries_truncated"], true);
    assert!(!bounded_legacy["data"]["deliveries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["event_id"] == event_id.to_string()));
    assert!(control_plane_contracts::ports::ManagedArtifactRemovalGuard::guard_managed_artifact_removal(runtime.composition.as_ref(),&[a]).await.is_err());
    assert_ne!(
        request(&app, &cookie, &csrf, "DELETE", &base, Value::Null)
            .await
            .0,
        StatusCode::OK,
        "legacy durable backlog still blocks family deletion"
    );
    std::fs::remove_file(worker.with_extension("started")).unwrap();
    std::fs::remove_file(worker.with_extension("release")).unwrap();
    runtime.host.stop().await.unwrap();
}
