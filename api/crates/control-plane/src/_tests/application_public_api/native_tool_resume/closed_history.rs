use super::*;
use crate::application_public_api::compat::openai::translate_response_request;

fn call(id: &str, custom: bool) -> Value {
    if custom {
        json!({"type":"custom_tool_call","call_id":id,"name":"exec","input":"read\0file"})
    } else {
        json!({"type":"function_call","call_id":id,"name":"read",
            "arguments":"{ \"z\":1, \"a\": [null, \"\\u0000\"] }",
            "opaque":{"future":[1,false,null]}})
    }
}

fn output(id: &str, custom: bool) -> Value {
    json!({"type":if custom { "custom_tool_call_output" } else { "function_call_output" },
        "call_id":id,"output":[{"type":"input_text","text":"result\0text"}],
        "opaque":{"keep":"verbatim"}})
}

#[test]
fn closed_history_matches_the_pair_count_oracle_for_all_small_sequences() {
    // Eight symbols: two IDs × two families × call/output. The oracle counts
    // positions independently of the production scan's per-call state machine.
    let symbols: Vec<Value> = (0..8)
        .map(|symbol| {
            let id = if symbol / 4 == 0 { "c0" } else { "c1" };
            if symbol % 2 == 0 {
                call(id, (symbol / 2) % 2 == 1)
            } else {
                output(id, (symbol / 2) % 2 == 1)
            }
        })
        .collect();
    let mut checked = 0;
    for len in 0..=5u32 {
        for mut encoding in 0..8usize.pow(len) {
            let sequence: Vec<usize> = (0..len)
                .map(|_| {
                    let symbol = encoding % 8;
                    encoding /= 8;
                    symbol
                })
                .collect();
            let expected = sequence.iter().any(|symbol| symbol % 2 == 1)
                && (0..2).all(|id| {
                    let calls: Vec<usize> = (0..sequence.len())
                        .filter(|&p| sequence[p] / 4 == id && sequence[p].is_multiple_of(2))
                        .collect();
                    let outputs: Vec<usize> = (0..sequence.len())
                        .filter(|&p| sequence[p] / 4 == id && sequence[p] % 2 == 1)
                        .collect();
                    (calls.is_empty() && outputs.is_empty())
                        || (calls.len() == 1
                            && outputs.len() == 1
                            && calls[0] < outputs[0]
                            && sequence[calls[0]] / 2 == sequence[outputs[0]] / 2)
                });
            let items: Vec<Value> = sequence.iter().map(|&s| symbols[s].clone()).collect();
            assert_eq!(is_closed_tool_history(&items), expected, "{sequence:?}");
            checked += 1;
        }
    }
    assert_eq!(checked, 37_449);
}

#[tokio::test]
async fn closed_history_samples_without_an_assistant_and_preserves_the_entire_wire_body() {
    let (repository, actor, _) = fixture().await;
    for (first_custom, second_custom) in [(false, false), (true, true), (false, true)] {
        let c1 = call("c1", first_custom);
        let c2 = call("c2", second_custom);
        let o1 = output("c1", first_custom);
        let o2 = output("c2", second_custom);
        for tools in [
            vec![c1.clone(), o1.clone()],
            vec![c1.clone(), c2.clone(), o2.clone(), o1.clone()],
            vec![c1.clone(), o1.clone(), c2.clone(), o2.clone()],
        ] {
            let mut items = vec![json!({"role":"user","content":"Read files"})];
            items.extend(tools);
            items.insert(
                2,
                json!({"type":"future_boundary","encrypted":"opaque\0payload"}),
            );
            items.push(json!({"role":"user","content":"Continue"}));
            let body = json!({"model":"fixture","input":items,"future":{"retain":true}});
            assert!(is_closed_tool_history(body["input"].as_array().unwrap()));
            for parent in [None, Some(Value::Null)] {
                let mut candidate = body.clone();
                if let Some(parent) = parent {
                    candidate["previous_response_id"] = parent;
                }
                let original = candidate.clone();
                let envelope = OpenAiResponsesEnvelope::capture(candidate).unwrap();
                assert!(correlate_native_responses_callback_from_envelope(
                    &repository,
                    &actor,
                    &envelope
                )
                .await
                .expect("closed external history must start ordinary sampling")
                .is_none());
                let mut translated =
                    translate_response_request(envelope.raw_body().clone()).unwrap();
                assert_eq!(envelope.raw_body(), &original);
                assert_eq!(
                    translated
                        .request
                        .metadata
                        .take_provider_transport_payload()
                        .unwrap()
                        .wire_body(),
                    &original
                );
            }
        }
    }
    assert!(repository.callback_resume_attempts().is_empty());
}

#[test]
fn closed_history_requires_unique_ordered_same_family_pairs_over_the_whole_input() {
    let c = call("c", false);
    let o = output("c", false);
    let mut invalid = vec![
        vec![],
        vec![c.clone()],
        vec![o.clone()],
        vec![o.clone(), c.clone()],
        vec![c.clone(), c.clone(), o.clone()],
        vec![c.clone(), o.clone(), c.clone(), o.clone()],
        vec![c.clone(), call("c", true), o.clone()],
        vec![c.clone(), o.clone(), o.clone()],
        vec![call("c", true), output("c", true), output("c", true)],
        vec![c.clone(), output("c", true)],
        vec![call("c", true), o.clone()],
        vec![call("unanswered", false), c.clone(), o.clone()],
        vec![output("unknown", false), c.clone(), o.clone()],
        vec![
            call("old", false),
            output("old", true),
            c.clone(),
            o.clone(),
        ],
        vec![
            call("old", false),
            output("old", false),
            output("old", false),
            c.clone(),
            o.clone(),
        ],
    ];
    for kind in [
        "function_call",
        "custom_tool_call",
        "custom_tool_call_output",
    ] {
        for id in [None, Some(Value::Null), Some(json!("")), Some(json!(7))] {
            let mut malformed = json!({"type":kind,"output":"bad"});
            if let Some(id) = id {
                malformed["call_id"] = id;
            }
            invalid.push(vec![malformed, c.clone(), o.clone()]);
        }
    }
    for id in [json!(""), json!(7)] {
        let mut malformed = json!({"type":"function_call_output","output":"bad"});
        malformed["call_id"] = id;
        invalid.push(vec![malformed, c.clone(), o.clone()]);
    }
    for (case, items) in invalid.iter().enumerate() {
        assert!(!is_closed_tool_history(items), "invalid shape {case}");
    }
    // Absent/null function output IDs retain their pre-existing standalone
    // context meaning. They neither close a call nor count as managed outputs.
    for notice in [
        json!({"type":"function_call_output","output":"context"}),
        json!({"type":"function_call_output","call_id":null,"output":"context"}),
    ] {
        assert!(!is_closed_tool_history(std::slice::from_ref(&notice)));
        assert!(!is_closed_tool_history(&[c.clone(), notice.clone()]));
        assert!(is_closed_tool_history(&[c.clone(), notice, o.clone()]));
    }
}

#[tokio::test]
async fn malformed_earlier_rounds_do_not_gain_admission_from_a_closed_final_suffix() {
    let (repository, actor, _) = fixture().await;
    for prefix in [
        vec![call("orphan", false)],
        vec![output("orphan", false)],
        vec![call("old", false), output("old", true)],
        vec![call("c", false), output("c", false)],
        vec![call("old", true), output("old", true), output("old", true)],
        vec![output("old", false), call("old", false)],
        vec![call("old", false), call("old", true), output("old", false)],
    ] {
        let mut items = prefix;
        items.extend([call("c", false), output("c", false)]);
        let body = json!({"model":"fixture","input":items});
        assert_eq!(
            correlate_native_responses_callback(&repository, &actor, &body)
                .await
                .unwrap_err()
                .downcast_ref::<ControlPlaneError>(),
            Some(&ControlPlaneError::Conflict("native_tool_output_unknown"))
        );
    }
}

#[tokio::test]
async fn closed_history_only_resumes_proven_owned_pending_or_completed_rounds() {
    for completed in [false, true] {
        let (repository, actor, run) =
            fixture_with_input(json!({"sys":{"requested_model_id":"fixture"}})).await;
        let (callback, body) = seed_proven_full_round(&repository, run, true);
        assert!(is_closed_tool_history(body["input"].as_array().unwrap()));
        if completed {
            repository.complete_callback_task_for_test(callback.id);
        }
        let (admitted, _) = correlate_native_responses_callback(&repository, &actor, &body)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(admitted.id, callback.id);
        assert_eq!(
            admitted.status,
            if completed {
                CallbackTaskStatus::Completed
            } else {
                CallbackTaskStatus::Pending
            }
        );
        let mut configuration = body.clone();
        configuration["model"] = json!("foreign");
        let mut history = body.clone();
        history["input"][1]["arguments"] = json!("tampered");
        let mut parent = body.clone();
        parent["previous_response_id"] = json!("resp_other");
        let mut partial = body.clone();
        partial["input"].as_array_mut().unwrap().remove(3);
        let stored = repository
            .get_published_callback_task(callback.id)
            .await
            .unwrap()
            .unwrap();
        assert!(
            correlate_native_responses_callback(&repository, &actor, &history)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            repository
                .get_published_callback_task(callback.id)
                .await
                .unwrap()
                .unwrap(),
            stored
        );
        for (candidate, code) in [
            (configuration, "native_tool_output_configuration_mismatch"),
            (parent, "native_tool_output_response_mismatch"),
            (partial, "native_tool_output_incomplete_round"),
        ] {
            assert_eq!(
                correlate_native_responses_callback(&repository, &actor, &candidate)
                    .await
                    .unwrap_err()
                    .downcast_ref::<ControlPlaneError>(),
                Some(&ControlPlaneError::Conflict(code))
            );
        }
        seed_proven_full_round(&repository, run, true);
        assert_eq!(
            correlate_native_responses_callback(&repository, &actor, &body)
                .await
                .unwrap_err()
                .downcast_ref::<ControlPlaneError>(),
            Some(&ControlPlaneError::Conflict(
                "native_tool_output_ambiguous_round"
            ))
        );
        assert!(repository.callback_resume_attempts().is_empty());
    }
}

#[tokio::test]
async fn closed_history_from_another_key_samples_without_claiming_its_callback_or_parent() {
    let (repository, actor, run) = fixture().await;
    let (callback, mut body) = seed_proven_full_round(&repository, run, true);
    let mut other = actor.clone();
    other.api_key_id = Uuid::now_v7();
    assert!(
        correlate_native_responses_callback(&repository, &other, &body)
            .await
            .unwrap()
            .is_none()
    );
    body["previous_response_id"] = json!("resp_round");
    assert_eq!(
        correlate_native_responses_callback(&repository, &other, &body)
            .await
            .unwrap_err()
            .downcast_ref::<ControlPlaneError>(),
        Some(&ControlPlaneError::Conflict("native_tool_output_unknown"))
    );
    let stored = repository
        .get_published_callback_task(callback.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.status, CallbackTaskStatus::Pending);
    assert!(stored.response_payload.is_none());
    assert!(repository.callback_resume_attempts().is_empty());
    assert_eq!(repository.flow_run_count(), 1);
}

#[tokio::test]
async fn closed_external_history_with_an_explicit_parent_keeps_the_unknown_round_error() {
    let (repository, actor, _) = fixture().await;
    let body = json!({"model":"fixture","previous_response_id":"resp_unknown",
        "input":[call("c", false), output("c", false)]});
    assert_eq!(
        correlate_native_responses_callback(&repository, &actor, &body)
            .await
            .unwrap_err()
            .downcast_ref::<ControlPlaneError>(),
        Some(&ControlPlaneError::Conflict("native_tool_output_unknown"))
    );
}
