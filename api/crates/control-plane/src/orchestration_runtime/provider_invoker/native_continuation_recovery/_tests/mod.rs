use super::*;

fn sealed() -> (ProviderTransportPayload, ProviderInvocationInput) {
    let prefix = json!([
        {"type":"message","role":"user","content":[{"type":"input_text","text":"question"}]},
        {"type":"reasoning","id":"rs_opaque","encrypted_content":"sealed==","future":{"exact":[1,2]}},
        {"type":"custom_tool_call","id":"ct_1","call_id":"call_1","name":"shell","input":"echo exact"}
    ]);
    let proof = completed_history(&json!({"input":prefix}), None, &[])
        .unwrap()
        .unwrap();
    let continuation = crate::ports::ProviderContinuation::new(
        "upstream_old",
        crate::ports::ProviderTransportAffinity::new("provider", "openai", "responses", "model"),
    )
    .unwrap()
    .with_native_history(Some(proof.clone()));
    let payload = ProviderTransportPayload::openai_responses(json!({"model":"model",
        "previous_response_id":"resp_host", "input":[{"type":"custom_tool_call_output","call_id":"call_1","output":"exact output"}],
        "tools":[{"type":"custom","name":"shell","format":{"type":"text"}}],"future":{"keep":true}}))
        .unwrap().with_native_recovery_source(Source { flow_run_id:Uuid::now_v7(),response_round_id:Uuid::now_v7() })
        .bind_openai_continuation(continuation).unwrap();
    let mut items = prefix.as_array().unwrap().clone();
    items.extend(native_input(payload.wire_body()).unwrap());
    let mut wire = payload.wire_body().clone();
    wire.as_object_mut().unwrap().remove("previous_response_id");
    wire["input"] = json!(items);
    let rebuilt = ProviderTransportPayload::openai_responses(wire).unwrap();
    let mut input = ProviderInvocationInput::default();
    input.native_transport = Some(
        plugin_framework::provider_contract::ProviderNativeTransport {
            protocol: "openai_responses".into(),
            wire_body: rebuilt.wire_body().clone(),
            digest: rebuilt.digest().into(),
            size_bytes: rebuilt.size_bytes() as u64,
        },
    );
    input.run_context.insert(CARRIER.into(),json!({"original_digest":payload.digest(),"source":payload.native_recovery_source(),"history":proof,"rebuilt_digest":rebuilt.digest()}));
    (payload, input)
}

#[test]
fn sealed_rebuild_requires_exact_prefix_delta_configuration_source_and_digest() {
    let (payload, input) = sealed();
    assert!(validate_rebuild(&payload, &input).is_ok());
    for mutation in [
        "prefix",
        "delta",
        "configuration",
        "digest",
        "source",
        "proof",
        "cursor",
    ] {
        let mut wrong = input.clone();
        let native = wrong.native_transport.as_mut().unwrap();
        match mutation {
            "prefix" => native.wire_body["input"][1]["encrypted_content"] = json!("changed"),
            "delta" => native.wire_body["input"][3]["output"] = json!("changed"),
            "configuration" => native.wire_body["future"] = json!({"keep":false}),
            "digest" => {
                wrong.run_context.get_mut(CARRIER).unwrap()["original_digest"] = json!("wrong")
            }
            "source" => {
                wrong.run_context.get_mut(CARRIER).unwrap()["source"]["flow_run_id"] =
                    json!(Uuid::now_v7())
            }
            "proof" => {
                wrong.run_context.get_mut(CARRIER).unwrap()["history"]["item_count"] = json!(0)
            }
            "cursor" => native.wire_body["previous_response_id"] = json!("reintroduced"),
            _ => unreachable!(),
        }
        // Recompute mutable body claims: digest strings alone must never authorize changes.
        if matches!(mutation, "prefix" | "delta" | "configuration" | "cursor") {
            let native = wrong.native_transport.as_mut().unwrap();
            let rebuilt =
                ProviderTransportPayload::openai_responses(native.wire_body.clone()).unwrap();
            native.digest = rebuilt.digest().into();
            native.size_bytes = rebuilt.size_bytes() as u64;
            wrong.run_context.get_mut(CARRIER).unwrap()["rebuilt_digest"] = json!(rebuilt.digest());
        }
        assert!(validate_rebuild(&payload, &wrong).is_err(), "{mutation}");
    }
}

#[test]
fn only_verified_rebuild_clears_all_cursor_carriers_and_string_input_is_normalized() {
    let (payload, mut input) = sealed();
    input.previous_response_id = Some("old".into());
    assert!(apply_verified_rebuild(Some(&payload), &mut input).unwrap());
    assert!(input.previous_response_id.is_none());
    assert!(is_verified_rebuild(Some(&payload), &input));
    assert_eq!(
        native_input(&json!({"input":"hello"})).unwrap(),
        vec![
            json!({"type":"message","role":"user","content":[{"type":"input_text","text":"hello"}]})
        ]
    );
    assert!(native_input(&json!({"input":[{"type":"item_reference","id":"missing"}]})).is_err());
    assert!(native_input(&json!({"input":[],"conversation":"external"})).is_err());
    let mut unverified = input;
    unverified.run_context.get_mut(CARRIER).unwrap()["original_digest"] = json!("wrong");
    unverified.previous_response_id = Some("retain".into());
    assert!(apply_verified_rebuild(Some(&payload), &mut unverified).is_err());
    assert_eq!(unverified.previous_response_id.as_deref(), Some("retain"));
}
