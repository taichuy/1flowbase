use super::*;
use serde_json::json;

#[test]
fn explicit_aliases_share_one_body_and_serialize_the_original_plain_object() {
    let request = json!({"arguments":"args", "nested":[1,true,null]});
    let result = json!({"content":"result", "result_context_usage":{"tokens":7}});
    let expected = json!({
        "id":"call", "optional":null,
        "request_payload":request,"tool_call":request,
        "callback_payload":result,"parsed_result":result,"tool_result":result,
    });
    let mut payload = TraceProjectionWritePayload::from(json!({"id":"call","optional":null}));
    payload
        .set_shared_fields(&["request_payload", "tool_call"], request)
        .unwrap();
    payload
        .set_shared_fields(
            &["callback_payload", "parsed_result", "tool_result"],
            result,
        )
        .unwrap();
    assert!(std::ptr::eq(
        payload.get("request_payload").unwrap(),
        payload.get("tool_call").unwrap()
    ));
    assert!(std::ptr::eq(
        payload.get("callback_payload").unwrap(),
        payload.get("parsed_result").unwrap()
    ));
    assert!(std::ptr::eq(
        payload.get("parsed_result").unwrap(),
        payload.get("tool_result").unwrap()
    ));
    assert_eq!(serde_json::to_value(&payload).unwrap(), expected);
    assert_eq!(payload.materialize_for_persistence().as_ref(), &expected);
    assert_eq!(payload, TraceProjectionWritePayload::from(expected));
    assert!(payload.get("missing").is_none());
    assert!(payload.get("optional").unwrap().is_null());
    assert!(payload["missing"].is_null());
}

#[test]
fn mutation_is_copy_on_write_per_member_and_per_projection_clone() {
    let mut payload = TraceProjectionWritePayload::from(json!({"id":"call"}));
    payload
        .set_shared_fields(
            &["request_payload", "tool_call"],
            json!({"arguments":"old"}),
        )
        .unwrap();
    payload
        .set_shared_fields(
            &["callback_payload", "parsed_result", "tool_result"],
            json!({"content":"truth"}),
        )
        .unwrap();
    let copy = payload.clone();
    assert!(std::ptr::eq(
        payload.get("request_payload").unwrap(),
        copy.get("request_payload").unwrap()
    ));
    payload["request_payload"]["arguments"] = json!("changed");
    payload["callback_payload"] = json!({"content":"changed callback only"});
    assert_eq!(payload["tool_call"]["arguments"], json!("old"));
    assert_eq!(copy["request_payload"]["arguments"], json!("old"));
    assert_eq!(payload["parsed_result"]["content"], json!("truth"));
    assert_eq!(payload["tool_result"]["content"], json!("truth"));
    assert_eq!(copy["callback_payload"]["content"], json!("truth"));
    payload["new"]["nested"] = json!(1);
    assert!(copy.get("new").is_none());
}

#[test]
fn distinct_callback_fields_are_not_inferred_to_be_aliases() {
    let mut payload = TraceProjectionWritePayload::from(json!({
        "callback_payload":{"content":"callback"},
        "parsed_result":{"content":"parsed"},
        "tool_result":{"content":"tool"},
    }));
    payload
        .set_shared_fields(
            &["request_payload", "tool_call"],
            json!({"arguments":"request"}),
        )
        .unwrap();
    assert_eq!(payload["callback_payload"]["content"], json!("callback"));
    assert_eq!(payload["parsed_result"]["content"], json!("parsed"));
    assert_eq!(payload["tool_result"]["content"], json!("tool"));
    payload["parsed_result"]["content"] = json!("updated parsed");
    assert_eq!(payload["callback_payload"]["content"], json!("callback"));
    assert_eq!(payload["tool_result"]["content"], json!("tool"));
}

#[test]
fn ordinary_json_is_borrowed_and_shared_json_remains_lossless() {
    for value in [
        json!(null),
        json!(false),
        json!([null, 1, 1.25, "nul\0value"]),
        json!({"nul\0key":"before\0after","nul\\u0000key":"distinct","integer":u64::MAX}),
    ] {
        let payload = TraceProjectionWritePayload::from(value.clone());
        assert!(matches!(
            payload.materialize_for_persistence(),
            Cow::Borrowed(_)
        ));
        assert_eq!(serde_json::to_value(&payload).unwrap(), value);
    }
    let mut payload = TraceProjectionWritePayload::from(
        json!({"nul\0key":"nul\0value", "nul\\u0000key":"literal"}),
    );
    payload
        .set_shared_fields(
            &["request_payload", "tool_call"],
            json!([u64::MAX, 1.25, null, "\\u0000", "\0"]),
        )
        .unwrap();
    let plain = payload.materialize_for_persistence();
    assert!(matches!(plain, Cow::Owned(_)));
    assert_eq!(
        serde_json::to_value(&payload).unwrap(),
        plain.as_ref().clone()
    );
}

#[test]
fn invalid_shared_updates_return_an_error_without_erasing_the_original_json() {
    for original in [
        json!(true),
        json!(42),
        json!(i64::MIN),
        json!(u64::MAX),
        json!(1.25),
        json!("body\0text"),
        json!([null, {"nested": true}]),
    ] {
        let mut payload = TraceProjectionWritePayload::from(original.clone());
        let before = payload.clone();
        let error = payload
            .set_shared_fields(&["request_payload", "tool_call"], json!({"body":"new"}))
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "shared trace fields require an object or null payload"
        );
        assert_eq!(payload, before);
        assert_eq!(serde_json::to_value(&payload).unwrap(), original);
        assert_eq!(payload.materialize_for_persistence().as_ref(), &original);
        assert!(matches!(
            payload.materialize_for_persistence(),
            Cow::Borrowed(_)
        ));
    }
}

#[test]
fn null_remains_an_explicit_empty_object_seed_for_shared_fields() {
    let mut payload = TraceProjectionWritePayload::from(Value::Null);
    payload
        .set_shared_fields(&["first", "alias"], json!([null, true]))
        .unwrap();
    assert!(std::ptr::eq(
        payload.get("first").unwrap(),
        payload.get("alias").unwrap()
    ));
    assert_eq!(
        serde_json::to_value(&payload).unwrap(),
        json!({"first":[null,true],"alias":[null,true]})
    );
}

#[test]
fn typed_shared_object_preserves_fields_and_accepts_arbitrary_alias_bodies() {
    let mut object = TraceProjectionSharedObject::from_fields([
        ("retained", json!({"nested":[null, u64::MAX, "nul\0value"]})),
        ("separate", json!("untouched")),
    ]);
    object.set_shared_fields(&["first", "alias"], json!(false));
    let mut payload = TraceProjectionWritePayload::from(object);
    let copy = payload.clone();
    assert!(std::ptr::eq(
        payload.get("first").unwrap(),
        payload.get("alias").unwrap()
    ));
    payload["first"] = json!("changed");
    assert_eq!(payload["alias"], json!(false));
    assert_eq!(copy["first"], json!(false));
    assert_eq!(
        serde_json::to_value(&payload).unwrap(),
        json!({
            "retained":{"nested":[null,u64::MAX,"nul\0value"]},
            "separate":"untouched", "first":"changed", "alias":false,
        })
    );
}

#[test]
fn equal_fields_are_not_shared_without_an_explicit_alias_declaration() {
    let body = json!({"nested":[null, true, "body"]});
    let object =
        TraceProjectionSharedObject::from_fields([("first", body.clone()), ("second", body)]);
    let mut payload = TraceProjectionWritePayload::from(object);
    assert!(!std::ptr::eq(
        payload.get("first").unwrap(),
        payload.get("second").unwrap()
    ));
    payload
        .set_shared_fields(&["request_payload", "tool_call"], json!(null))
        .unwrap();
    assert!(!std::ptr::eq(
        payload.get("first").unwrap(),
        payload.get("second").unwrap()
    ));
    payload["first"]["nested"][0] = json!("changed");
    assert_eq!(payload["second"]["nested"][0], Value::Null);
}
