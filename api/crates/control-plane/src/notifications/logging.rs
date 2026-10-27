use super::*;
use std::collections::BTreeMap;

impl<R> NotificationService<R>
where
    R: NotificationRepository,
{
    pub(super) async fn snapshot_send_message(
        &self,
        actor: &ActorContext,
        log_id: Uuid,
        descriptor: &NotificationTypeDescriptor,
        input: &Value,
        config: &Value,
        managed: &BTreeMap<String, Value>,
    ) -> Result<()> {
        let Some(mapping) = &descriptor.log else {
            return Ok(());
        };
        let snapshot = mapping
            .project_input(input)
            .map_err(|_| Error::InvalidInput("notification_input_schema"))?;
        // Keep the full log envelope: authorization-only QR exemptions never apply here.
        if log_contains_credentials(
            &serde_json::to_value(&snapshot)?,
            descriptor,
            config,
            managed,
            &BTreeMap::new(),
        ) {
            return Err(Error::InvalidInput("notification_input_schema").into());
        }
        self.repository
            .snapshot_notification_send_log(actor.current_workspace_id, log_id, snapshot)
            .await
    }
}

pub(super) fn result_extra(
    descriptor: &NotificationTypeDescriptor,
    result: &Value,
    config: &Value,
    managed: &BTreeMap<String, Value>,
    patch: &BTreeMap<String, Value>,
) -> Result<Option<Value>> {
    let Some(mapping) = &descriptor.log else {
        return Ok(None);
    };
    let extra = mapping.project_result(result);
    if log_contains_credentials(&json!({"extra": extra}), descriptor, config, managed, patch) {
        return Err(Error::UpstreamUnavailable("notification_provider_unsafe_output").into());
    }
    Ok(Some(extra))
}

// Logs have no QR-display exemption. String and numeric credential values are private;
// booleans/null remain protocol state rather than matching every ordinary true/false/null.
fn log_contains_credentials(
    public: &Value,
    descriptor: &NotificationTypeDescriptor,
    config: &Value,
    managed: &BTreeMap<String, Value>,
    patch: &BTreeMap<String, Value>,
) -> bool {
    fn contains_scalar(public: &Value, secret: &str) -> bool {
        match public {
            Value::String(value) => value.contains(secret),
            Value::Number(value) => value.to_string() == secret,
            Value::Array(values) => values.iter().any(|value| contains_scalar(value, secret)),
            Value::Object(fields) => fields
                .iter()
                .any(|(key, value)| key.contains(secret) || contains_scalar(value, secret)),
            _ => false,
        }
    }
    fn reflects(public: &Value, secret: &Value) -> bool {
        match secret {
            Value::String(secret) => !secret.is_empty() && contains_scalar(public, secret),
            Value::Number(secret) => contains_scalar(public, &secret.to_string()),
            Value::Array(values) => values.iter().any(|secret| reflects(public, secret)),
            Value::Object(fields) => fields.values().any(|secret| reflects(public, secret)),
            _ => false,
        }
    }
    descriptor
        .secret_config_fields()
        .filter_map(|key| config.get(key))
        .chain(managed.values())
        .chain(patch.values())
        .any(|secret| reflects(public, secret))
}
