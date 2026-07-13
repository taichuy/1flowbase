use std::collections::HashMap;

use anyhow::Result;
use plugin_framework::provider_contract::PluginFormFieldSchema;
use serde_json::{json, Map, Value};

use crate::errors::ControlPlaneError;

pub(super) async fn load_data_source_config_schema(
    installation: &domain::PluginInstallationRecord,
) -> Result<Vec<PluginFormFieldSchema>> {
    let installed_path = installation.installed_path.clone();
    let expected_source_code = installation.provider_code.clone();
    let package = tokio::task::spawn_blocking(move || {
        plugin_framework::DataSourcePackage::load_from_dir(installed_path)
    })
    .await??;
    if package.definition.source_code != expected_source_code {
        return Err(ControlPlaneError::InvalidInput("source_code").into());
    }
    Ok(package.definition.config_schema)
}

pub(super) fn classify_data_source_config(
    config_schema: &[PluginFormFieldSchema],
    config_json: &Value,
    secret_json: &Value,
    secret_ref: &str,
    secret_version: i32,
) -> Result<(Value, Value)> {
    let config_input = ensure_json_object(config_json, "config_json")?;
    let secret_input = ensure_json_object(secret_json, "secret_json")?;
    let config_input = config_input
        .as_object()
        .ok_or(ControlPlaneError::InvalidInput("config_json"))?;
    let secret_input = secret_input
        .as_object()
        .ok_or(ControlPlaneError::InvalidInput("secret_json"))?;
    let schema_by_key = config_schema
        .iter()
        .map(|field| (field.key.as_str(), field))
        .collect::<HashMap<_, _>>();

    for key in config_input.keys().chain(secret_input.keys()) {
        if !schema_by_key.contains_key(key.as_str()) {
            return Err(ControlPlaneError::InvalidInput("config_json").into());
        }
    }

    let mut classified_config = Map::new();
    let mut classified_secrets = Map::new();
    for field in config_schema {
        let public_value = config_input.get(&field.key);
        let secret_value = secret_input.get(&field.key);
        if public_value.is_some() && secret_value.is_some() {
            return Err(ControlPlaneError::InvalidInput("config_json").into());
        }
        let value = public_value.or(secret_value);
        let is_secret = field
            .send_mode
            .as_deref()
            .is_some_and(|mode| mode.eq_ignore_ascii_case("secret_ref"));

        if field.required.unwrap_or(false) && value.is_none_or(is_empty_required_config_value) {
            return Err(ControlPlaneError::InvalidInput("config_json").into());
        }

        let Some(value) = value else {
            continue;
        };
        if is_secret {
            if is_secret_reference_marker(value) {
                return Err(ControlPlaneError::InvalidInput("secret_json").into());
            }
            classified_secrets.insert(field.key.clone(), value.clone());
        } else {
            classified_config.insert(field.key.clone(), value.clone());
        }
    }

    let mut merged_secret_json = Value::Object(classified_secrets);
    let sanitized_config = scrub_secret_like_config_values(
        &Value::Object(classified_config),
        &mut merged_secret_json,
        secret_ref,
        secret_version,
        &mut Vec::new(),
    );
    Ok((sanitized_config, merged_secret_json))
}

pub(super) fn validate_data_source_secret_rotation(
    config_schema: &[PluginFormFieldSchema],
    secret_json: &Value,
) -> Result<Value> {
    let secret_json = ensure_json_object(secret_json, "secret_json")?;
    let secret_object = secret_json
        .as_object()
        .ok_or(ControlPlaneError::InvalidInput("secret_json"))?;
    if secret_object.is_empty() {
        return Err(ControlPlaneError::InvalidInput("secret_json").into());
    }

    let schema_by_key = config_schema
        .iter()
        .map(|field| (field.key.as_str(), field))
        .collect::<HashMap<_, _>>();
    for (key, value) in secret_object {
        let field = schema_by_key
            .get(key.as_str())
            .ok_or(ControlPlaneError::InvalidInput("secret_json"))?;
        let is_secret = field
            .send_mode
            .as_deref()
            .is_some_and(|mode| mode.eq_ignore_ascii_case("secret_ref"));
        if !is_secret || is_empty_required_config_value(value) || is_secret_reference_marker(value)
        {
            return Err(ControlPlaneError::InvalidInput("secret_json").into());
        }
    }
    Ok(secret_json)
}

fn ensure_json_object(value: &Value, field: &'static str) -> Result<Value> {
    if value.is_object() {
        Ok(value.clone())
    } else {
        Err(ControlPlaneError::InvalidInput(field).into())
    }
}

fn is_empty_required_config_value(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(value) => value.trim().is_empty(),
        Value::Array(values) => values.is_empty(),
        Value::Object(values) => values.is_empty(),
        _ => false,
    }
}

fn scrub_secret_like_config_values(
    value: &Value,
    secret_json: &mut Value,
    secret_ref: &str,
    secret_version: i32,
    path: &mut Vec<String>,
) -> Value {
    match value {
        Value::Object(object) => {
            let mut sanitized = Map::new();
            for (key, child) in object {
                path.push(key.clone());
                let next = if is_secret_bearing_config_value(key, child, path, object)
                    && !is_secret_reference_marker(child)
                {
                    store_config_secret_value(secret_json, path, child.clone());
                    secret_reference_marker(secret_ref, secret_version)
                } else {
                    scrub_secret_like_config_values(
                        child,
                        secret_json,
                        secret_ref,
                        secret_version,
                        path,
                    )
                };
                path.pop();
                sanitized.insert(key.clone(), next);
            }
            Value::Object(sanitized)
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    path.push(index.to_string());
                    let next = scrub_secret_like_config_values(
                        item,
                        secret_json,
                        secret_ref,
                        secret_version,
                        path,
                    );
                    path.pop();
                    next
                })
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn store_config_secret_value(secret_json: &mut Value, path: &[String], value: Value) {
    if let Some(last) = path.last() {
        if path.len() == 1 {
            if let Some(secret_object) = secret_json.as_object_mut() {
                secret_object
                    .entry(last.clone())
                    .or_insert_with(|| value.clone());
            }
        }
    }

    let pointer = format!("/{}", path.join("/"));
    if let Some(secret_object) = secret_json.as_object_mut() {
        let entry = secret_object
            .entry("__config_secret_values")
            .or_insert_with(|| Value::Object(Map::new()));
        if let Some(config_secret_values) = entry.as_object_mut() {
            config_secret_values.insert(pointer, value);
        }
    }
}

fn is_secret_like_config_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase().replace(['-', ' '], "_");
    if normalized == "secret_ref" || normalized == "secret_version" || normalized.ends_with("_ref")
    {
        return false;
    }

    normalized.contains("secret")
        || normalized.contains("password")
        || normalized.contains("token")
        || normalized.contains("api_key")
        || normalized.contains("apikey")
        || normalized.contains("authorization")
        || normalized.contains("private_key")
}

fn is_secret_bearing_config_value(
    key: &str,
    child: &Value,
    path: &[String],
    parent: &Map<String, Value>,
) -> bool {
    if is_secret_like_config_key(key) {
        return true;
    }

    if key == "value" && path_matches_headers_value(path) {
        return parent
            .get("name")
            .or_else(|| parent.get("key"))
            .and_then(Value::as_str)
            .map(is_secret_bearing_header_name)
            .unwrap_or(false);
    }

    key == "value" && path_matches_credentials_value(path) && !child.is_null()
}

fn is_secret_bearing_header_name(name: &str) -> bool {
    matches!(
        name.trim().to_ascii_lowercase().as_str(),
        "authorization"
            | "proxy-authorization"
            | "x-api-key"
            | "api-key"
            | "x-auth-token"
            | "cookie"
    )
}

fn path_matches_headers_value(path: &[String]) -> bool {
    path.len() >= 3
        && path.last().map(String::as_str) == Some("value")
        && path.get(path.len() - 3).map(String::as_str) == Some("headers")
        && path
            .get(path.len() - 2)
            .map(|segment| segment.parse::<usize>().is_ok())
            .unwrap_or(false)
}

fn path_matches_credentials_value(path: &[String]) -> bool {
    path.len() >= 2
        && path.last().map(String::as_str) == Some("value")
        && path.get(path.len() - 2).map(String::as_str) == Some("credentials")
}

fn is_secret_reference_marker(value: &Value) -> bool {
    value
        .as_object()
        .map(|object| object.contains_key("secret_ref") && object.contains_key("secret_version"))
        .unwrap_or(false)
}

fn secret_reference_marker(secret_ref: &str, secret_version: i32) -> Value {
    json!({
        "secret_ref": secret_ref,
        "secret_version": secret_version,
    })
}
