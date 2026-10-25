use super::*;
use serde_json::json;

#[test]
fn committed_event_index_preserves_exact_json_and_event_type_identity() {
    let mut index = ForwardedCommittedEvents::default();
    let original = json!({"call_id": "a", "value": {"x": 1, "y": 2}});
    let reordered: Value =
        serde_json::from_str(r#"{"value":{"y":2,"x":1},"call_id":"a"}"#).unwrap();
    index.insert("tool_done".into(), original.clone());
    assert!(index.contains("tool_done", &reordered));
    assert!(!index.contains("tool_error", &original));
    assert!(!index.contains("tool_done", &json!({"call_id":"b","value":{"x":1,"y":2}})));
    assert!(!index.contains("tool_done", &json!({"call_id":"a","value":{"x":1,"y":"2"}})));
    assert!(!ForwardedCommittedEvents::default().contains("tool_done", &original));
}

#[test]
fn committed_event_index_matches_ordered_reference_for_all_bounded_traces() {
    let alphabet = [
        ("done", json!({"a": 1, "b": 2})),
        ("done", json!({"b": 2, "a": 1})),
        ("error", json!({"a": 1, "b": 2})),
        ("done", json!([1, "2", null])),
        ("done", json!(null)),
        ("done", json!({"a": 2, "b": 2})),
    ];
    let mut checked = 0;
    for length in 0..=5 {
        for mut code in 0usize..6usize.pow(length) {
            let mut reference = Vec::<(&str, Value)>::new();
            let mut index = ForwardedCommittedEvents::default();
            for _ in 0..length {
                let (kind, payload) = &alphabet[code % alphabet.len()];
                code /= alphabet.len();
                let found = reference.iter().any(|(k, p)| k == kind && p == payload);
                assert_eq!(index.contains(kind, payload), found);
                if !found {
                    reference.push((kind, payload.clone()));
                    index.insert((*kind).to_string(), payload.clone());
                }
                for (query_kind, query_payload) in &alphabet {
                    assert_eq!(
                        index.contains(query_kind, query_payload),
                        reference
                            .iter()
                            .any(|(k, p)| k == query_kind && p == query_payload)
                    );
                }
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 9_331);
}
