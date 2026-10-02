use super::*;
use crate::application_public_api::compat::openai::{
    translate_chat_completion_request, translate_response_request,
};

#[test]
fn translated_chat_and_semantic_responses_mount_tools_independent_of_inputs_selector() {
    let native_tools: Vec<_> = (0..24).map(|index| json!({"name":format!("tool_{index}"),"description":"client tool","input_schema":{"type":"object","properties":{"query":{"type":"string"}}},"source":"client"})).collect();
    let chat_tools: Vec<_> = native_tools.iter().map(|tool|json!({"type":"function","function":{"name":tool["name"],"description":tool["description"],"parameters":tool["input_schema"]}})).collect();
    let responses_tools: Vec<_> = native_tools.iter().map(|tool|json!({"type":"function","name":tool["name"],"description":tool["description"],"parameters":tool["input_schema"]})).collect();
    let chat = translate_chat_completion_request(json!({"model":"deepseek-flash","messages":[{"role":"user","content":"Use a tool"}],"tools":chat_tools,"tool_choice":"required"})).unwrap().request;
    let responses = translate_response_request(json!({"model":"deepseek-flash","input":"Use a tool","tools":responses_tools,"tool_choice":"required"})).unwrap().request;
    let native: NativeRunRequest = serde_json::from_value(json!({"query":"Use a tool","inputs":{"tools":native_tools,"tool_choice":{"type":"required"}}})).unwrap();
    for request in [chat, responses, native] {
        for selector in [
            Some("node-start.inputs"),
            Some("sys.inputs"),
            Some("custom.business"),
            None,
        ] {
            let mut mapping = ApplicationApiMappingConfig::default_native();
            mapping.input.inputs_target = selector.map(str::to_owned);
            let mapped = NativeInputMapper::map(&request, &mapping)
                .unwrap()
                .node_input_payload;
            let context = &mapped[NATIVE_MODEL_TOOL_CONTEXT_PAYLOAD_KEY];
            assert_eq!(context["tools"], json!(native_tools));
            assert_eq!(context["tool_choice"], json!({"type":"required"}));
            if let Some(selector) = selector {
                let path = format!("/{}", selector.replace('.', "/"));
                let business = mapped.pointer(&path).unwrap();
                assert_eq!(business["tools"], context["tools"]);
                assert_eq!(business["tool_choice"], context["tool_choice"]);
            } else {
                assert_eq!(mapped["node-start"]["tools"], context["tools"]);
                assert_eq!(mapped["node-start"]["tool_choice"], context["tool_choice"]);
            }
        }
    }
}

#[test]
fn absent_tools_do_not_mount_model_context_and_empty_tools_stay_exact() {
    for inputs in [json!({}), json!({"tools":[]})] {
        let request: NativeRunRequest =
            serde_json::from_value(json!({"query":"hello","inputs":inputs})).unwrap();
        let mapped =
            NativeInputMapper::map(&request, &ApplicationApiMappingConfig::default_native())
                .unwrap()
                .node_input_payload;
        if inputs.get("tools").is_some() {
            assert_eq!(
                mapped[NATIVE_MODEL_TOOL_CONTEXT_PAYLOAD_KEY]["tools"],
                inputs["tools"]
            );
        } else {
            assert!(mapped.get(NATIVE_MODEL_TOOL_CONTEXT_PAYLOAD_KEY).is_none());
        }
        assert!(mapped[NATIVE_MODEL_TOOL_CONTEXT_PAYLOAD_KEY]
            .get("tool_choice")
            .is_none());
    }
}

#[test]
fn canonical_tool_mount_preserves_valid_tools_business_namespace() {
    let request: NativeRunRequest = serde_json::from_value(json!({"query":"hello","inputs":{"tools":[{"name":"lookup","input_schema":{"type":"object"}}]}})).unwrap();
    let mut mapping = ApplicationApiMappingConfig::default_native();
    mapping.input.query_target = "tools.query".into();
    let mapped = NativeInputMapper::map(&request, &mapping)
        .unwrap()
        .node_input_payload;
    assert_eq!(mapped["tools"]["query"], "hello");
    assert_eq!(
        mapped[NATIVE_MODEL_TOOL_CONTEXT_PAYLOAD_KEY]["tools"],
        request.inputs.get("tools").unwrap().clone()
    );
}
