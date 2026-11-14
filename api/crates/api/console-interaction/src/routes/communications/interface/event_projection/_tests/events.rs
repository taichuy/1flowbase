use super::*;
#[test]
fn communication_message_values_keep_text_identifiers_and_file_metadata_without_download_reference()
{
    let value = Output(
        json!({"id":"message-1","conversation_id":"conversation-1","instance_id":"instance-1","content":"actual message text","type":"file","file":{
            "file_name":"actual.txt","media_type":"text/plain","status":"ready","size_bytes":12,"file_table_id":"table-1","file_record_id":"record-1","failure_code":null,
            "access_path":"private-download","download_reference":{"token":"secret"},
        },"authorization":{"token":"secret"}}),
    );
    let projected = value.project_for_event().unwrap();
    assert!(jsonschema::validator_for(&output_schema())
        .unwrap()
        .is_valid(&projected));
    assert_eq!(projected["content"], "actual message text");
    assert_eq!(projected["file"]["file_name"], "actual.txt");
    assert!(projected["file"].get("access_path").is_none());
    assert!(projected.get("authorization").is_none());
    assert!(Input::File(Uuid::nil(), Uuid::nil(), FileQuery::default())
        .project_for_event()
        .is_none());
    assert_eq!(
        output_schema()["properties"]["content"]["anyOf"][0]["type"],
        "string"
    );
}
#[test]
fn communication_binding_input_and_result_select_business_fields_explicitly() {
    let body: domain::CommunicationBindingInput = serde_json::from_value(json!({
            "application_id":Uuid::now_v7(),"model":"real-model","reasoning_effort":null,"file_table_id":null,"mcp_instance_ids":["actual-mcp"],"system_prompt":"actual prompt",
        })).unwrap();
    let input = Input::SaveBinding(Uuid::now_v7(), body);
    assert!(jsonschema::validator_for(&input_schema())
        .unwrap()
        .is_valid(&input.project_for_event().unwrap()));
    assert_eq!(
        input.project_for_event().unwrap()["body"]["model"],
        "real-model"
    );
    let output = Output(
        json!({"binding":{"application_id":"app-1","system_prompt":"actual prompt","model":"real-model","config_json":{"secret":"hidden"},"authorization":{"token":"hidden"}},"commands":[]}),
    );
    let projected = output.project_for_event().unwrap();
    assert_eq!(projected["binding"]["system_prompt"], "actual prompt");
    assert!(projected["binding"].get("config_json").is_none());
    assert!(projected["binding"].get("authorization").is_none());
}
