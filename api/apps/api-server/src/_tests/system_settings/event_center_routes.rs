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
    EventCenterRuntime, EventCenterRuntimePolicy, EventCenterSignalBody, EventRuntimeSubscription,
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

async fn event_state() -> Arc<crate::app_state::ApiState> {
    let (mut state, _) = test_api_state_with_database_url().await;
    let state_mut = Arc::get_mut(&mut state).unwrap();
    state_mut.event_center = EventCenterRuntime::new(
        Arc::new(MemoryEventBus::new()),
        Some(EventCenterRuntimePolicy {
            bus_limits: EventBusLimits {
                max_items: 32,
                max_json_bytes: 256_000,
                max_waiters: 16,
                max_topics: 32,
                max_consumers: 32,
                max_topic_name_bytes: 16_000,
            },
            observation_max_bytes: 32_000,
            observation_ttl_ms: 60_000,
        }),
    )
    .unwrap();
    // Bind only this fixture to the compiled serving catalog. General support remains
    // deliberately unavailable until a test opts into an event runtime.
    state_mut.event_sources = Arc::new(crate::app_state::EventSourceCatalog::for_test_support());
    state
}

fn interface_rule(interface_id: &str, phase: &str, results: &[&str]) -> Value {
    json!({"name":"Application interface observed","description":"Route fixture","enabled":true,
        "interface_event":{"interface_id":interface_id,"phase":phase,"results":results}})
}

fn definition() -> Value {
    interface_rule("applications.list", "before", &[])
}

#[tokio::test]
async fn event_center_mcp_rules_use_declared_dto_fields() {
    let state = event_state().await;
    let app = crate::app_with_state_and_config(state, &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let (status, catalog) = request(
        &app,
        "GET",
        "/api/console/mcp/interface-capabilities?bindable_only=true",
        &cookie,
        "",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{catalog}");
    let entries = catalog["data"].as_array().unwrap();
    let mut definition_id = String::new();
    for operation in ["events.definitions.create", "events.definitions.update"] {
        let entry = entries
            .iter()
            .find(|entry| entry["interface_id"] == operation)
            .unwrap();
        let descriptors = entry["parameter_descriptors"].as_array().unwrap();
        for name in ["name", "description", "enabled", "interface_event"] {
            assert!(
                descriptors.iter().any(|field| field["name"] == name),
                "{operation} must expose the actual DTO field {name}: {entry}"
            );
        }
        assert!(
            !descriptors.iter().any(|field| field["name"] == "body"),
            "an opaque body descriptor would create a second body object"
        );
        let binding =
            &entry["parameter_schema"]["properties"]["body"]["properties"]["interface_event"];
        let wire = binding.to_string();
        for field in [
            "operation_id",
            "interface_id",
            "phase",
            "before",
            "completion",
            "results",
            "completed",
            "failed",
            "rejected",
            "cancelled",
        ] {
            assert!(
                wire.contains(field),
                "typed API/legacy union must expose {field}: {binding}"
            );
        }
        let mappings: Vec<_> = descriptors
            .iter()
            .map(|field| {
                json!({
                    "interface_param":field["name"], "mcp_param":field["name"],
                    "required":field["required"]
                })
            })
            .collect();
        let mut arguments = definition();
        if operation.ends_with("update") {
            arguments["event_definition_id"] = json!(definition_id);
            arguments["enabled"] = json!(false);
        }
        let (status, result) = request(
            &app,
            "POST",
            "/api/console/mcp/debug/execute",
            &cookie,
            &csrf,
            Some(json!({
                "interface_id":operation,
                "debug_response_mode":"tool_result",
                "mcp_arguments":arguments,
                "input_mapping":{"mappings":mappings},
                "output_mapping":{}
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{operation}: {result}");
        if operation.ends_with("create") {
            definition_id = result["data"]["id"].as_str().unwrap().to_owned();
            assert_eq!(result["data"]["enabled"], true);
        } else {
            assert_eq!(result["data"]["id"], definition_id);
            assert_eq!(result["data"]["enabled"], false);
        }
        assert_eq!(
            result["data"]["interface_event"]["interface_id"],
            "applications.list"
        );
    }
}

#[tokio::test]
async fn event_center_routes_keep_definition_configuration_separate_from_publication() {
    let state = event_state().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    assert_eq!(
        request(&app, "GET", "/api/console/events", "", "", None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let mut invalid = definition();
    invalid["interface_event"]["interface_id"] = json!("applications.undeclared");
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
    assert!(created["data"]["contract_id"]
        .as_str()
        .unwrap()
        .starts_with("custom.interface."));
    assert_eq!(
        created["data"]["interface_event"]["interface_id"],
        "applications.list"
    );
    assert_eq!(created["data"]["interface_event"]["phase"], "before");
    assert!(created["data"].get("contract_version").is_none());
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/console/events",
            &cookie,
            "",
            Some(definition())
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    for field in ["contract_id", "payload_schema"] {
        let mut obsolete = definition();
        obsolete[field] = json!("free-form contract is no longer accepted");
        assert_eq!(
            request(
                &app,
                "POST",
                "/api/console/events",
                &cookie,
                &csrf,
                Some(obsolete)
            )
            .await
            .0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    let mut removed_version = definition();
    removed_version["contract_version"] = json!("1");
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/console/events",
            &cookie,
            &csrf,
            Some(removed_version)
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let id = created["data"]["id"].as_str().unwrap();
    let (status, catalog) = request(&app, "GET", "/api/console/events", &cookie, "", None).await;
    assert_eq!(status, StatusCode::OK);
    let row = catalog["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == id)
        .unwrap();
    assert_eq!(row["owner_id"], Value::Null);
    assert_eq!(row["enabled"], true);
    assert_eq!(row["scope"]["kind"], "workspace");
    assert!(row.get("contract_version").is_none());
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
    let views = runtime["data"]["source_views"].as_array().unwrap();
    assert_eq!(views.len(), 3);
    assert_eq!(views[0]["scope"]["kind"], "workspace");
    assert_eq!(views[1]["scope"]["kind"], "system");
    assert_eq!(views[2]["scope"]["kind"], "unbound");
    let mut update = definition();
    let object = update.as_object_mut().unwrap();
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
            Some(definition())
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn event_center_routes_use_independent_operations_and_fail_closed_when_group_disabled() {
    let state = event_state().await;
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
    group["operations"]
        .as_array_mut()
        .unwrap()
        .push(json!({"kind":"simple","operation_id":"events.runtime.get","enabled":true}));
    assert_eq!(
        request(
            &app,
            "PUT",
            &policy_path,
            &cookie,
            &csrf,
            Some(json!({"groups":[group.clone()]}))
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    let (status, scoped) = request(
        &app,
        "GET",
        "/api/console/events/runtime",
        &member_cookie,
        "",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let views = scoped["data"]["source_views"].as_array().unwrap();
    assert_eq!(
        views.len(),
        1,
        "operation permission never grants system/unbound visibility"
    );
    assert_eq!(views[0]["scope"]["kind"], "workspace");
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
    let state = event_state().await;
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
                trigger_phase: EventTriggerPhase::Published,
                consumer_id: Uuid::now_v7(),
            },
            false,
        )
        .unwrap();
    state
        .event_center
        .publish(&definition, json!({"interface_id":"applications.list","invocation_id":"runtime-observation-private-id","phase":"before","input":{"variant":"List"}}))
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
        assert!(!snapshot
            .to_string()
            .contains("runtime-observation-private-id"));
    }
    let delivery = subscription.recv().await.unwrap().unwrap();
    assert_eq!(
        state.event_center.snapshot(definition.workspace_id).queues[0].consumed,
        1
    );
    drop(delivery);
    subscription.close();
}

async fn create_rule(
    app: &Router,
    state: &Arc<crate::app_state::ApiState>,
    cookie: &str,
    csrf: &str,
    body: Value,
) -> domain::event_center::EventDefinitionRecord {
    let (status, created) =
        request(app, "POST", "/api/console/events", cookie, csrf, Some(body)).await;
    assert_eq!(status, StatusCode::OK, "{created}");
    state
        .store
        .get_event_definition(
            state.bootstrap_workspace_id,
            Uuid::parse_str(created["data"]["id"].as_str().unwrap()).unwrap(),
        )
        .await
        .unwrap()
        .unwrap()
}

fn subscribe_rule(
    state: &Arc<crate::app_state::ApiState>,
    definition: &domain::event_center::EventDefinitionRecord,
    workspace_id: Uuid,
) -> control_plane::event_center::EventCenterSubscription {
    state
        .event_center
        .subscribe(
            EventRuntimeSubscription {
                subscription_id: Uuid::now_v7(),
                workspace_id,
                contract_id: definition.contract_id.clone(),
                trigger_phase: EventTriggerPhase::Published,
                consumer_id: Uuid::now_v7(),
            },
            false,
        )
        .unwrap()
}

fn take_payload(subscription: &control_plane::event_center::EventCenterSubscription) -> Value {
    let delivery = subscription
        .try_recv()
        .unwrap()
        .expect("real invocation publishes a rule event");
    let EventCenterSignalBody::Published { payload } = &delivery.signal().body else {
        panic!("rule subscription must receive a published signal");
    };
    payload.clone()
}

#[tokio::test]
async fn interface_rules_publish_actual_call_values_once_and_keep_result_and_workspace_boundaries()
{
    let state = event_state().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let (status, application) = request(&app, "POST", "/api/console/applications", &cookie, &csrf,
        Some(json!({"application_type":"agent_flow","name":"Actual event app","description":"Actual event description"}))).await;
    assert_eq!(status, StatusCode::CREATED, "{application}");

    let before = create_rule(&app, &state, &cookie, &csrf, definition()).await;
    let completion = create_rule(
        &app,
        &state,
        &cookie,
        &csrf,
        interface_rule("applications.list", "completion", &["completed"]),
    )
    .await;
    let before_subscription = subscribe_rule(&state, &before, before.workspace_id);
    let completion_subscription = subscribe_rule(&state, &completion, completion.workspace_id);
    let foreign = subscribe_rule(&state, &completion, Uuid::now_v7());

    let (status, catalog) = request(&app, "GET", "/api/console/events", &cookie, "", None).await;
    assert_eq!(status, StatusCode::OK, "{catalog}");
    for rule in [&before, &completion] {
        let declared = catalog["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| {
                entry["origin"] == "core"
                    && entry["interface_event"]["interface_id"] == "applications.list"
                    && entry["interface_event"]["phase"]
                        == serde_json::to_value(rule.interface_event.as_ref().unwrap().phase())
                            .unwrap()
            })
            .expect("compiled generation advertises the selected capability");
        assert_eq!(rule.payload_schema, declared["payload_schema"]);
        assert_eq!(declared["supported_phases"], json!(["published"]));
    }

    let (status, listed) =
        request(&app, "GET", "/api/console/applications", &cookie, "", None).await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    let before_payload = take_payload(&before_subscription);
    let completion_payload = take_payload(&completion_subscription);
    assert_eq!(before_payload["interface_id"], "applications.list");
    assert_eq!(before_payload["phase"], "before");
    assert_eq!(before_payload["input"], json!({"variant":"List"}));
    assert!(before_payload.get("output").is_none());
    assert_eq!(completion_payload["interface_id"], "applications.list");
    assert_eq!(completion_payload["phase"], "completion");
    assert_eq!(completion_payload["terminal"], "completed");
    assert_eq!(
        completion_payload["invocation_id"],
        before_payload["invocation_id"]
    );
    assert!(Uuid::parse_str(before_payload["invocation_id"].as_str().unwrap()).is_ok());
    assert_eq!(completion_payload["output"]["variant"], "Applications");
    assert_eq!(completion_payload["output"]["0"], listed["data"]);
    let actual = completion_payload["output"]["0"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == application["data"]["id"])
        .unwrap();
    assert_eq!(actual["name"], "Actual event app");
    assert_eq!(actual["description"], "Actual event description");
    assert!(jsonschema::validator_for(&before.payload_schema)
        .unwrap()
        .is_valid(&before_payload));
    let completion_validator = jsonschema::validator_for(&completion.payload_schema).unwrap();
    assert!(completion_validator.is_valid(&completion_payload));
    assert!(before_subscription.try_recv().unwrap().is_none());
    assert!(completion_subscription.try_recv().unwrap().is_none());
    assert!(foreign.try_recv().unwrap().is_none());

    // The catalog's terminal contract permits cancellation facts without business output.
    // A cancellation must never manufacture the successful output seen above.
    let mut cancelled = completion_payload.clone();
    cancelled["terminal"] = json!("cancelled");
    cancelled.as_object_mut().unwrap().remove("output");
    assert!(completion_validator.is_valid(&cancelled));
    cancelled["output"] = completion_payload["output"].clone();
    assert!(!completion_validator.is_valid(&cancelled));

    let failed = create_rule(
        &app,
        &state,
        &cookie,
        &csrf,
        interface_rule("applications.get", "completion", &["failed"]),
    )
    .await;
    let success_only = create_rule(
        &app,
        &state,
        &cookie,
        &csrf,
        interface_rule("applications.get", "completion", &["completed"]),
    )
    .await;
    let failed_subscription = subscribe_rule(&state, &failed, failed.workspace_id);
    let success_subscription = subscribe_rule(&state, &success_only, success_only.workspace_id);
    let (status, error) = request(
        &app,
        "GET",
        &format!("/api/console/applications/{}", Uuid::now_v7()),
        &cookie,
        "",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{error}");
    let failed_payload = take_payload(&failed_subscription);
    assert_eq!(failed_payload["interface_id"], "applications.get");
    assert_eq!(failed_payload["phase"], "completion");
    assert_eq!(failed_payload["terminal"], "failed");
    assert!(failed_payload.get("output").is_none());
    assert!(jsonschema::validator_for(&failed.payload_schema)
        .unwrap()
        .is_valid(&failed_payload));
    assert!(failed_subscription.try_recv().unwrap().is_none());
    assert!(success_subscription.try_recv().unwrap().is_none());
    // Unauthenticated calls establish no workspace and cannot publish a rule fact.
    assert_eq!(
        request(&app, "GET", "/api/console/applications", "", "", None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert!(before_subscription.try_recv().unwrap().is_none());
    assert!(completion_subscription.try_recv().unwrap().is_none());
}

#[path = "event_center_routes/api_operations.rs"]
mod api_operations;

#[path = "event_center_routes/json_contract.rs"]
mod json_contract;
