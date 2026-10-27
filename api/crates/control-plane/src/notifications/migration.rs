use super::*;

/// Conservative, declaration-based compatibility for preserving a channel's durable state.
/// Presentation and receive-tool descriptions may evolve; stored configuration, credentials,
/// send payloads and the receiving contribution must keep their existing contract.
pub fn compatible_notification_migration_types(
    current_metadata: &Value,
    target_metadata: &Value,
) -> Result<Vec<String>> {
    let current = descriptors(current_metadata)?;
    let target = descriptors(target_metadata)?;
    Ok(current
        .iter()
        .filter(|old| target.iter().any(|new| compatible(old, new)))
        .map(|descriptor| descriptor.type_code.clone())
        .collect())
}

fn descriptors(metadata: &Value) -> Result<Vec<NotificationTypeDescriptor>> {
    let Some(raw) = metadata.get("notification_types") else {
        return Ok(Vec::new());
    };
    let descriptors: Vec<NotificationTypeDescriptor> = serde_json::from_value(raw.clone())
        .map_err(|_| Error::Conflict("notification_type_metadata"))?;
    let mut codes = BTreeSet::new();
    for descriptor in &descriptors {
        descriptor
            .validate()
            .map_err(|_| Error::Conflict("notification_type_metadata"))?;
        if !codes.insert(&descriptor.type_code) {
            return Err(Error::Conflict("notification_type_metadata").into());
        }
    }
    Ok(descriptors)
}

fn compatible(old: &NotificationTypeDescriptor, new: &NotificationTypeDescriptor) -> bool {
    old.type_code == new.type_code
        && old.contribution_id == new.contribution_id
        && old.config_schema == new.config_schema
        && old.input_schema == new.input_schema
        && old.output_schema == new.output_schema
        && old.secret_config_fields().collect::<BTreeSet<_>>()
            == new.secret_config_fields().collect::<BTreeSet<_>>()
        && match (&old.authorization, &new.authorization) {
            (None, None) => true,
            (Some(old), Some(new)) => {
                old.supports_delivery_window == new.supports_delivery_window
                    && old.managed_secret_keys.iter().collect::<BTreeSet<_>>()
                        == new.managed_secret_keys.iter().collect::<BTreeSet<_>>()
                    && old.actions.len() == new.actions.len()
                    && old.actions.iter().all(|action| {
                        new.actions.iter().any(|candidate| {
                            action.code == candidate.code && action.kind == candidate.kind
                        })
                    })
            }
            _ => false,
        }
        && match (&old.receive, &new.receive) {
            (None, _) => true,
            (Some(old), Some(new)) => old.contribution_id == new.contribution_id,
            (Some(_), None) => false,
        }
}

#[cfg(test)]
#[path = "_tests/migration.rs"]
mod tests;
