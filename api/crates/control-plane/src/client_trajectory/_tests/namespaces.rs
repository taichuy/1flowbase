use super::*;
use serde_json::Value;

const AT: &str = "2026-09-22T00:00:00Z";
fn classifier() -> classify::Classifier {
    let mut classifier = classify::Classifier::new(
        Uuid::now_v7(),
        Uuid::now_v7(),
        None,
        ClientTrajectoryTransport::Http,
    );
    classifier.begin_request(AT);
    classifier
}
fn schema(facts: &[ClientTrajectoryFact], id: Uuid) -> Option<&Value> {
    facts.iter().find_map(|fact| match fact {
        ClientTrajectoryFact::Section {
            step_id,
            section,
            value,
        } if *step_id == id && section == "schema" => Some(value),
        _ => None,
    })
}
fn steps(facts: &[ClientTrajectoryFact]) -> Vec<&crate::ports::ClientTrajectoryStep> {
    facts
        .iter()
        .filter_map(|fact| match fact {
            ClientTrajectoryFact::Step { step } => Some(step),
            _ => None,
        })
        .collect()
}
#[test]
fn exact_namespace_schemas_keep_only_top_level_rows_and_original_call_content() {
    let mut classifier = classifier();
    let a = json!({"type":"function","name":"_fetch","parameters":{"const":"github"}});
    let b = json!({"type":"custom","name":"_fetch","format":{"type":"text"}});
    let plain = json!({"type":"function","name":"_fetch","parameters":{"const":"unqualified"}});
    let roots = vec![
        json!({"type":"namespace","name":"github","description":"Actual namespace","tools":[a]}),
        json!({"type":"namespace","name":"docs","tools":[b]}),
        plain.clone(),
    ];
    let definitions = classifier.observe(
        ClientTrajectoryFrameKind::Request,
        json!({"tools":roots,"input":[]}),
        AT,
    );
    assert_eq!(
        steps(&definitions)
            .iter()
            .filter(|step| step.category == "tool_definition")
            .count(),
        3
    );
    assert!(!classifier.incomplete);
    let original = json!({"type":"function_call","id":"item-a","call_id":"a","namespace":"github","name":"_fetch","arguments":" { \"token\": \"user\" } "});
    let output = classifier.observe(ClientTrajectoryFrameKind::ResponseJson, json!({"object":"response","output":[original,
        {"type":"custom_tool_call","id":"item-b","call_id":"b","namespace":"docs","name":"_fetch","input":" raw custom input "},
        {"type":"function_call","id":"item-c","call_id":"c","name":"_fetch","arguments":"{}"},
        {"type":"function_call","id":"item-d","call_id":"d","namespace":"unknown","name":"_fetch","arguments":"{}"}
    ]}), AT);
    let calls: Vec<_> = steps(&output)
        .into_iter()
        .filter(|step| step.category == "tool_call")
        .collect();
    for (call, expected, namespace) in [
        (calls[0], &a, Some("github")),
        (calls[1], &b, Some("docs")),
        (calls[2], &plain, None),
    ] {
        assert_eq!(schema(&output, call.id), Some(expected));
        assert_eq!(call.namespace.as_deref(), namespace);
        assert!(call
            .available_sections
            .iter()
            .any(|section| section == "schema"));
    }
    assert_eq!(calls[3].namespace.as_deref(), Some("unknown"));
    assert!(schema(&output, calls[3].id).is_none());
    assert!(!calls[3]
        .available_sections
        .iter()
        .any(|section| section == "schema"));
    assert!(output.iter().any(|fact|matches!(fact,ClientTrajectoryFact::Section {step_id,section,value} if *step_id==calls[0].id && section=="overview" && *value==original)));
    let mut old = serde_json::to_value(calls[0]).unwrap();
    old.as_object_mut().unwrap().remove("namespace");
    assert!(
        serde_json::from_value::<crate::ports::ClientTrajectoryStep>(old)
            .unwrap()
            .namespace
            .is_none()
    );
}
#[test]
fn results_inherit_only_actual_related_call_namespace_and_respect_explicit_namespace() {
    let mut classifier = classifier();
    let facts=classifier.observe(ClientTrajectoryFrameKind::Request,json!({"input":[
        {"type":"function_call","call_id":"a","namespace":"github","name":"_fetch","arguments":"{}"},
        {"type":"function_call_output","call_id":"a","output":"ok"},
        {"type":"function_call_output","call_id":"a","namespace":"other","output":"other"},
        {"type":"function_call_output","call_id":"missing","output":"unknown"},
        {"type":"function_call","call_id":"old","name":"legacy","arguments":"{}"},
        {"type":"function_call_output","call_id":"old","output":"legacy"}
    ]}),AT);
    let items = steps(&facts);
    let call = items
        .iter()
        .find(|step| step.category == "tool_call" && step.call_id.as_deref() == Some("a"))
        .unwrap();
    let results: Vec<_> = items
        .iter()
        .filter(|step| step.category == "tool_result")
        .collect();
    assert_eq!(results[0].namespace.as_deref(), Some("github"));
    assert_eq!(results[0].related_step_id, Some(call.id));
    assert_eq!(results[1].namespace.as_deref(), Some("other"));
    assert!(results[1].related_step_id.is_none());
    assert!(results[2].namespace.is_none());
    assert!(results[2].related_step_id.is_none());
    assert!(results[3].namespace.is_none());
    assert!(results[3].related_step_id.is_some());
    for result in results {
        assert_eq!(result.origin, "submitted");
    }
}
#[test]
fn nested_schema_index_is_bounded_without_emitting_leaf_rows() {
    let mut classifier = classifier();
    let leaves:Vec<_>=(0..1025).map(|n|json!({"type":"function","name":format!("tool-{n}"),"parameters":{"type":"object"}})).collect();
    let facts = classifier.observe(
        ClientTrajectoryFrameKind::Request,
        json!({"tools":[{"type":"namespace","name":"bulk","tools":leaves}]}),
        AT,
    );
    assert!(classifier.incomplete);
    assert_eq!(
        steps(&facts)
            .iter()
            .filter(|step| step.category == "tool_definition")
            .count(),
        1
    );
    let calls=classifier.observe(ClientTrajectoryFrameKind::ResponseJson,json!({"object":"response","output":[
        {"type":"function_call","id":"within","namespace":"bulk","name":"tool-1023","arguments":"{}"},
        {"type":"function_call","id":"beyond","namespace":"bulk","name":"tool-1024","arguments":"{}"}
    ]}),AT);
    let calls_steps = steps(&calls);
    assert!(schema(&calls, calls_steps[0].id).is_some());
    assert!(schema(&calls, calls_steps[1].id).is_none());
    let mut deep = json!({"type":"function","name":"leaf","parameters":{}});
    for n in 0..9 {
        deep = json!({"type":"namespace","name":format!("level-{n}"),"tools":[deep]});
    }
    let mut exact = super::super::schemas::SchemaIndex::default();
    exact.insert_root(&json!({"type":"namespace","name":"outer","tools":[{"type":"namespace","name":"inner","tools":[{"type":"function","name":"leaf","parameters":{}}]}]}));
    assert!(exact.get(Some("inner"), "leaf").is_some());
    assert!(exact.get(None, "leaf").is_none());
    assert!(exact.get(Some("outer.inner"), "leaf").is_none());
    assert!(!exact.incomplete);
    let mut index = super::super::schemas::SchemaIndex::default();
    index.insert_root(&deep);
    assert!(index.incomplete);
    assert!(index.get(Some("level-0"), "leaf").is_none());
    let mut index = super::super::schemas::SchemaIndex::default();
    index.insert_root(&json!({"type":"namespace","name":"large","tools":[{"type":"function","name":"leaf","description":"x".repeat(512*1024)}]}));
    assert!(index.incomplete);
    assert!(index.get(Some("large"), "leaf").is_none());
}
