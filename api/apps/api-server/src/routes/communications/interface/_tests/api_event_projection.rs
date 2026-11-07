use super::*;
fn operation(id: &str) -> ApiEventOperationContext {
    ApiEventOperationContext {
        interface_id: id.into(),
        method: "PUT".into(),
        path: "/api/console/communications/instances/:instance_id/binding".into(),
    }
}
#[test]
fn communication_binding_keeps_complete_command_configuration_and_real_path_body_shape() {
    let op = operation("communications.binding.update");
    let body = domain::CommunicationBindingInput {
        command_settings: (0..70)
            .map(|i| domain::CommunicationCommandSetting {
                command_id: format!("command-{i}"),
                enabled: true,
            })
            .collect(),
        event_commands: vec![domain::CommunicationEventCommandBinding {
            command_id: "command-a".into(),
            name: "Command".into(),
            description: "Description".repeat(200),
            enabled: true,
            contract_id: "contract-a".into(),
            input_mapping: std::collections::BTreeMap::from([(
                "text".into(),
                "$.message.text".into(),
            )]),
        }],
        application_id: Uuid::nil(),
        model: Some("model-a".into()),
        reasoning_effort: None,
        file_table_id: None,
        mcp_instance_ids: vec!["instance-a".into()],
        system_prompt: "Prompt".repeat(500),
    };
    let expected = event::to_value(&body).unwrap();
    let input = Input::SaveBinding(Uuid::nil(), body);
    let value = input.project_document_input(&op).unwrap();
    assert_eq!(value["path"], json!({"instance_id":Uuid::nil()}));
    assert_eq!(value["body"], expected);
    let legacy = input.project_for_event().unwrap();
    assert!(legacy["body"].get("command_settings").is_none());
    assert_eq!(legacy["operation"], "save_binding");
}
#[test]
fn communication_file_json_is_not_misclassified_as_raw_binary() {
    let op = operation("communications.files.get");
    let value = json!({"message":{"id":"message-a","text":"Description".repeat(200)},"data_base64":"YWJj".repeat(10000),"business_metadata":{"secret_label":"Public title","values":(0..90).collect::<Vec<_>>()},"url":"/api/console/files/table-a/records/record-a/content","preview_url":null});
    assert_eq!(
        Output(value.clone()).project_document_output(&op).unwrap(),
        value
    );
    assert_eq!(Output::document_output_unavailable(&op), None);
    let value = Input::File(
        Uuid::nil(),
        Uuid::nil(),
        FileQuery {
            offset: Some(10),
            max_bytes: None,
        },
    )
    .project_document_input(&op)
    .unwrap();
    assert_eq!(
        value,
        json!({"path":{"instance_id":Uuid::nil(),"message_id":Uuid::nil()},"query":{"offset":10}})
    );
}
