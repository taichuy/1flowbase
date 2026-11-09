use super::*;
use control_plane::application_public_api::workflow_extension::{
    AuthorizedPublishedWorkflowOperation, WorkflowExtensionRunService,
};
use control_plane::event_center::{
    EventCenterRuntime, EventCenterRuntimePolicy, EventCenterSignalBody, EventCenterSubscription,
    EventRuntimeSubscription,
};
use control_plane_contracts::ports::{
    ApplicationLifecycleFact, ApplicationLifecycleObserver, ApplicationLifecyclePhase,
    AuthRepository, EventBusLimits, EventCenterRepository,
};
use domain::event_center::{EventDefinitionRecord, EventTriggerPhase};
use std::sync::Mutex;
use storage_ephemeral::MemoryEventBus;

#[derive(Default)]
struct ApplicationFacts(Mutex<Vec<ApplicationLifecycleFact>>);
impl ApplicationLifecycleObserver for ApplicationFacts {
    fn observe(&self, fact: ApplicationLifecycleFact) {
        self.0.lock().unwrap().push(fact);
    }
}

async fn event_app() -> (Router, Arc<ApiState>, Arc<ApplicationFacts>) {
    let (mut state, _) = test_api_state_with_database_url().await;
    let facts = Arc::new(ApplicationFacts::default());
    let owner = Arc::get_mut(&mut state).unwrap();
    owner.event_center = EventCenterRuntime::new(
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
    owner.event_sources = Arc::new(crate::app_state::EventSourceCatalog::for_test_support());
    // Capture real post-commit owner facts before assembly clones the store into execution ports.
    // This native test target does not depend on another target's private event fixture module.
    owner.store = owner
        .store
        .clone()
        .with_application_lifecycle_observer(facts.clone());
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    (app, state, facts)
}

async fn request(
    app: &Router,
    method: &str,
    path: &str,
    cookie: &str,
    csrf: &str,
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
                .body(if body.is_null() {
                    Body::empty()
                } else {
                    Body::from(body.to_string())
                })
                .unwrap(),
        )
        .await
        .unwrap();
    (response.status(), response_json(response).await)
}

async fn invoke(app: &Router, token: &str, slug: &str, customer: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/ex/{slug}"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"customer_id":customer}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    (response.status(), response_json(response).await)
}

async fn details(app: &Router, cookie: &str, operation: &str, slug: &str) -> Value {
    let (status, listed) = request(
        app,
        "GET",
        "/api/console/events/api-operations",
        cookie,
        "",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    let matches = listed["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["operation_id"] == operation)
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "one documented operation per publication");
    let (status, detail) = request(
        app,
        "GET",
        &format!("/api/console/events/api-operations/{operation}"),
        cookie,
        "",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{detail}");
    for key in [
        "operation_id",
        "method",
        "path",
        "scope",
        "before",
        "completion",
    ] {
        assert_eq!(
            matches[0][key], detail["data"][key],
            "catalog drift at {key}"
        );
    }
    assert_eq!(detail["data"]["path"], format!("/api/ex/{slug}"));
    assert_eq!(detail["data"]["before"]["status"], "available");
    assert_eq!(detail["data"]["completion"]["status"], "available");
    detail["data"].clone()
}

fn rule_body(operation: &str, phase: &str, enabled: bool) -> Value {
    json!({"name":format!("Workflow {phase}"),"description":"Actual published Workflow API observation",
        "enabled":enabled,"interface_event":{"operation_id":operation,"phase":phase,
            "results":if phase=="completion" {vec!["completed"]} else {vec![]}}})
}

async fn save_rule(
    app: &Router,
    state: &ApiState,
    cookie: &str,
    csrf: &str,
    operation: &str,
    phase: &str,
    existing: Option<Uuid>,
) -> EventDefinitionRecord {
    let path = existing
        .map(|id| format!("/api/console/events/{id}"))
        .unwrap_or_else(|| "/api/console/events".into());
    let (status, response) = request(
        app,
        if existing.is_some() { "PUT" } else { "POST" },
        &path,
        cookie,
        csrf,
        rule_body(operation, phase, true),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    let id = Uuid::parse_str(response["data"]["id"].as_str().unwrap()).unwrap();
    let saved = state
        .store
        .get_event_definition(state.bootstrap_workspace_id, id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.payload_schema, response["data"]["payload_schema"]);
    let (status, catalog) =
        request(app, "GET", "/api/console/events", cookie, "", Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{catalog}");
    let declared = catalog["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["contract_id"] == saved.contract_id)
        .expect("saved rule is discoverable through the actual event catalog");
    assert_eq!(declared["payload_schema"], saved.payload_schema);
    saved
}

fn subscribe(state: &ApiState, rule: &EventDefinitionRecord) -> EventCenterSubscription {
    state
        .event_center
        .subscribe(
            EventRuntimeSubscription {
                subscription_id: Uuid::now_v7(),
                workspace_id: rule.workspace_id,
                contract_id: rule.contract_id.clone(),
                trigger_phase: EventTriggerPhase::Published,
                consumer_id: Uuid::now_v7(),
            },
            false,
        )
        .unwrap()
}

fn take(subscription: &EventCenterSubscription, rule: &EventDefinitionRecord) -> Value {
    let delivery = subscription
        .try_recv()
        .unwrap()
        .expect("the actual invocation must publish one selected rule event");
    let EventCenterSignalBody::Published { payload } = &delivery.signal().body else {
        panic!("rule subscription must receive a published signal")
    };
    let validator = jsonschema::validator_for(&rule.payload_schema).unwrap();
    assert!(
        validator.is_valid(payload),
        "saved schema rejects actual payload: {payload}"
    );
    payload.clone()
}

async fn authorized(state: &ApiState, operation: &str) -> AuthorizedPublishedWorkflowOperation {
    let user = state
        .store
        .find_user_for_password_login(domain::BUILTIN_PASSWORD_LOGIN_ENTRY_ID, "root")
        .await
        .unwrap()
        .unwrap();
    let actor = domain::ActorContext::root(user.id, state.bootstrap_workspace_id, "root");
    WorkflowExtensionRunService::new(state.store.clone())
        .authorize_operation_id(&actor, operation)
        .await
        .unwrap()
}

#[tokio::test]
async fn full_api_workflow_sync_events_follow_actual_publication_and_withdrawal() {
    let (app, state, _) = event_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let first = create_workflow_application(&app, &cookie, &csrf, "event-ticket-first").await;
    let second = create_workflow_application(&app, &cookie, &csrf, "event-ticket-second").await;
    for id in [&first, &second] {
        save_workflow_document(&app, &cookie, &csrf, id).await;
    }
    let publication =
        publish_workflow_extension(&app, &cookie, &csrf, &first, "event-ticket-first", "sync")
            .await;
    let other_publication =
        publish_workflow_extension(&app, &cookie, &csrf, &second, "event-ticket-second", "sync")
            .await;
    let operation = publication["data"]["operation"]["interface_id"]
        .as_str()
        .unwrap();
    let other_operation = other_publication["data"]["operation"]["interface_id"]
        .as_str()
        .unwrap();
    assert_ne!(operation, other_operation);
    let registry = state
        .extension_boot_snapshot
        .as_ref()
        .unwrap()
        .interface_registry()
        .unwrap()
        .snapshot();
    let binding = interface_runtime::BindingId::new("http.workflow-extension.invoke.v1").unwrap();
    assert_eq!(
        registry
            .plan(&binding)
            .unwrap()
            .definition()
            .interface_id()
            .as_str(),
        "workflow-extension.invoke"
    );
    assert!(registry.document_operation(&binding).unwrap().dynamic);
    let original = authorized(&state, operation).await;
    let detail = details(&app, &cookie, operation, "event-ticket-first").await;
    let before = save_rule(&app, &state, &cookie, &csrf, operation, "before", None).await;
    let mut completion =
        save_rule(&app, &state, &cookie, &csrf, operation, "completion", None).await;
    let other = save_rule(
        &app,
        &state,
        &cookie,
        &csrf,
        other_operation,
        "completion",
        None,
    )
    .await;
    assert_eq!(before.payload_schema, detail["before_schema"]);
    assert_eq!(completion.payload_schema, detail["completion_schema"]);
    let before_events = subscribe(&state, &before);
    let completion_events = subscribe(&state, &completion);
    let other_events = subscribe(&state, &other);
    let token = create_user_api_key(&app, &cookie, &csrf).await;

    let (status, output) = invoke(&app, &token, "event-ticket-first", "C-first").await;
    assert_eq!(status, StatusCode::OK, "{output}");
    assert_eq!(output, json!({"ticket_id":"ticket-C-first"}));
    let started = take(&before_events, &before);
    let completed = take(&completion_events, &completion);
    assert_eq!(started["operation_id"], operation);
    assert_eq!(started["phase"], "before");
    assert_eq!(started["input"], json!({"body":{"customer_id":"C-first"}}));
    assert!(started.get("output").is_none());
    assert_eq!(completed["operation_id"], operation);
    assert_eq!(completed["phase"], "completion");
    assert_eq!(completed["terminal"], "completed");
    assert_eq!(completed["invocation_id"], started["invocation_id"]);
    assert!(Uuid::parse_str(completed["invocation_id"].as_str().unwrap()).is_ok());
    assert_eq!(
        completed["output"], output,
        "event output is the HTTP business value"
    );
    assert!(completed.get("input").is_none());
    assert!(!started.to_string().contains(&token));
    assert!(
        other_events.try_recv().unwrap().is_none(),
        "shared canonical handler must not cross application identities"
    );
    assert!(before_events.try_recv().unwrap().is_none());
    assert!(completion_events.try_recv().unwrap().is_none());

    let (status, other_output) = invoke(&app, &token, "event-ticket-second", "C-second").await;
    assert_eq!(status, StatusCode::OK, "{other_output}");
    let other_completed = take(&other_events, &other);
    assert_eq!(other_completed["operation_id"], other_operation);
    assert_eq!(other_completed["output"], other_output);
    assert!(before_events.try_recv().unwrap().is_none());
    assert!(completion_events.try_recv().unwrap().is_none());
    assert!(other_events.try_recv().unwrap().is_none());

    save_workflow_document_with_builder(&app, &cookie, &csrf, &first, |flow_id| {
        let mut document = workflow_document(flow_id);
        document["graph"]["nodes"][2]["bindings"] =
            json!({"reference":{"kind":"selector","value":["node-transform","ticket_id"]}});
        document["graph"]["nodes"][2]["outputs"] =
            json!([{"key":"reference","title":"Reference","valueType":"string"}]);
        document
    })
    .await;
    publish_workflow_extension(&app, &cookie, &csrf, &first, "event-ticket-first", "sync").await;
    let replacement = authorized(&state, operation).await;
    assert_eq!(
        original.operation().publication_version_id,
        replacement.operation().publication_version_id,
        "republish keeps the persisted publication UUID"
    );
    assert_ne!(
        original.generation(),
        replacement.generation(),
        "generation follows the immutable owner snapshot"
    );
    assert_eq!(
        original.operation().result_schema["required"],
        json!(["ticket_id"])
    );
    assert_eq!(
        replacement.operation().result_schema["required"],
        json!(["reference"])
    );
    let changed = details(&app, &cookie, operation, "event-ticket-first").await;
    assert_ne!(changed["completion_schema"], completion.payload_schema);
    let contract_id = completion.contract_id.clone();
    completion = save_rule(
        &app,
        &state,
        &cookie,
        &csrf,
        operation,
        "completion",
        Some(completion.id),
    )
    .await;
    assert_eq!(completion.contract_id, contract_id);
    assert_eq!(completion.payload_schema, changed["completion_schema"]);
    let (status, output) = invoke(&app, &token, "event-ticket-first", "C-new").await;
    assert_eq!(status, StatusCode::OK, "{output}");
    assert_eq!(output, json!({"reference":"ticket-C-new"}));
    let started = take(&before_events, &before);
    let completed = take(&completion_events, &completion);
    assert_eq!(started["input"], json!({"body":{"customer_id":"C-new"}}));
    assert_eq!(completed["invocation_id"], started["invocation_id"]);
    assert_eq!(completed["output"], output);
    assert!(other_events.try_recv().unwrap().is_none());

    publish_workflow_extension_with_enabled(
        &app,
        &cookie,
        &csrf,
        &first,
        WorkflowExtensionPublishOptions::new("event-ticket-first", "sync", false),
    )
    .await;
    let (status, listed) = request(
        &app,
        "GET",
        "/api/console/events/api-operations",
        &cookie,
        "",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    assert!(!listed["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["operation_id"] == operation));
    assert!(listed["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["operation_id"] == other_operation));
    let (status, _) = request(
        &app,
        "GET",
        &format!("/api/console/events/api-operations/{operation}"),
        &cookie,
        "",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, rejected) = request(
        &app,
        "POST",
        "/api/console/events",
        &cookie,
        &csrf,
        rule_body(operation, "before", true),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{rejected}");
    let (status, response) = invoke(&app, &token, "event-ticket-first", "C-withdrawn").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{response}");
    for rule in [&before, &completion] {
        let (status, disabled) = request(
            &app,
            "PUT",
            &format!("/api/console/events/{}", rule.id),
            &cookie,
            &csrf,
            rule_body(
                operation,
                if rule.id == before.id {
                    "before"
                } else {
                    "completion"
                },
                false,
            ),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::OK,
            "withdrawn target must not prevent stopping an unchanged rule: {disabled}"
        );
        assert_eq!(disabled["data"]["enabled"], false);
        assert_eq!(disabled["data"]["payload_schema"], rule.payload_schema);
    }
    assert!(before_events.try_recv().unwrap().is_none());
    assert!(completion_events.try_recv().unwrap().is_none());
    assert!(other_events.try_recv().unwrap().is_none());
    before_events.close();
    completion_events.close();
    other_events.close();
}

#[tokio::test]
async fn full_api_workflow_async_completion_is_accepted_receipt_not_application_ended() {
    let (app, state, facts) = event_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let application = create_workflow_application(&app, &cookie, &csrf, "event-ticket-async").await;
    save_workflow_document(&app, &cookie, &csrf, &application).await;
    let publication = publish_workflow_extension(
        &app,
        &cookie,
        &csrf,
        &application,
        "event-ticket-async",
        "async",
    )
    .await;
    let operation = publication["data"]["operation"]["interface_id"]
        .as_str()
        .unwrap();
    let detail = details(&app, &cookie, operation, "event-ticket-async").await;
    let before = save_rule(&app, &state, &cookie, &csrf, operation, "before", None).await;
    let completion = save_rule(&app, &state, &cookie, &csrf, operation, "completion", None).await;
    assert_eq!(before.payload_schema, detail["before_schema"]);
    assert_eq!(completion.payload_schema, detail["completion_schema"]);
    let before_events = subscribe(&state, &before);
    let completion_events = subscribe(&state, &completion);
    let token = create_user_api_key(&app, &cookie, &csrf).await;
    let (status, receipt) = invoke(&app, &token, "event-ticket-async", "C-async").await;
    assert_eq!(status, StatusCode::ACCEPTED, "{receipt}");
    assert_eq!(receipt["status"], "queued");
    let run = Uuid::parse_str(receipt["run_id"].as_str().unwrap()).unwrap();
    let started = take(&before_events, &before);
    let completed = take(&completion_events, &completion);
    assert_eq!(started["operation_id"], operation);
    assert_eq!(started["input"], json!({"body":{"customer_id":"C-async"}}));
    assert_eq!(completed["operation_id"], operation);
    assert_eq!(completed["terminal"], "completed");
    assert_eq!(completed["invocation_id"], started["invocation_id"]);
    assert_eq!(
        completed["output"], receipt,
        "API Completion describes the accepted call result"
    );
    assert_ne!(completed["invocation_id"], receipt["run_id"]);
    assert!(completed["output"].get("ticket_id").is_none());
    let business_output = wait_for_flow_run_status(&state, run, "succeeded").await;
    assert_eq!(business_output, json!({"ticket_id":"ticket-C-async"}));
    let ended = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let fact = facts
                .0
                .lock()
                .unwrap()
                .iter()
                .find(|fact| {
                    fact.flow_run_id == run && fact.phase == ApplicationLifecyclePhase::Ended
                })
                .cloned();
            if let Some(fact) = fact {
                break fact;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("real execution commits its independent application Ended fact");
    assert_eq!(ended.application_id.to_string(), application);
    assert_eq!(ended.workspace_id, state.bootstrap_workspace_id);
    assert_eq!(ended.status, domain::FlowRunStatus::Succeeded);
    assert_eq!(ended.run_mode, domain::FlowRunMode::WorkflowHttpRun);
    assert!(
        jsonschema::validator_for(&ApplicationLifecycleFact::event_source_schema())
            .unwrap()
            .is_valid(&ended.event_payload())
    );
    // Ended may race the HTTP response; its business meaning is independent of API Completion.
    assert_eq!(completed["output"]["status"], "queued");
    assert_ne!(completed["output"], business_output);
    assert!(before_events.try_recv().unwrap().is_none());
    assert!(
        completion_events.try_recv().unwrap().is_none(),
        "application Ended must not emit a second API Completion"
    );
    before_events.close();
    completion_events.close();
}
