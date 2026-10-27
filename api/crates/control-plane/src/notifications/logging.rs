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
        if authorization::public_contains_credentials(
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
    if authorization::public_contains_credentials(
        &json!({"extra": extra}),
        descriptor,
        config,
        managed,
        patch,
    ) {
        return Err(Error::UpstreamUnavailable("notification_provider_unsafe_output").into());
    }
    Ok(Some(extra))
}
