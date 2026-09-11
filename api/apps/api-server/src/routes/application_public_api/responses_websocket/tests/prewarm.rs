use serde_json::{json, Value};

use super::super::actor::{ConnectionAction, ConnectionState, ResponsesConnectionActor};

fn prewarm(actor: &mut ResponsesConnectionActor, input: Value) -> String {
    let action = actor
        .accept_response(json!({
            "generate": false,
            "model": "published-model",
            "input": input,
            "instructions": "prewarm-only instructions"
        }))
        .expect("valid prewarm must be accepted");
    let ConnectionAction::Prewarmed { response_id } = action else {
        panic!("prewarm must not start an inference turn");
    };
    response_id
}

// #2028 AC-001: Codex Responses Lite places tools and base instructions in the
// prewarm input, then references that response while sending only new input.
#[test]
fn prewarm_reference_preserves_lite_tools_and_instructions_before_delta() {
    let mut actor = ResponsesConnectionActor::new();
    let prefix = json!([
        {"type": "additional_tools", "role": "developer", "tools": [
            {"type": "namespace", "name": "functions", "tools": [
                {"type": "function", "name": "exec_command", "parameters": {
                    "type": "object", "properties": {"cmd": {"type": "string"}}
                }}
            ]}
        ]},
        {"type": "message", "role": "developer", "content": [
            {"type": "input_text", "text": "Use the terminal tool to inspect the repository."}
        ]}
    ]);
    let response_id = prewarm(&mut actor, prefix.clone());
    let user = json!({"type": "message", "role": "user", "content": "Inspect git status."});
    let action = actor
        .accept_response(json!({
            "previous_response_id": response_id,
            "input": [user.clone()]
        }))
        .unwrap();
    let ConnectionAction::StartTurn { response, .. } = action else {
        panic!("referenced prewarm must start a turn");
    };
    assert_eq!(response["input"], json!([prefix[0], prefix[1], user]));
    assert_eq!(response["model"], "published-model");
    assert!(response.get("previous_response_id").is_none());
    assert_eq!(actor.prewarmed_response_id(), None);
}

// #2028 AC-002: An empty delta must not erase the already-sent input.
#[test]
fn empty_or_omitted_delta_preserves_prewarm_input() {
    for delta in [Some(json!([])), None] {
        let mut actor = ResponsesConnectionActor::new();
        let prefix = json!([{"role": "user", "content": "Already supplied."}]);
        let id = prewarm(&mut actor, prefix.clone());
        let mut request = json!({"previous_response_id": id});
        if let Some(delta) = delta {
            request["input"] = delta;
        }
        let ConnectionAction::StartTurn { response, .. } = actor.accept_response(request).unwrap()
        else {
            panic!("empty delta must start the prewarmed turn");
        };
        assert_eq!(response["input"], prefix);
    }
}

// #2028 AC-002: String input is a user message, not an array to be replaced.
#[test]
fn string_inputs_are_preserved_as_ordered_user_messages() {
    let mut actor = ResponsesConnectionActor::new();
    let id = prewarm(&mut actor, json!("Earlier input"));
    let ConnectionAction::StartTurn { response, .. } = actor
        .accept_response(json!({"previous_response_id": id, "input": "New input"}))
        .unwrap()
    else {
        panic!("text delta must start the prewarmed turn");
    };
    assert_eq!(
        response["input"],
        json!([
            {"role": "user", "content": "Earlier input"},
            {"role": "user", "content": "New input"}
        ])
    );
}

// #2028 AC-002: Invalid input is rejected before it can replace valid state.
#[test]
fn invalid_prewarm_or_delta_input_does_not_consume_valid_state() {
    for invalid in [
        json!(null),
        json!(false),
        json!(42),
        json!({"text": "invalid"}),
    ] {
        let mut actor = ResponsesConnectionActor::new();
        assert!(actor
            .accept_response(json!({"generate": false, "input": invalid}))
            .is_err());
        assert_eq!(actor.state(), ConnectionState::Idle);
        let prefix = json!([{"role": "user", "content": "Keep me"}]);
        let id = prewarm(&mut actor, prefix.clone());
        assert!(actor
            .accept_response(json!({"previous_response_id": id, "input": invalid}))
            .is_err());
        assert_eq!(actor.prewarmed_response_id(), Some(id.as_str()));
        let ConnectionAction::StartTurn { response, .. } = actor
            .accept_response(json!({"previous_response_id": id, "input": []}))
            .unwrap()
        else {
            panic!("valid retry must retain its prewarm");
        };
        assert_eq!(response["input"], prefix);
    }
}

// #2028 AC-003/004: Full requests and ordinary upstream continuations never
// acquire context from an unrelated local prewarm.
#[test]
fn unreferenced_requests_do_not_inherit_prewarm_fields() {
    for previous in [None, Some("resp_real_upstream")] {
        let mut actor = ResponsesConnectionActor::new();
        prewarm(
            &mut actor,
            json!([{"role": "user", "content": "Unrelated"}]),
        );
        let mut request = json!({"model": "new-model", "input": "Full request"});
        if let Some(previous) = previous {
            request["previous_response_id"] = json!(previous);
        }
        let expected = request.clone();
        let ConnectionAction::StartTurn { response, .. } = actor.accept_response(request).unwrap()
        else {
            panic!("full request must start a turn");
        };
        assert_eq!(response, expected);
        assert_eq!(actor.prewarmed_response_id(), None);
    }
}

// #2028 AC-004: Locally synthesized references must identify one connection's
// current prewarm, not merely the first prewarm on any connection.
#[test]
fn prewarm_references_are_unique_and_reject_cross_connection_or_unknown_ids() {
    let mut first = ResponsesConnectionActor::new();
    let first_id = prewarm(&mut first, json!([]));
    let mut second = ResponsesConnectionActor::new();
    let second_id = prewarm(&mut second, json!([]));
    assert_ne!(first_id, second_id);
    for id in [first_id.as_str(), "resp_prewarm_unknown"] {
        let error = second
            .accept_response(json!({"previous_response_id": id, "input": []}))
            .unwrap_err();
        assert_eq!(error.close_code(), 1008);
        assert_eq!(second.prewarmed_response_id(), Some(second_id.as_str()));
    }
    let mut fresh = ResponsesConnectionActor::new();
    assert!(fresh
        .accept_response(json!({"previous_response_id": first_id, "input": []}))
        .is_err());
}

// #2028 AC-004: Once consumed or superseded, a prewarm cursor cannot be reused.
#[test]
fn consumed_and_superseded_prewarm_references_are_rejected() {
    let mut actor = ResponsesConnectionActor::new();
    let old_id = prewarm(&mut actor, json!([]));
    let id = prewarm(&mut actor, json!([]));
    assert_ne!(old_id, id);
    assert!(actor
        .accept_response(json!({"previous_response_id": old_id, "input": []}))
        .is_err());
    let ConnectionAction::StartTurn { turn, .. } = actor
        .accept_response(json!({"previous_response_id": id, "input": "Run"}))
        .unwrap()
    else {
        panic!("current prewarm must start a turn");
    };
    actor.complete_turn(turn);
    assert!(actor
        .accept_response(json!({"previous_response_id": id, "input": []}))
        .is_err());
}
