use axum::{http::StatusCode, response::IntoResponse};
use serde_json::json;

use super::*;

// AC-012: typed ordered-tree errors keep stable HTTP status classes.
#[test]
fn ordered_tree_errors_map_to_bad_request_not_found_conflict_and_unavailable() {
    use runtime_core::runtime_record_repository::OrderedTreeCommandError;

    let cases = [
        (
            anyhow::Error::new(
                runtime_core::runtime_engine::RuntimeModelError::InvalidOperationInput("payload"),
            ),
            StatusCode::BAD_REQUEST,
        ),
        (
            anyhow::Error::new(OrderedTreeCommandError::NodeNotFound),
            StatusCode::NOT_FOUND,
        ),
        (
            anyhow::Error::new(OrderedTreeCommandError::TreeNodeHasChildren),
            StatusCode::CONFLICT,
        ),
        (
            anyhow::Error::new(
                runtime_core::runtime_engine::RuntimeModelError::OrderedTreeUnavailable,
            ),
            StatusCode::SERVICE_UNAVAILABLE,
        ),
    ];

    for (error, expected) in cases {
        assert_eq!(map_runtime_error(error).into_response().status(), expected);
    }
}

#[test]
fn runtime_record_response_rounds_application_log_cache_hit_rate() {
    let record = runtime_record_response(
        "application_run_log_summaries",
        json!({
            "id": "run-1",
            "run_mode": "debug_flow_run",
            "total_tokens": 49901,
            "input_cache_hit_tokens": 49063,
            "input_cache_hit_rate": 0.9505703422053232
        }),
    );

    assert_eq!(record["input_cache_hit_rate"], json!(0.9832));
}

#[test]
fn runtime_record_response_does_not_fall_back_to_projected_cache_hit_rate() {
    let record = runtime_record_response(
        "application_run_log_summaries",
        json!({
            "id": "run-1",
            "run_mode": "debug_flow_run",
            "input_cache_hit_rate": 1.0
        }),
    );

    assert_eq!(record["input_cache_hit_rate"], Value::Null);
}

#[test]
fn application_run_records_receive_nullable_count_tokens_results() {
    let count_tokens_run_id = Uuid::now_v7();
    let generate_run_id = Uuid::now_v7();
    let mut records = vec![
        json!({ "flow_run_id": count_tokens_run_id }),
        json!({ "flow_run_id": generate_run_id }),
    ];

    apply_application_run_count_tokens_results(
        &mut records,
        &[control_plane::ports::ApplicationRunCountTokensResult {
            flow_run_id: count_tokens_run_id,
            input_tokens: 6_956,
        }],
    );

    assert_eq!(records[0]["count_tokens_input_tokens"], json!(6_956));
    assert_eq!(records[1]["count_tokens_input_tokens"], Value::Null);
}

#[test]
fn runtime_record_response_leaves_other_models_unchanged() {
    let record = runtime_record_response(
        "orders",
        json!({
            "id": "order-1",
            "input_cache_hit_rate": 0.9505703422053232
        }),
    );

    assert_eq!(record["input_cache_hit_rate"], json!(0.9505703422053232));
}

#[test]
fn runtime_record_response_derives_principal_from_run_credentials() {
    for (run_mode, invocation_source, principal_kind, keeps_creator) in [
        ("workflow_http_run", "workflow_http", "user", true),
        (
            "workflow_schedule_run",
            "workflow_schedule",
            "scheduler",
            false,
        ),
    ] {
        let creator_id = Uuid::now_v7();
        let record = runtime_record_response(
            "application_run_log_summaries",
            json!({
                "run_mode": run_mode,
                "created_by": creator_id.to_string(),
                "authorized_account": "publication creator"
            }),
        );

        assert_eq!(record["execution_stage"], json!("published"));
        assert_eq!(record["invocation_source"], json!(invocation_source));
        assert_eq!(record["principal"]["kind"], json!(principal_kind));
        if keeps_creator {
            assert_eq!(record["principal"]["id"], json!(creator_id));
            assert_eq!(
                record["principal"]["display_name"],
                json!("publication creator")
            );
        } else {
            assert_eq!(record["principal"]["id"], Value::Null);
            assert_eq!(record["principal"]["display_name"], Value::Null);
        }
    }
}
