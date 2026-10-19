//! Private configuration never shares the public extension_config projection.
use crate::{errors::ControlPlaneError, ports::AuthenticationSecretCodec};
use anyhow::Result;
use serde_json::{Map, Value};

pub(super) fn protect_configuration(
    entry: &mut domain::LoginEntryRecord,
    config: &mut Map<String, Value>,
    schema: &Value,
    codec: Option<&dyn AuthenticationSecretCodec>,
) -> Result<()> {
    let fields = schema
        .as_array()
        .ok_or(ControlPlaneError::InvalidInput("config_schema"))?;
    let secret_keys: Vec<&str> = fields
        .iter()
        .filter(|field| {
            matches!(
                field.get("type").and_then(Value::as_str),
                Some("secret" | "password")
            )
        })
        .filter_map(|field| field.get("key").and_then(Value::as_str))
        .collect();
    // Only declared fields enter configuration; callback URL is a Host-owned projection.
    if config.keys().any(|key| {
        key != "self_registration_enabled"
            && !fields
                .iter()
                .any(|field| field["key"].as_str() == Some(key))
    }) {
        return Err(ControlPlaneError::InvalidInput("extension_config").into());
    }
    config.remove("callback_url");
    if secret_keys.is_empty() {
        return validate_fields(config, fields);
    }
    let codec = codec.ok_or(ControlPlaneError::InvalidInput(
        "authentication_secret_store",
    ))?;
    let mut secrets = match entry.options.get("authentication_secret_envelope") {
        Some(envelope) => codec
            .open(entry.connection_id, envelope)?
            .as_object()
            .cloned()
            .ok_or(ControlPlaneError::InvalidInput(
                "authentication_secret_envelope",
            ))?,
        None => Map::new(),
    };
    for key in secret_keys {
        if let Some(value) = config.remove(key) {
            match value {
                Value::String(value) if !value.trim().is_empty() => {
                    secrets.insert(key.to_owned(), Value::String(value));
                }
                Value::Null => {}
                Value::String(_) => {} // An unchanged password input keeps the stored secret.
                _ => return Err(ControlPlaneError::InvalidInput("authentication_secret").into()),
            }
        }
    }
    let keys = Value::Array(secrets.keys().cloned().map(Value::String).collect());
    let options = entry
        .options
        .as_object_mut()
        .ok_or(ControlPlaneError::InvalidInput("login_entry_options"))?;
    options.insert("authentication_secret_keys".into(), keys);
    options.insert(
        "authentication_secret_envelope".into(),
        codec.seal(entry.connection_id, &Value::Object(secrets))?,
    );
    validate_fields(config, fields)?;
    let stored_keys = options
        .get("authentication_secret_keys")
        .and_then(Value::as_array);
    for field in fields.iter().filter(|field| {
        field["required"] == true && matches!(field["type"].as_str(), Some("secret" | "password"))
    }) {
        if !stored_keys.is_some_and(|keys| keys.contains(&field["key"])) {
            return Err(ControlPlaneError::InvalidInput("authentication_secret_required").into());
        }
    }
    Ok(())
}

fn validate_fields(config: &Map<String, Value>, fields: &[Value]) -> Result<()> {
    for field in fields {
        let Some(key) = field["key"].as_str() else {
            continue;
        };
        if key == "callback_url"
            || super::AUTH_CENTER_HOST_CONFIG_KEYS.contains(&key)
            || matches!(field["type"].as_str(), Some("secret" | "password"))
        {
            continue;
        }
        let value = config.get(key).filter(|value| !value.is_null());
        if field["required"] == true
            && value.is_none_or(|value| value.as_str().is_some_and(|value| value.trim().is_empty()))
        {
            return Err(ControlPlaneError::InvalidInput("authentication_config_required").into());
        }
        let Some(value) = value else {
            continue;
        };
        let valid = match field["type"].as_str() {
            Some("string") => value.is_string(),
            Some("boolean") => value.is_boolean(),
            Some("number") => value.as_f64().is_some_and(|value| {
                value.is_finite()
                    && field["min"].as_f64().is_none_or(|min| value >= min)
                    && field["max"].as_f64().is_none_or(|max| value <= max)
                    && (field["precision"] != 0 || value.fract() == 0.0)
            }),
            _ => true,
        };
        if !valid {
            return Err(ControlPlaneError::InvalidInput("authentication_config").into());
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "_tests/secrets.rs"]
mod tests;
