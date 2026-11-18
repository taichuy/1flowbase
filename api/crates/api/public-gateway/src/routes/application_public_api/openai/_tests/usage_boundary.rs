use super::*;
use serde_json::json;

fn usage(input: u64, output: u64, cached: u64, reasoning: u64) -> NativeUsage {
    NativeUsage {
        prompt_tokens: Some(input),
        completion_tokens: Some(output),
        total_tokens: Some(input + output),
        cache_read_tokens: Some(cached),
        reasoning_tokens: Some(reasoning),
        ..NativeUsage::default()
    }
}

fn boundary(usage: Option<NativeUsage>) -> ResponseUsageBaseline {
    ResponseUsageBaseline::from_callback_payload(
        &json!({"native_usage_baseline_source":"usage_ledger","native_usage_baseline":usage}),
    )
}

#[test]
fn response_round_counters_do_not_recount_fifteen_step_flow_history() {
    // Numeric evidence only, from the incident's independent provider ledger.
    let input = [
        9334, 9453, 9548, 12683, 19003, 28341, 32460, 32699, 33474, 34037, 35462, 36265, 37755,
        37872, 38009,
    ];
    let output = [
        81, 67, 54, 143, 111, 220, 211, 27, 523, 681, 763, 49, 89, 68, 1379,
    ];
    let cache = [
        0, 9344, 9472, 9472, 12672, 9216, 28416, 32512, 28288, 33920, 33408, 36096, 36224, 37760,
        37888,
    ];
    let mut cumulative = usage(0, 0, 0, 0);
    let mut projected_total = 0;
    let mut old_double_counted_total = 0;
    for step in 0..input.len() {
        let baseline = if step == 0 {
            ResponseUsageBaseline::NewRun
        } else {
            boundary(Some(cumulative.clone()))
        };
        cumulative.prompt_tokens = Some(cumulative.prompt_tokens.unwrap() + input[step]);
        cumulative.completion_tokens = Some(cumulative.completion_tokens.unwrap() + output[step]);
        cumulative.total_tokens =
            Some(cumulative.total_tokens.unwrap() + input[step] + output[step]);
        cumulative.cache_read_tokens = Some(cumulative.cache_read_tokens.unwrap() + cache[step]);
        let round = baseline.project_responses(Some(&cumulative)).unwrap();
        assert_eq!(round.input_tokens, input[step]);
        assert_eq!(round.output_tokens, output[step]);
        assert_eq!(
            round.input_tokens_details.unwrap().cached_tokens,
            cache[step]
        );
        assert_eq!(round.total_tokens, input[step] + output[step]);
        projected_total += round.total_tokens;
        old_double_counted_total += cumulative.total_tokens.unwrap();
    }
    assert_eq!(projected_total, 410861);
    assert_eq!(old_double_counted_total, 2606422);
    assert_eq!(cumulative.prompt_tokens, Some(406395));
    assert_eq!(cumulative.completion_tokens, Some(4466));
    assert_eq!(cumulative.cache_read_tokens, Some(354688));
}

#[test]
fn every_native_counter_uses_checked_immutable_boundary_without_mutation() {
    let before = NativeUsage {
        prompt_tokens: Some(100),
        completion_tokens: Some(20),
        total_tokens: Some(120),
        reasoning_tokens: Some(10),
        input_cache_hit_tokens: Some(70),
        input_cache_miss_tokens: Some(30),
        cache_read_tokens: Some(70),
        cache_write_tokens: Some(5),
    };
    let cumulative = NativeUsage {
        prompt_tokens: Some(240),
        completion_tokens: Some(50),
        total_tokens: Some(290),
        reasoning_tokens: Some(22),
        input_cache_hit_tokens: Some(170),
        input_cache_miss_tokens: Some(70),
        cache_read_tokens: Some(170),
        cache_write_tokens: Some(12),
    };
    let saved = serde_json::to_value(&cumulative).unwrap();
    let baseline = boundary(Some(before.clone()));
    for _ in 0..3 {
        let delta = baseline.native_round(Some(&cumulative)).unwrap();
        assert_eq!(
            serde_json::to_value(delta).unwrap(),
            json!({"prompt_tokens":140,"completion_tokens":30,"total_tokens":170,"reasoning_tokens":12,"input_cache_hit_tokens":100,"input_cache_miss_tokens":40,"cache_read_tokens":100,"cache_write_tokens":7})
        );
        let chat = baseline.project(Some(&cumulative)).unwrap();
        let responses = baseline.project_responses(Some(&cumulative)).unwrap();
        assert_eq!(responses.input_tokens, chat.prompt_tokens);
        assert_eq!(responses.output_tokens, chat.completion_tokens);
        assert_eq!(responses.total_tokens, chat.total_tokens);
        assert_eq!(responses.input_tokens_details.unwrap().cached_tokens, 100);
        assert_eq!(
            responses.output_tokens_details.unwrap().reasoning_tokens,
            12
        );
    }
    assert_eq!(serde_json::to_value(cumulative).unwrap(), saved);
    let zero = baseline.project_responses(Some(&before)).unwrap();
    assert_eq!(zero.total_tokens, 0);
    assert_eq!(zero.input_tokens_details.unwrap().cached_tokens, 0);
    assert_eq!(zero.output_tokens_details.unwrap().reasoning_tokens, 0);
}

#[test]
fn absent_corrupt_or_unmeasured_boundaries_are_unknown_not_cumulative() {
    for payload in [
        json!({}),
        json!({"native_usage_baseline_source":"usage_ledger","native_usage_baseline":"corrupt"}),
        json!({"native_usage_baseline_source":"usage_ledger","native_usage_baseline":{}}),
        json!({"native_usage_baseline_source":"usage_ledger","native_usage_baseline":{"unknown":1}}),
    ] {
        let baseline = ResponseUsageBaseline::from_callback_payload(&payload);
        assert!(baseline
            .project_responses(Some(&usage(100, 20, 30, 4)))
            .is_none());
        assert!(baseline.project(Some(&usage(100, 20, 30, 4))).is_none());
    }
    assert!(boundary(Some(usage(100, 20, 30, 4)))
        .project_responses(None)
        .is_none());
    assert_eq!(
        boundary(None)
            .project_responses(Some(&usage(100, 20, 30, 4)))
            .unwrap()
            .total_tokens,
        120
    );
    assert_eq!(
        ResponseUsageBaseline::NewRun
            .project_responses(None)
            .unwrap()
            .total_tokens,
        0
    );
}

#[test]
fn regressed_or_disappearing_counter_makes_entire_round_usage_unavailable() {
    let before = usage(100, 20, 30, 4);
    let current = usage(220, 40, 100, 10);
    for field in [
        "prompt_tokens",
        "completion_tokens",
        "total_tokens",
        "reasoning_tokens",
        "cache_read_tokens",
    ] {
        for value in [json!(0), Value::Null] {
            let mut corrupt = serde_json::to_value(&current).unwrap();
            corrupt[field] = value;
            let corrupt: NativeUsage = serde_json::from_value(corrupt).unwrap();
            assert!(
                boundary(Some(before.clone()))
                    .project_responses(Some(&corrupt))
                    .is_none(),
                "field {field}"
            );
        }
    }
}

#[test]
fn response_event_projection_never_changes_non_usage_payloads() {
    let baseline = boundary(Some(usage(100, 20, 30, 4)));
    for mut event in [
        json!({"type":"response.created","response":{"id":"resp_test","output":[]}}),
        json!({"type":"response.output_item.done","item":{"id":"msg_test","type":"message","role":"assistant","content":[]}}),
        json!({"type":"response.output_text.delta","delta":"keep"}),
    ] {
        let original = event.clone();
        baseline.project_responses_event(&mut event, Some(&usage(220, 40, 100, 10)));
        assert_eq!(event, original);
    }
    let mut event = json!({"type":"response.completed","response":{"id":"resp_test","usage":{"total_tokens":999}}});
    ResponseUsageBaseline::Unavailable
        .project_responses_event(&mut event, Some(&usage(220, 40, 100, 10)));
    assert_eq!(event["response"]["usage"], Value::Null);
}

#[test]
fn incident_round_usage_preserves_full_context_and_decreasing_output_counters() {
    // Provider calls reuse one mutable node; only the append-only ledger is cumulative.
    let calls = [(43757, 104, 43264), (46884, 152, 43648), (48036, 82, 44416)];
    let mut cumulative = usage(0, 0, 0, 0);
    let mut emitted = 0;
    for (step, (input, output, cached)) in calls.into_iter().enumerate() {
        let baseline = if step == 0 {
            ResponseUsageBaseline::NewRun
        } else {
            boundary(Some(cumulative.clone()))
        };
        cumulative.prompt_tokens = Some(cumulative.prompt_tokens.unwrap() + input);
        cumulative.completion_tokens = Some(cumulative.completion_tokens.unwrap() + output);
        cumulative.total_tokens = Some(cumulative.total_tokens.unwrap() + input + output);
        cumulative.cache_read_tokens = Some(cumulative.cache_read_tokens.unwrap() + cached);
        let response = baseline.project_responses(Some(&cumulative)).unwrap();
        assert_eq!(response.input_tokens, input);
        assert_eq!(response.output_tokens, output);
        assert_eq!(response.input_tokens_details.unwrap().cached_tokens, cached);
        emitted += response.total_tokens;
    }
    assert_eq!(emitted, cumulative.total_tokens.unwrap());
}

#[test]
fn legacy_snapshot_boundaries_are_never_subtracted_from_ledger_totals() {
    let legacy = json!({"native_usage_baseline":usage(43757,104,43264,0)});
    let baseline = ResponseUsageBaseline::from_callback_payload(&legacy);
    assert!(baseline
        .project_responses(Some(&usage(90641, 256, 86912, 0)))
        .is_none());
}
