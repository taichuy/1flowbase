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
        .managed_lifecycle_delivery_page(installation, workspace, None)
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

pub(super) async fn request(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    method: &str,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    static NEXT_REQUEST: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);
    let request_id = NEXT_REQUEST.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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
                .unwrap_or_else(|error| panic!("request #{request_id} {method} {path}: {error}")),
        )
        .await
        .unwrap_or_else(|error| panic!("request #{request_id} {method} {path}: {error}"));
    let status = response.status();
    let content_type = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .map(|value| {
            value
                .to_str()
                .unwrap_or("<invalid content-type>")
                .to_owned()
        });
    let context =
        format!("request #{request_id} {method} {path} -> {status}; content-type={content_type:?}");
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap_or_else(|error| panic!("{context}; raw body unavailable: {error}"));
    let prefix = &bytes[..bytes.len().min(2048)];
    let preview = match std::str::from_utf8(prefix) {
        Ok(text) => format!("{text:?}"),
        Err(_) => format!("hex:{prefix:02x?}"),
    };
    // Rust captures these lines for any later assertion failure, including status-only checks.
    // Never log session/CSRF request headers; response previews are explicitly byte-bounded.
    eprintln!(
        "{context}; body_bytes={}; raw_body_prefix={preview}; truncated={}",
        bytes.len(),
        bytes.len() > prefix.len()
    );
    let media_type = content_type
        .as_deref()
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let is_json = media_type
        .split_once('/')
        .is_some_and(|(_, subtype)| subtype == "json" || subtype.ends_with("+json"));
    let body = if is_json {
        // A JSON media type is a contract: malformed or empty JSON must fail here with evidence.
        serde_json::from_slice(&bytes).unwrap_or_else(|error| {
            panic!("{context}; invalid JSON: {error}; raw_body_prefix={preview}")
        })
    } else if bytes.is_empty() {
        Value::Null
    } else {
        // Preserve non-JSON bodies for existing status assertions and any downstream diagnostics.
        // JSON-looking text with a non-JSON media type remains text; it is not silently parsed.
        Value::String(match String::from_utf8(bytes.to_vec()) {
            Ok(text) => text,
            Err(error) => format!("hex:{:02x?}", error.as_bytes()),
        })
    };
    (status, body)
}
async fn allow(app: &axum::Router, cookie: &str, csrf: &str, role: &str, operation: &str) {
    let (status,body)=request(app,cookie,csrf,"PUT",&format!("/api/console/settings/roles/{role}/console-policy"),json!({"groups":[{"kind":"settings_feature","group_id":"system.extension-center","enabled":true,"strategy":"custom","operations":[{"kind":"simple","operation_id":operation,"enabled":true}]}]})).await;
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
async fn root_2325_http_history_cursor_and_authorization() {
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
    .with_node_id(&state.api_node_id)
    .with_managed_activation();

    let installation = management
        .install_uploaded_plugin(InstallUploadedPluginCommand {
            actor_user_id: actor_id,
            file_name: "root-2325.1flowbasepkg".into(),
            package_bytes: package(&manifest("a")),
        })
        .await
        .unwrap()
        .installation
        .id;
    management
        .assign_plugin(AssignPluginCommand {
            actor_user_id: actor_id,
            installation_id: installation,
        })
        .await
        .unwrap();
    let version = format!(
        "{}:sha256:{}:sha256:{}:1:{installation}",
        Uuid::now_v7(),
        "a".repeat(64),
        "b".repeat(64)
    );
    let subscriber = format!("managed.{workspace}.history");
    sqlx::query("insert into plugin_contribution_authorization_revisions(installation_id,workspace_id) values($1,$2) on conflict do nothing").bind(installation).bind(workspace).execute(&pool).await.unwrap();
    let ids = (1..=600).map(Uuid::from_u128).collect::<Vec<_>>();
    sqlx::query("insert into lifecycle_outbox(event_id,transaction_id,contract_id,contract_version,canonical_payload,occurred_at,graph_fingerprint) select id,id,'history','v1',decode('ff','hex'),now(),'history' from unnest($1::uuid[]) id").bind(&ids).execute(&pool).await.unwrap();
    sqlx::query("insert into lifecycle_outbox_deliveries(event_id,subscriber_id,handler_id,handler_version) select id,$2,$2,$3 from unnest($1::uuid[]) id").bind(&ids).bind(&subscriber).bind(&version).execute(&pool).await.unwrap();
    let view = format!(
        "/api/console/settings/extension-center/installed/{installation}/managed-execution"
    );
    let (status, first) = request(&app, &cookie, &csrf, "GET", &view, Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["data"]["deliveries"].as_array().unwrap().len(), 256);
    let token = first["data"]["next_cursor"].as_str().unwrap();
    let continued = format!("{view}?cursor={token}");
    assert_eq!(
        request(&app, "", "", "GET", &continued, Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let role = "root_2325_cursor_denied";
    create_role(&app, &cookie, &csrf, role).await;
    let member = create_member(&app, &cookie, &csrf, "root-2325-member", "temp-pass").await;
    replace_member_roles(&app, &cookie, &csrf, &member, &[role]).await;
    let (member_cookie, member_csrf) =
        login_and_capture_cookie(&app, "root-2325-member", "temp-pass").await;
    assert_eq!(
        request(
            &app,
            &member_cookie,
            &member_csrf,
            "GET",
            &continued,
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let mut cursor = ManagedDeliveryCursor::decode(token, installation, workspace).unwrap();
    cursor.workspace_id = Uuid::now_v7();
    let cross_workspace = cursor.encode();
    cursor.workspace_id = workspace;
    cursor.installation_id = Uuid::now_v7();
    for invalid in ["invalid".to_owned(), cross_workspace, cursor.encode()] {
        let (status, body) = request(
            &app,
            &cookie,
            &csrf,
            "GET",
            &format!("{view}?cursor={invalid}"),
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }
    let mut observed = Vec::new();
    let mut page = first;
    let mut pages = 0;
    loop {
        pages += 1;
        assert!(pages <= 3);
        observed.extend(
            page["data"]["deliveries"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| Uuid::parse_str(row["event_id"].as_str().unwrap()).unwrap()),
        );
        let Some(next) = page["data"]["next_cursor"].as_str() else {
            assert_eq!(page["data"]["deliveries_truncated"], false);
            break;
        };
        assert_eq!(page["data"]["deliveries_truncated"], true);
        let (status, body) = request(
            &app,
            &cookie,
            &csrf,
            "GET",
            &format!("{view}?cursor={next}"),
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        page = body;
    }
    assert_eq!(observed, ids);
    assert_eq!(pages, 3);
}

#[path = "managed_removal_tests.rs"]
mod managed_removal_tests;

#[path = "managed_governance_tests/system_scope.rs"]
mod system_scope;

#[path = "managed_governance_tests/activation_permissions.rs"]
mod activation_permissions;
mod pause_revoke_retire;
