use super::*;
use crate::openapi_interface::contract::{ApiInput, ApiOperationContext, ApiOutput};
use serde_json::json;

fn operation(id: &str) -> ApiOperationContext {
    ApiOperationContext {
        interface_id: id.into(),
        method: "POST".into(),
        path: "/api/logs/v1/events".into(),
    }
}

#[test]
fn agent_logs_document_keeps_business_batch_and_receipt_without_raw_source() {
    let mut batch = event_projection_tests::batch_without_task_end();
    batch.events = (0..45)
        .map(|index| {
            let mut event = batch.events[0].clone();
            event.event_id = format!("event-{index}");
            event.sequence = index;
            event.content = Some("complete business answer ".repeat(300));
            event
        })
        .collect();
    let input = Input(batch);
    let value = input.project_document_input(&operation(ID)).unwrap();
    assert_eq!(value["body"]["events"].as_array().unwrap().len(), 45);
    assert_eq!(
        value["body"]["events"][44]["content"],
        "complete business answer ".repeat(300)
    );
    assert!(value["body"]["events"][0].get("raw").is_none());
    assert!(!value.to_string().contains("RAW_SECRET"));
    assert!(
        jsonschema::validator_for(&Input::document_input_schema(&operation(ID)).unwrap())
            .unwrap()
            .is_valid(&value)
    );
    let output = Output(AgentLogsReceipt {
        accepted_events: 45,
        duplicate_events: 2,
        record_ids: vec![uuid::Uuid::now_v7()],
    });
    let value = output.project_document_output(&operation(ID)).unwrap();
    assert_eq!(value["accepted_events"], 45);
    assert_eq!(value["duplicate_events"], 2);
    assert!(value.get("task_status").is_none());
}
