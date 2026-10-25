//! Actual process fixture using the SDK and existing capability wire envelope.
use runtime_extension_sdk::{
    execute_communication, execute_notification, CapabilityStdioRequest, NotificationOperation,
    NotificationRuntimeResult, NotificationTypeDescriptor,
};
use serde_json::{json, Value};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let request: CapabilityStdioRequest = serde_json::from_reader(std::io::stdin().lock())?;
    let manifest: Value = serde_json::from_str(include_str!(
        "../../../../plugins/fixtures/acme.notification/manifest.yaml"
    ))?;
    let mut descriptor: NotificationTypeDescriptor =
        serde_json::from_value(manifest["notification_types"][0].clone())?;
    if request.input["input_payload"].get("operation").is_some() {
        descriptor.receive = Some(serde_json::from_str(include_str!(
            "../../../../plugins/fixtures/acme.notification/receive.json"
        ))?);
        let response = execute_communication(request, &descriptor, |_config, input| async move {
            let value = match input.operation {
                NotificationOperation::Receive { cursor, .. } => json!({
                    "type":"receive", "outcome":{"status":"succeeded","batch":{
                        "messages":if cursor.is_some() {json!([])} else {json!([
                            {"provider_message_id":"alice-input","conversation_id":"external-room","sender":{"id":"alice"},"received_at_ms":42,"text":"Alice private input"},
                            {"provider_message_id":"bob-input","conversation_id":"external-room","sender":{"id":"bob"},"received_at_ms":43,"text":"Bob private input"}
                        ])}, "candidate_cursor":"accepted-cursor", "has_more":false
                    }}
                }),
                NotificationOperation::Send { input_payload, target } => {
                    let target = target.ok_or("target required")?;
                    let expected_message = match target.sender_id.as_str() {
                        "alice" => "alice-input", "bob" => "bob-input", _ => return Err("unknown sender")
                    };
                    if target.conversation_id != "external-room" || target.provider_message_id != expected_message
                        || input_payload["text"] != format!("Final for {}", target.sender_id) {
                        return Err("send must use exact durable target and final answer");
                    }
                    json!({"type":"send","outcome":{"status":"succeeded","result":{
                        "accepted":true,"delivery_id":format!("{}:{}:{}:{}", target.sender_id, target.conversation_id, target.provider_message_id, input_payload["text"].as_str().unwrap_or_default())
                    }}})
                }
                _ => return Err("unsupported communication fixture operation"),
            };
            Ok::<NotificationRuntimeResult, &str>(serde_json::from_value(value).unwrap())
        }).await;
        serde_json::to_writer(std::io::stdout().lock(), &response)?;
        return Ok(());
    }
    let response = if request.input["input_payload"]["text"] == "fixture-invalid-output" {
        // Deliberately bypass author-side validation to prove the Host rejects a bad peer.
        json!({"ok":true,"result":{"accepted":"yes","delivery_id":"invalid"}})
    } else {
        serde_json::to_value(execute_notification(
            request,
            &descriptor,
            |config, input| -> Result<Value, String> {
                if input["text"] == "fixture-provider-error" {
                    return Err(format!("failed with token {}", config["token"]));
                }
                let delivery_id = if input["text"] == "fixture-secret-output" {
                    config["token"].as_str().unwrap().to_owned()
                } else {
                    format!(
                        "{}:{}",
                        config["channel"].as_str().unwrap(),
                        input["text"].as_str().unwrap()
                    )
                };
                Ok(json!({"accepted":true,"delivery_id":delivery_id}))
            },
        ))?
    };
    serde_json::to_writer(std::io::stdout().lock(), &response)?;
    Ok(())
}
