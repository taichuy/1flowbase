use super::*;
use std::collections::BTreeSet;

fn api_rule(operation_id: &str, phase: &str) -> Value {
    json!({"name":"Documented API observation","description":"Full API fixture","enabled":true,
        "interface_event":{"operation_id":operation_id,"phase":phase}})
}
async fn operation(app: &Router, cookie: &str, method: &str, path: &str) -> Value {
    let (status, catalog) = request(
        app,
        "GET",
        "/api/console/events/api-operations",
        cookie,
        "",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{catalog}");
    catalog["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["method"] == method && row["path"] == path)
        .unwrap_or_else(|| panic!("complete event API directory lacks {method} {path}"))
        .clone()
}
async fn detail(app: &Router, cookie: &str, operation_id: &str) -> Value {
    let (status, value) = request(
        app,
        "GET",
        &format!("/api/console/events/api-operations/{operation_id}"),
        cookie,
        "",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    value["data"].clone()
}
#[tokio::test]
async fn event_center_api_directory_matches_complete_shared_catalog_including_mcp_restricted_rows()
{
    let state = event_state().await;
    let app = crate::app_with_state_and_config(state, &test_config());
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;
    let (status, events) = request(
        &app,
        "GET",
        "/api/console/events/api-operations",
        &cookie,
        "",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{events}");
    let (status, mcp) = request(
        &app,
        "GET",
        "/api/console/mcp/interface-capabilities?bindable_only=false",
        &cookie,
        "",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{mcp}");
    let rows = events["data"].as_array().unwrap();
    let shared = mcp["data"].as_array().unwrap();
    let tuple = |row: &Value, id: &str| {
        (
            row[id].as_str().unwrap().to_owned(),
            row["method"].as_str().unwrap().to_owned(),
            row["path"].as_str().unwrap().to_owned(),
        )
    };
    let event_set = rows
        .iter()
        .map(|row| tuple(row, "operation_id"))
        .collect::<BTreeSet<_>>();
    let shared_set = shared
        .iter()
        .map(|row| tuple(row, "interface_id"))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        event_set, shared_set,
        "no event whitelist or MCP.bindable filter"
    );
    assert!(rows.len() > 26);
    assert_eq!(event_set.len(), rows.len(), "operation rows must be unique");
    assert!(shared.iter().any(|row| row["bindable"] == false));
    for row in rows {
        assert!(
            row.get("before_schema").is_none() && row.get("completion_schema").is_none(),
            "list is metadata only"
        );
        for phase in ["before", "completion"] {
            assert!(["available", "unavailable"].contains(&row[phase]["status"].as_str().unwrap()));
            if row[phase]["status"] == "unavailable" {
                assert!(row[phase]["reason"].is_string());
            }
            assert_ne!(
                row[phase]["reason"], "missing_typed_codec",
                "ordinary API codec inventory must be complete: {row}"
            );
        }
    }
    assert_eq!(
        request(
            &app,
            "GET",
            "/api/console/events/api-operations",
            "",
            "",
            None
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn event_center_document_rule_outside_legacy_catalog_observes_real_query_and_result_once() {
    let state = event_state().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let path = "/api/console/settings/applications";
    let selected = operation(&app, &cookie, "GET", path).await;
    assert_eq!(selected["before"]["status"], "available", "{selected}");
    assert_eq!(selected["completion"]["status"], "available", "{selected}");
    let id = selected["operation_id"].as_str().unwrap();
    let details = detail(&app, &cookie, id).await;
    let before = create_rule(&app, &state, &cookie, &csrf, api_rule(id, "before")).await;
    let completion = create_rule(&app, &state, &cookie, &csrf, api_rule(id, "completion")).await;
    assert_eq!(before.payload_schema, details["before_schema"]);
    assert_eq!(completion.payload_schema, details["completion_schema"]);
    let before_queue = subscribe_rule(&state, &before, before.workspace_id);
    let completion_queue = subscribe_rule(&state, &completion, completion.workspace_id);
    let foreign_queue = subscribe_rule(&state, &completion, Uuid::now_v7());
    let (status, output) = request(
        &app,
        "GET",
        &format!("{path}?page=1&page_size=7&unused_credential=SENTINEL"),
        &cookie,
        "",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{output}");
    let input_event = take_payload(&before_queue);
    let output_event = take_payload(&completion_queue);
    assert_eq!(input_event["operation_id"], id);
    assert!(input_event.get("interface_id").is_none());
    assert_eq!(input_event["input"]["query"]["page"], 1);
    assert_eq!(input_event["input"]["query"]["page_size"], 7);
    assert!(!input_event.to_string().contains("SENTINEL"));
    assert_eq!(output_event["terminal"], "completed");
    assert_eq!(
        output_event["output"], output["data"],
        "actual DTO, no byte/item summary"
    );
    assert!(jsonschema::validator_for(&before.payload_schema)
        .unwrap()
        .is_valid(&input_event));
    assert!(jsonschema::validator_for(&completion.payload_schema)
        .unwrap()
        .is_valid(&output_event));
    for queue in [&before_queue, &completion_queue, &foreign_queue] {
        assert!(queue.try_recv().unwrap().is_none());
        queue.close();
    }
    let mut mixed = api_rule(id, "before");
    mixed["interface_event"]["interface_id"] = json!("applications.list");
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/console/events",
            &cookie,
            &csrf,
            Some(mixed)
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test]
async fn event_center_document_key_creation_preserves_http_token_and_omits_it_from_event_schema_and_value(
) {
    let state = event_state().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let selected = operation(&app, &cookie, "POST", "/api/console/user-api-keys").await;
    let id = selected["operation_id"].as_str().unwrap();
    let rule = create_rule(&app, &state, &cookie, &csrf, api_rule(id, "completion")).await;
    let queue = subscribe_rule(&state, &rule, rule.workspace_id);
    let (status, response) = request(
        &app,
        "POST",
        "/api/console/user-api-keys",
        &cookie,
        &csrf,
        Some(json!({"name":"Observed owned key","expiration_policy":"never"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{response}");
    let token = response["data"]["token"]
        .as_str()
        .expect("HTTP creation still returns token");
    assert!(!token.is_empty());
    let event = take_payload(&queue);
    assert_eq!(event["output"]["name"], "Observed owned key");
    assert!(event["output"].get("token").is_none());
    assert!(!event.to_string().contains(token));
    assert!(!rule.payload_schema.to_string().contains("\"token\""));
    assert!(jsonschema::validator_for(&rule.payload_schema)
        .unwrap()
        .is_valid(&event));
    let (_, runtime) = request(
        &app,
        "GET",
        "/api/console/events/runtime",
        &cookie,
        "",
        None,
    )
    .await;
    assert!(!runtime.to_string().contains(token));
    queue.close();
}

async fn create_model(app: &Router, cookie: &str, csrf: &str, code: &str) -> String {
    let base = "/api/console/settings/data-models/model-definitions";
    let (status, created) = request(app, "POST", base, cookie, csrf, Some(json!({"scope_kind":"workspace","template_provider":"core","template_code":"general","template_version":"v1","code":code,"title":code,"status":"published"}))).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["data"]["id"].as_str().unwrap().to_owned();
    let (status, field) = request(app, "POST", &format!("{base}/{id}/fields"), cookie, csrf, Some(json!({"code":"order_title","title":"Order title","field_kind":"string","is_required":true}))).await;
    assert_eq!(status, StatusCode::CREATED, "{field}");
    let (status, published) = request(
        app,
        "PATCH",
        &format!("{base}/{id}"),
        cookie,
        csrf,
        Some(json!({"status":"published"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{published}");
    id
}
#[tokio::test]
async fn event_center_dynamic_models_same_verb_keep_distinct_rules_and_actual_owner_values() {
    let state = event_state().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let model_a = create_model(&app, &cookie, &csrf, "events_model_a").await;
    let model_b = create_model(&app, &cookie, &csrf, "events_model_b").await;
    let op_a = format!("data_model__{model_a}__create_record");
    let op_b = format!("data_model__{model_b}__create_record");
    let rule_a = create_rule(&app, &state, &cookie, &csrf, api_rule(&op_a, "before")).await;
    let rule_b = create_rule(&app, &state, &cookie, &csrf, api_rule(&op_b, "before")).await;
    let completed = create_rule(&app, &state, &cookie, &csrf, api_rule(&op_a, "completion")).await;
    let queue_a = subscribe_rule(&state, &rule_a, rule_a.workspace_id);
    let queue_b = subscribe_rule(&state, &rule_b, rule_b.workspace_id);
    let queue_completed = subscribe_rule(&state, &completed, completed.workspace_id);
    let (status, created) = request(
        &app,
        "POST",
        "/api/runtime/models/events_model_a/create",
        &cookie,
        &csrf,
        Some(json!({"order_title":"Actual model A"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let before = take_payload(&queue_a);
    let completion = take_payload(&queue_completed);
    assert_eq!(before["operation_id"], op_a);
    assert_eq!(before["input"]["body"]["order_title"], "Actual model A");
    assert_eq!(completion["output"], created["data"]);
    assert!(jsonschema::validator_for(&rule_a.payload_schema)
        .unwrap()
        .is_valid(&before));
    assert!(jsonschema::validator_for(&completed.payload_schema)
        .unwrap()
        .is_valid(&completion));
    assert!(
        queue_b.try_recv().unwrap().is_none(),
        "same canonical POST must not cross models"
    );
    let (status, created_b) = request(
        &app,
        "POST",
        "/api/runtime/models/events_model_b/create",
        &cookie,
        &csrf,
        Some(json!({"order_title":"Actual model B"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created_b}");
    assert_eq!(take_payload(&queue_b)["operation_id"], op_b);
    assert!(queue_a.try_recv().unwrap().is_none());
    let (status, withdrawn) = request(
        &app,
        "PATCH",
        &format!("/api/console/settings/data-models/model-definitions/{model_a}"),
        &cookie,
        &csrf,
        Some(json!({"status":"draft"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{withdrawn}");
    let mut disabled = api_rule(&op_a, "before");
    disabled["enabled"] = json!(false);
    assert_eq!(
        request(
            &app,
            "PUT",
            &format!("/api/console/events/{}", rule_a.id),
            &cookie,
            &csrf,
            Some(disabled)
        )
        .await
        .0,
        StatusCode::OK,
        "stopping withdrawn unchanged rule remains possible"
    );
    for queue in [&queue_a, &queue_b, &queue_completed] {
        queue.close();
    }
}
