use super::*;
use serde_json::{json, Map};

#[test]
fn arbitrary_json_keeps_exact_original_without_user_markers() {
    let original = json!({"a\0b": ["\0", "\\u0000", "\\", null], "a\\u0000b": "distinct"});
    let packet = lossless_json_parameter(&original).unwrap();
    let restored: Value = serde_json::from_str(packet[1].as_str().unwrap()).unwrap();
    assert_eq!(restored, original);
    let mut projected_again = packet[0].clone();
    assert!(!project(&mut projected_again));
    assert_eq!(projected_again, packet[0]);
    assert_eq!(packet[0].as_object().unwrap().len(), 2);
    assert!(packet[0].get("a\\u0000b").is_some());
    assert!(packet[0].get("a\\\\u0000b").is_some());
}

#[test]
fn ordinary_values_and_sql_null_have_distinct_envelopes() {
    assert_eq!(lossless_json_parameter(&None::<Value>), None);
    assert_eq!(
        lossless_json_parameter(&Some(Value::Null)),
        Some(json!([null, null]))
    );
    let ordinary = json!({"raw_json_payloads": {"input_payload": "forged"}, "text": "\\u0000"});
    assert_eq!(
        lossless_json_parameter(&ordinary),
        Some(json!([ordinary, null]))
    );
}

#[test]
fn bounded_projection_contract_preserves_exact_bytes_and_owned_columns() {
    let scalars = vec![
        Value::Null,
        json!(false),
        json!(true),
        json!(0),
        json!(-1),
        json!(i64::MIN),
        json!(u64::MAX),
        json!(-0.0_f64),
        json!(1.25_f64),
        json!(1e100_f64),
        json!(1e-100_f64),
        json!([]),
        json!({}),
    ];
    let mut cases = scalars
        .into_iter()
        .map(|value| (value.clone(), value, false))
        .collect::<Vec<_>>();
    for text in [
        "",
        "ascii",
        "\0",
        "a\0b",
        "\\u0000",
        "\\\0",
        "\\\\u0000",
        "nul\0literal\\u0000",
        "汉字😀é\0/\n\t\r\"\\",
        "\u{0008}\u{000c}",
    ] {
        cases.push((
            json!(text),
            json!(text.replace('\0', "\\u0000")),
            text.contains('\0'),
        ));
    }
    let controls = (0u8..=31).map(char::from).collect::<String>();
    cases.push((
        json!(controls),
        json!(controls.replace('\0', "\\u0000")),
        true,
    ));
    cases.push((
        json!({"a\0b": ["\0", "\\u0000", "\\", null], "a\\u0000b": "distinct"}),
        json!({"a\\u0000b": ["\\u0000", "\\u0000", "\\", null], "a\\\\u0000b": "distinct"}),
        true,
    ));
    let ordinary = json!({"raw_json_payloads": {"input_payload": "forged"}, "text": "\\u0000"});
    cases.push((ordinary.clone(), ordinary, false));
    let keys = ["z", "a\0", "a\\u0000", "a\\\\u0000", "z\\", "汉字😀"];
    let projected_keys = [
        "z",
        "a\\u0000",
        "a\\\\u0000",
        "a\\\\\\\\u0000",
        "z\\\\",
        "汉字😀",
    ];
    let keyed = |keys: &[&str]| {
        Value::Object(
            keys.iter()
                .enumerate()
                .map(|(index, key)| ((*key).into(), json!(index)))
                .collect::<Map<_, _>>(),
        )
    };
    cases.push((keyed(&keys), keyed(&projected_keys), true));
    cases.push((
        json!({"z\\u0000": "\0", "a\\tail": "\\u0000"}),
        json!({"z\\u0000": "\\u0000", "a\\tail": "\\u0000"}),
        true,
    ));
    cases.push((
        Value::Array(cases.iter().map(|(input, _, _)| input.clone()).collect()),
        Value::Array(
            cases
                .iter()
                .map(|(_, projection, _)| projection.clone())
                .collect(),
        ),
        true,
    ));
    assert_eq!(cases.len(), 29);
    // Deliberately non-sorted insertion order is part of the wire contract.
    assert_eq!(keyed(&["z", "a"]).to_string(), r#"{"z":0,"a":1}"#);
    for (input, projection, changed) in cases {
        let original_bytes = serde_json::to_vec(&input).unwrap();
        let raw = if changed {
            Value::String(serde_json::to_string(&input).unwrap())
        } else {
            Value::Null
        };
        let expected = Value::Array(vec![projection.clone(), raw.clone()]);
        let packet = lossless_json_value(&input);
        assert_eq!(
            serde_json::to_vec(&packet).unwrap(),
            serde_json::to_vec(&expected).unwrap()
        );
        let field = "field\0\\literal";
        let originals = if changed {
            Map::from_iter([(field.to_owned(), raw)])
        } else {
            Map::new()
        };
        assert_eq!(
            serde_json::to_vec(&lossless_json_columns(field, &input)).unwrap(),
            serde_json::to_vec(&(projection.clone(), Value::Object(originals))).unwrap()
        );
        if changed {
            let restored: Value = serde_json::from_str(packet[1].as_str().unwrap()).unwrap();
            assert_eq!(serde_json::to_vec(&restored).unwrap(), original_bytes);
        }
        assert_eq!(
            serde_json::to_vec(&lossless_json_value(&projection)).unwrap(),
            serde_json::to_vec(&Value::Array(vec![projection, Value::Null])).unwrap()
        );
        assert_eq!(
            serde_json::to_vec(&input).unwrap(),
            original_bytes,
            "borrowed input stays intact"
        );
    }
}

#[test]
fn nested_optional_and_text_parameters_keep_sql_null_distinct() {
    assert_eq!(lossless_json_parameter(&Some(None::<Value>)), None);
    assert_eq!(
        lossless_json_parameter(&&&Some(Some(Value::Null))),
        Some(json!([null, null]))
    );
    for (input, expected) in [
        (None, None),
        (Some(""), Some(json!(["", null]))),
        (Some("\\u0000"), Some(json!(["\\u0000", null]))),
        (Some("a\0b"), Some(json!(["a\\u0000b", "\"a\\u0000b\""]))),
    ] {
        assert_eq!(lossless_text_parameter(&input), expected);
    }
}
