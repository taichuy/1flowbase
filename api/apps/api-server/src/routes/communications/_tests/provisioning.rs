use super::*;

#[test]
fn communication_generated_mapping_obeys_call_controls_and_preserves_channel_binding() {
    let channel = Uuid::now_v7();
    let parameters =
        ["instance_id", "conversation_id"].map(|name| domain::McpParameterDescriptor {
            name: name.into(),
            field_type: "string".into(),
            parameter_type: domain::McpParameterType::Url,
            description: None,
            required: true,
            schema: json!({"type":"string"}),
        });
    let mapping = channel_input_mapping(&parameters, &json!({}), channel);
    domain::mcp_management::validate_mcp_call_parameters(&mapping).unwrap();
    domain::mcp_management::input_defaults::validate_input_defaults(&mapping, &parameters).unwrap();
    assert!(domain::mcp_management::mcp_des_id_required(&mapping));
    assert!(domain::mcp_management::mcp_call_parameter_is_open(
        &mapping, "des_id"
    ));
    for closed in ["max_inline_chars", "response_fields"] {
        assert!(!domain::mcp_management::mcp_call_parameter_is_open(
            &mapping, closed
        ));
    }
    let binding: domain::mcp_management::input_defaults::McpInputDefaults =
        serde_json::from_value(mapping["mappings"][0].clone()).unwrap();
    assert!(binding.hidden);
    assert_eq!(
        binding.resolve(Some(&json!(Uuid::now_v7()))),
        Some(&json!(channel))
    );
    // Reproduce the rejected pre-fix construction: a call mapping without its declarations.
    let mut undeclared = mapping;
    undeclared["interface_parameters"]
        .as_array_mut()
        .unwrap()
        .retain(|parameter| parameter.pointer("/source/kind") != Some(&json!("mcp_call")));
    assert!(domain::mcp_management::validate_mcp_call_parameters(&undeclared).is_err());
}
