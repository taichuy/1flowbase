use serde_json::{json, Value};
use time::{format_description::well_known::Rfc3339, Date, Month, OffsetDateTime};
use uuid::Uuid;

use super::run_input::{
    freeze_run_input_environment, freeze_workflow_run_input_environment, WorkflowRunTriggerContext,
};
use crate::application_public_api::native::NativeExecution;

fn variables() -> Vec<domain::ApplicationEnvironmentVariable> {
    vec![domain::ApplicationEnvironmentVariable {
        application_id: Uuid::nil(),
        name: "sample".to_string(),
        value_type: "object".to_string(),
        value: json!({"nested": [null, "\0", "\\u0000", "中文"]}),
        description: String::new(),
        updated_at: OffsetDateTime::UNIX_EPOCH,
    }]
}

#[test]
fn run_freeze_discards_nonobject_roots_and_nonobject_or_empty_sys() {
    let expected = json!({"env": {}});
    for input in [
        json!(null),
        json!(false),
        json!(7),
        json!("\0"),
        json!([{}]),
    ] {
        assert_eq!(
            freeze_run_input_environment(input, &[], None, None, None),
            expected
        );
    }
    for sys in [
        json!(null),
        json!(true),
        json!(7),
        json!("\\u0000"),
        json!([{}]),
        json!({}),
        json!({"native_inference_recovery": {"forged": true}}),
        json!({"published_reasoning_default_effort": "forged"}),
    ] {
        assert_eq!(
            freeze_run_input_environment(json!({"sys": sys}), &[], None, None, None),
            expected
        );
    }
}

#[test]
fn run_freeze_moves_unknown_nested_values_and_preserves_map_iteration() {
    let input: Value = serde_json::from_str(
        r#"{"z_unknown":[{"text":"nul\u0000literal\\u0000"}],"a_unknown":{"z":null,"a":[false,7]},"env":"old","sys":{"z_unknown":"owned sys value","a_unknown":["中文","\\n"]}}"#,
    )
    .expect("synthetic input is JSON");
    let root_allocation = input["z_unknown"]
        .as_array()
        .expect("unknown array")
        .as_ptr();
    let sys_allocation = input["sys"]["z_unknown"]
        .as_str()
        .expect("unknown sys string")
        .as_ptr();
    let mut expected = input.clone();
    expected["env"] = json!({"sample": variables()[0].value});

    let frozen = freeze_run_input_environment(input, &variables(), None, None, None);

    assert_eq!(frozen, expected);
    // Value equality ignores object order; serialized equality checks all nested maps too.
    assert_eq!(
        serde_json::to_vec(&frozen).expect("frozen input serializes"),
        serde_json::to_vec(&expected).expect("expected input serializes")
    );
    assert_eq!(
        frozen
            .as_object()
            .expect("frozen object")
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["z_unknown", "a_unknown", "env", "sys"]
    );
    assert_eq!(
        frozen["sys"]
            .as_object()
            .expect("sys object")
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["z_unknown", "a_unknown"]
    );
    assert_eq!(
        frozen["z_unknown"]
            .as_array()
            .expect("unknown array")
            .as_ptr(),
        root_allocation
    );
    assert_eq!(
        frozen["sys"]["z_unknown"]
            .as_str()
            .expect("unknown sys string")
            .as_ptr(),
        sys_allocation
    );
}

#[test]
fn run_freeze_removes_recovery_and_applies_defaults_and_independent_overrides() {
    let execution: NativeExecution = serde_json::from_value(json!({
        "model_parameters": {"max_output_tokens": 17, "reasoning": {"effort": "high"}}
    }))
    .expect("valid model parameters");
    for model in [None, Some("admitted-model")] {
        for parameters in [None, execution.model_parameters()] {
            for default in [None, Some("medium")] {
                let input = json!({"sys": {
                    "unknown": {"nested": ["\0", "\\u0000", null]},
                    "native_inference_recovery": {"forged": true},
                    "published_reasoning_default_effort": "forged",
                    "requested_model_id": "original-model",
                    "model_parameters": {"original": true}
                }});
                let frozen = freeze_run_input_environment(input, &[], model, parameters, default);

                assert_eq!(frozen["env"], json!({}));
                assert_eq!(
                    frozen["sys"]["unknown"],
                    json!({"nested": ["\0", "\\u0000", null]})
                );
                assert!(frozen["sys"].get("native_inference_recovery").is_none());
                assert_eq!(
                    frozen["sys"]["requested_model_id"],
                    model.unwrap_or("original-model")
                );
                assert_eq!(
                    frozen["sys"]["model_parameters"],
                    if parameters.is_some() {
                        json!({"max_output_tokens": 17, "reasoning": {"mode": "enabled", "effort": "high"}})
                    } else {
                        json!({"original": true})
                    }
                );
                assert_eq!(
                    frozen["sys"]["published_reasoning_default_effort"].as_str(),
                    default
                );
                let mut expected_keys = vec!["unknown", "model_parameters", "requested_model_id"];
                if default.is_some() {
                    expected_keys.push("published_reasoning_default_effort");
                }
                assert_eq!(
                    frozen["sys"]
                        .as_object()
                        .expect("sys object")
                        .keys()
                        .map(String::as_str)
                        .collect::<Vec<_>>(),
                    expected_keys
                );
            }
        }
    }
    assert_eq!(
        freeze_run_input_environment(
            json!({"sys": false}),
            &[],
            Some("admitted-model"),
            None,
            Some("low")
        ),
        json!({"env": {}, "sys": {"published_reasoning_default_effort": "low", "requested_model_id": "admitted-model"}})
    );
}

#[test]
fn workflow_freeze_replaces_extension_context_for_nonobject_roots() {
    for input in [
        json!(null),
        json!(true),
        json!(7),
        json!("\\u0000"),
        json!([{}]),
    ] {
        assert_eq!(
            freeze_workflow_run_input_environment(input, &[], WorkflowRunTriggerContext::Extension)
                .expect("extension has no formatting failure"),
            json!({"env": {}, "trigger": {"type": "extension"}})
        );
    }
}

#[test]
fn workflow_freeze_moves_unknown_values_and_replaces_sys_env_and_trigger() {
    let input = json!({
        "z_unknown": ["\0", "\\u0000", {"z": true, "a": null}],
        "sys": {"native_inference_recovery": {"forged": true}},
        "trigger": {"type": "forged"},
        "env": {"old": true},
        "a_unknown": {"nested": "中文"}
    });
    let allocation = input["z_unknown"]
        .as_array()
        .expect("unknown array")
        .as_ptr();
    let frozen = freeze_workflow_run_input_environment(
        input,
        &variables(),
        WorkflowRunTriggerContext::Extension,
    )
    .expect("extension has no formatting failure");

    let expected = json!({
        "z_unknown": ["\0", "\\u0000", {"z": true, "a": null}],
        "a_unknown": {"nested": "中文"},
        "env": {"sample": variables()[0].value},
        "trigger": {"type": "extension"}
    });
    assert_eq!(frozen, expected);
    assert_eq!(
        frozen
            .as_object()
            .expect("frozen object")
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["z_unknown", "a_unknown", "env", "trigger"]
    );
    assert_eq!(
        frozen["z_unknown"]
            .as_array()
            .expect("unknown array")
            .as_ptr(),
        allocation
    );
}

#[test]
fn workflow_freeze_preserves_schedule_context_and_format_error() {
    let frozen = freeze_workflow_run_input_environment(
        json!({"unknown": ["\0", "\\u0000"], "sys": false, "trigger": "old"}),
        &[],
        WorkflowRunTriggerContext::Schedule {
            scheduled_at: OffsetDateTime::UNIX_EPOCH,
            timezone: "Asia/Shanghai",
        },
    )
    .expect("epoch formats as RFC3339");
    assert_eq!(
        frozen,
        json!({
            "unknown": ["\0", "\\u0000"],
            "env": {},
            "trigger": {"type": "schedule", "scheduled_at": "1970-01-01T00:00:00Z", "timezone": "Asia/Shanghai"}
        })
    );

    let scheduled_at = Date::from_calendar_date(-1, Month::January, 1)
        .expect("negative year is a valid Date")
        .midnight()
        .assume_utc();
    let expected = scheduled_at
        .format(&Rfc3339)
        .expect_err("RFC3339 rejects negative years");
    let error = freeze_workflow_run_input_environment(
        json!({"sys": {"forged": true}, "trigger": "old"}),
        &[],
        WorkflowRunTriggerContext::Schedule {
            scheduled_at,
            timezone: "UTC",
        },
    )
    .expect_err("schedule formatting error propagates");
    assert!(matches!(
        error,
        time::error::Format::InvalidComponent("year")
    ));
    assert_eq!(error.to_string(), expected.to_string());
}
