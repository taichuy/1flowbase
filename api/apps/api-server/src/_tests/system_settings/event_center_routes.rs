use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles,
    test_api_state_with_database_url, test_config,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use control_plane::event_center::{
    EventCenterRuntime, EventCenterRuntimePolicy, EventRuntimeSubscription,
};
use control_plane_contracts::ports::{EventBusLimits, EventCenterRepository};
use domain::event_center::EventTriggerPhase;
use serde_json::{json, Value};
use std::sync::Arc;
use storage_ephemeral::MemoryEventBus;
use tower::ServiceExt;
use uuid::Uuid;

async fn request(
    app: &Router,
    method: &str,
    uri: &str,
    cookie: &str,
    csrf: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(
                    body.map(|value| Body::from(value.to_string()))
                        .unwrap_or_else(Body::empty),
                )
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn definition() -> Value {
    json!({"contract_id":"custom.invoice","contract_version":"1","name":"Invoice","description":"Route fixture","enabled":true,
        "payload_schema":{"type":"object","properties":{"amount":{"type":"number"}},"required":["amount"],"additionalProperties":false}})
}

#[tokio::test]
async fn event_center_routes_keep_definition_configuration_separate_from_publication() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    assert_eq!(
        request(&app, "GET", "/api/console/events", "", "", None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let mut invalid = definition();
    invalid["contract_id"] = json!("1flowbase.owned");
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/console/events",
            &cookie,
            &csrf,
            Some(invalid)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (status, created) = request(
        &app,
        "POST",
        "/api/console/events",
        &cookie,
        &csrf,
        Some(definition()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert!(created["data"]["created_at"].is_string());
    let id = created["data"]["id"].as_str().unwrap();
    let (status, catalog) = request(&app, "GET", "/api/console/events", &cookie, "", None).await;
    assert_eq!(status, StatusCode::OK);
    let row = catalog["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == id)
        .unwrap();
    assert_eq!(
        row["supported_phases"],
        json!(["subscribed", "published", "consumed"])
    );
    let (_, runtime) = request(
        &app,
        "GET",
        "/api/console/events/runtime",
        &cookie,
        "",
        None,
    )
    .await;
    assert_eq!(runtime["data"]["activity"], json!([]));
    assert_eq!(runtime["data"]["queues"], json!([]));
    assert_eq!(runtime["data"]["workflow_subscriptions"], json!([]));
    let mut update = definition();
    let object = update.as_object_mut().unwrap();
    object.remove("contract_id");
    object.remove("contract_version");
    object.insert("enabled".into(), json!(false));
    let (status, saved) = request(
        &app,
        "PUT",
        &format!("/api/console/events/{id}"),
        &cookie,
        &csrf,
        Some(update),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["data"]["enabled"], false);
    let missing = Uuid::now_v7();
    assert_eq!(
        request(
            &app,
            "PUT",
            &format!("/api/console/events/{missing}"),
            &cookie,
            &csrf,
            Some(json!({"name":"Missing","description":"","enabled":true,"payload_schema":true}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn event_center_routes_use_independent_operations_and_fail_closed_when_group_disabled() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state, &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let role = "event_reader";
    create_role(&app, &cookie, &csrf, role).await;
    let member = create_member(&app, &cookie, &csrf, "event-reader", "temp-pass").await;
    replace_member_roles(&app, &cookie, &csrf, &member, &[role]).await;
    let (member_cookie, member_csrf) =
        login_and_capture_cookie(&app, "event-reader", "temp-pass").await;
    assert_eq!(
        request(&app, "GET", "/api/console/events", &member_cookie, "", None)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let mut group = json!({"kind":"settings_feature","group_id":"system.events","enabled":true,"strategy":"custom",
        "operations":[{"kind":"simple","operation_id":"events.catalog.list","enabled":true}]});
    let policy_path = format!("/api/console/settings/roles/{role}/console-policy");
    let (status, body) = request(
        &app,
        "PUT",
        &policy_path,
        &cookie,
        &csrf,
        Some(json!({"groups":[group.clone()]})),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert_eq!(
        request(&app, "GET", "/api/console/events", &member_cookie, "", None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        request(
            &app,
            "GET",
            "/api/console/events/runtime",
            &member_cookie,
            "",
            None
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/console/events",
            &member_cookie,
            &member_csrf,
            Some(definition())
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    group["enabled"] = json!(false);
    assert_eq!(
        request(
            &app,
            "PUT",
            &policy_path,
            &cookie,
            &csrf,
            Some(json!({"groups":[group]}))
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        request(&app, "GET", "/api/console/events", &member_cookie, "", None)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn event_center_runtime_route_observes_without_consuming_or_exposing_payload() {
    let (mut state, _) = test_api_state_with_database_url().await;
    Arc::get_mut(&mut state).unwrap().event_center = EventCenterRuntime::new(
        Arc::new(MemoryEventBus::new()),
        Some(EventCenterRuntimePolicy {
            bus_limits: EventBusLimits {
                max_items: 16,
                max_json_bytes: 64000,
                max_waiters: 16,
                max_topics: 16,
                max_consumers: 16,
                max_topic_name_bytes: 16000,
            },
            observation_max_bytes: 16000,
            observation_ttl_ms: 60000,
        }),
    )
    .unwrap();
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let (status, created) = request(
        &app,
        "POST",
        "/api/console/events",
        &cookie,
        &csrf,
        Some(definition()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let definition = state
        .store
        .get_event_definition(
            state.bootstrap_workspace_id,
            Uuid::parse_str(created["data"]["id"].as_str().unwrap()).unwrap(),
        )
        .await
        .unwrap()
        .unwrap();
    let subscription = state
        .event_center
        .subscribe(
            EventRuntimeSubscription {
                subscription_id: Uuid::now_v7(),
                workspace_id: definition.workspace_id,
                contract_id: definition.contract_id.clone(),
                contract_version: definition.contract_version.clone(),
                trigger_phase: EventTriggerPhase::Published,
                consumer_id: Uuid::now_v7(),
            },
            false,
        )
        .unwrap();
    state
        .event_center
        .publish(&definition, json!({"amount":7}))
        .unwrap();
    for _ in 0..2 {
        let (status, snapshot) = request(
            &app,
            "GET",
            "/api/console/events/runtime",
            &cookie,
            "",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{snapshot}");
        assert_eq!(snapshot["data"]["queues"][0]["buffered"], 1);
        assert_eq!(snapshot["data"]["queues"][0]["consumed"], 0);
        assert!(!snapshot.to_string().contains("amount"));
    }
    let delivery = subscription.recv().await.unwrap().unwrap();
    assert_eq!(
        state.event_center.snapshot(definition.workspace_id).queues[0].consumed,
        1
    );
    drop(delivery);
    subscription.close();
}
