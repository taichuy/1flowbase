//! Deterministic authorizable supplier fixture on the shared SDK multiplex dispatcher.
//! No provider communication or persistence; Host tests supply/recover encrypted patches.
use runtime_extension_sdk::{
    execute_authorizable_notification, serve, CapabilityStdioRequest,
    NotificationAuthorizationOperation, NotificationOperation, NotificationRuntimeResult,
    NotificationTypeDescriptor,
};
use serde_json::{json, Value};

fn descriptor() -> NotificationTypeDescriptor {
    let manifest: Value = serde_json::from_str(include_str!(
        "../../../../../plugins/fixtures/acme.notification/manifest.yaml"
    ))
    .expect("fixture manifest");
    let mut value: NotificationTypeDescriptor =
        serde_json::from_value(manifest["notification_types"][0].clone())
            .expect("fixture descriptor");
    value.authorization = Some(serde_json::from_value(json!({
        "actions":[{"code":"qr","label":"Scan QR code","kind":"qr_code"}],
        "managed_secret_keys":["qr_credential","qr_code_url","bot_token","bound_user_id","context_token","cursor"]
    })).expect("fixture authorization"));
    value
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    serve(|request, _| async move {
        let request: CapabilityStdioRequest = match serde_json::from_value(request) {
            Ok(request) => request,
            Err(_) => return json!({"ok":false,"result":null,"error":{"message":"invalid fixture request"}}),
        };
        // A peer can bypass author helpers; Host must independently reject its write set.
        if request.input["input_payload"]["operation"]["input_payload"]["text"] == "fixture-undeclared-patch" {
            return json!({"ok":true,"result":{"type":"send","outcome":{"status":"failed","failure_code":"provider_rejected"},"managed_secret_patch":{"undeclared":"private-provider-secret"}}});
        }
        let response = execute_authorizable_notification(request, &descriptor(), |config, input| async move {
            let result = match input.operation {
                NotificationOperation::Authorize { operation: NotificationAuthorizationOperation::Begin { .. } } => json!({
                    "type":"authorization","authorization":{"status":"pending","user_action":{"kind":"qr_code","qr_code_url":if config["channel"]=="unsafe-qr" {"https://example.com/fixture-qr-credential"} else {"https://example.com/fixture-qr"},"poll_interval_seconds":2}},"managed_secret_patch":{"qr_credential":"fixture-qr-credential","qr_code_url":"https://example.com/fixture-qr"}
                }),
                NotificationOperation::Authorize { operation: NotificationAuthorizationOperation::Poll } => {
                    if !input.managed_secrets.contains_key("qr_credential") {
                        return Err("fixture missing persisted QR credential");
                    }
                    json!({"type":"authorization","authorization":{"status":"authorized"},"managed_secret_patch":{"qr_credential":null,"qr_code_url":null,"bot_token":"fixture-bot-token","bound_user_id":"fixture-scanned-peer"}})
                }
                NotificationOperation::Authorize { operation: NotificationAuthorizationOperation::Cancel } => json!({"type":"authorization","authorization":{"status":"unauthorized"},"managed_secret_patch":{"qr_credential":null}}),
                NotificationOperation::Authorize { operation: NotificationAuthorizationOperation::Maintain } => {
                    let mut view = json!({"status":if input.managed_secrets.contains_key("bot_token") {"authorized"} else {"unauthorized"}});
                    if let Some(window) = input.delivery_window { view["delivery_window"] = serde_json::to_value(window).unwrap(); }
                    json!({"type":"authorization","authorization":view})
                },
                NotificationOperation::Send { input_payload } => {
                    if input_payload["text"] == "fixture-subscription-unavailable" {
                        if input.delivery_window.as_ref().map(|window| window.status) != Some(runtime_extension_sdk::NotificationDeliveryWindowStatus::Expired) {
                            return Err("fixture requires Host-projected expired evidence");
                        }
                        if input.managed_secrets.contains_key("delivery_window") { return Err("window must not be secret"); }
                    }
                    let (code, status) = match input_payload["text"].as_str() {
                        Some("fixture-invalid") => ("authorization_invalid", Some("invalid")),
                        Some("fixture-recipient-not-ready") => ("recipient_not_ready", Some("authorized")),
                        Some("fixture-subscription-required") => ("subscription_required", Some("authorized")),
                        Some("fixture-subscription-expired") => ("subscription_expired", Some("authorized")),
                        Some("fixture-subscription-unavailable") => ("subscription_unavailable", Some("authorized")),
                        Some("fixture-rejected") => ("provider_rejected", None),
                        Some("fixture-unknown") => return Err("fixture ambiguous provider response"),
                        _ if !input.managed_secrets.contains_key("bot_token") => ("authorization_required", Some("unauthorized")),
                        _ => ("", None),
                    };
                    if code.is_empty() {
                        json!({"type":"send","outcome":{"status":"succeeded","result":{"accepted":true,"delivery_id":if input_payload["text"]=="fixture-secret-output" {input.managed_secrets["bot_token"].as_str().unwrap_or("private-provider-secret").to_owned()} else {format!("{}:fixture",config["channel"].as_str().unwrap())}}},"authorization":{"status":"authorized"},"managed_secret_patch":{"cursor":"next","context_token":"fixture-context"}})
                    } else {
                        let mut result = json!({"type":"send","outcome":{"status":"failed","failure_code":code},"managed_secret_patch":{"cursor":"failure-next"}});
                        if let Some(status) = status { result["authorization"] = json!({"status":status}); }
                        let window = match code {
                            "subscription_required" => Some(json!({"status":"waiting_for_message"})),
                            "subscription_expired" => Some(json!({"status":"active","last_received_at_ms":1,"expires_at_ms":2})),
                            "subscription_unavailable" => Some(json!({"status":"unknown","last_received_at_ms":1,"expires_at_ms":2})),
                            _ => None,
                        };
                        if let Some(window) = window { result["authorization"]["delivery_window"] = window; }
                        result
                    }
                }
                NotificationOperation::Receive { .. } | NotificationOperation::DownloadAttachment { .. } => return Err("send-only fixture"),
            };
            Ok::<NotificationRuntimeResult, &str>(serde_json::from_value(result).expect("fixture outcome"))
        }).await;
        serde_json::to_value(response).expect("fixture response")
    }).await?;
    Ok(())
}
