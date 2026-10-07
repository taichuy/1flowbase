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
                runtime_core::runtime_record_repository::OrderedTreeQueryError::InvalidCursor,
            ),
            StatusCode::BAD_REQUEST,
        ),
        (
            anyhow::Error::new(
                runtime_core::runtime_record_repository::OrderedTreeQueryError::StaleCursor,
            ),
            StatusCode::CONFLICT,
        ),
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
            "input_tokens": 49901,
            "total_tokens": 59901,
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
fn application_log_cache_hit_rate_corrects_historical_anthropic_records() {
    // Real records previously displayed 172.41%, 123.07%, and 109.75%.
    for model in ["application_run_log_summaries", "application_run_log_tasks"] {
        let records = [
            (827_119, 494_000, 286_532, 0.5973),
            (803_199, 444_600, 361_256, 0.5535),
            (280_611, 148_200, 135_040, 0.5281),
        ];
        let items = records
            .iter()
            .map(|&(input, hit, legacy_total, _)| {
                json!({
                    "input_tokens": input,
                    "input_cache_hit_tokens": hit,
                    "total_tokens": legacy_total
                })
            })
            .collect::<Vec<_>>();
        let list = runtime_list_response(model, items.clone(), items.len() as i64);
        for ((item, listed), (_, _, _, expected)) in items.into_iter().zip(list.items).zip(records)
        {
            assert_eq!(listed["input_cache_hit_rate"], json!(expected));
            assert_eq!(
                runtime_record_response(model, item)["input_cache_hit_rate"],
                json!(expected)
            );
        }
    }
}

#[test]
fn application_log_cache_hit_rate_uses_inclusive_input_without_adding_cache_twice() {
    // OpenAI input already includes the 800 cache reads. Output must not affect the rate.
    for output in [0, 100, 50_000] {
        let record = runtime_record_response(
            "application_run_log_summaries",
            json!({
                "input_tokens": 1000,
                "input_cache_hit_tokens": 800,
                "output_tokens": output,
                "total_tokens": 1000 + output
            }),
        );
        assert_eq!(record["input_cache_hit_rate"], json!(0.8));
    }
}

#[test]
fn application_log_cache_hit_rate_preserves_unknown_and_zero_input() {
    for input in [Value::Null, json!(0)] {
        let record = runtime_record_response(
            "application_run_log_tasks",
            json!({
                "input_tokens": input,
                "total_tokens": 1000,
                "input_cache_hit_tokens": 500,
                "input_cache_hit_rate": 0.5
            }),
        );
        assert_eq!(record["input_cache_hit_rate"], Value::Null);
    }
    for (hit, expected) in [(0, 0.0), (1000, 1.0)] {
        let record = runtime_record_response(
            "application_run_log_tasks",
            json!({
                "input_tokens": 1000,
                "input_cache_hit_tokens": hit,
                "total_tokens": 2000
            }),
        );
        assert_eq!(record["input_cache_hit_rate"], json!(expected));
    }
}

#[test]
fn application_log_task_cache_hit_rate_uses_aggregated_counts() {
    // Two calls: 90/100 and 0/900. The combined rate is 90/1000, not their mean.
    let record = runtime_record_response(
        "application_run_log_tasks",
        json!({
            "input_tokens": 1000,
            "input_cache_hit_tokens": 90,
            "total_tokens": 1200
        }),
    );
    assert_eq!(record["input_cache_hit_rate"], json!(0.09));
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
