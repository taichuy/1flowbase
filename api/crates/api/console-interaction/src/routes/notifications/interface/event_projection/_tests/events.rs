use super::*;
#[test]
fn only_send_exposes_actual_business_payload_and_public_result() {
    let id = Uuid::now_v7();
    let input = NotificationInput::Send(
        id,
        json!({"text":"actual notification","recipient":"actual recipient"}),
    );
    let projected = input.project_for_event().unwrap();
    assert!(jsonschema::validator_for(&input_schema())
        .unwrap()
        .is_valid(&projected));
    assert_eq!(projected["input_payload"]["text"], "actual notification");
    assert!(projected.get("config_json").is_none());
    assert!(projected.get("authorization").is_none());
    assert!(NotificationInput::Get(id).project_for_event().is_none());
    let output = NotificationOutput(
        json!({"instance_id":id,"type_code":"wechat","result":{"accepted":true,"delivery_id":"actual-delivery"},"managed_secret_patch":{"token":"secret"},"authorization":{"token":"secret"}}),
    );
    let projected = output.project_for_event().unwrap();
    assert!(jsonschema::validator_for(&output_schema())
        .unwrap()
        .is_valid(&projected));
    assert_eq!(projected["result"]["delivery_id"], "actual-delivery");
    assert!(projected.get("managed_secret_patch").is_none());
    assert!(projected.get("authorization").is_none());
    assert_eq!(
        input_schema()["properties"]["input_payload"]["additionalProperties"],
        true
    );
}
