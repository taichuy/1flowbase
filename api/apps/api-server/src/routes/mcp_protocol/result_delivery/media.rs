//! Native content is independent of metadata continuation and selection.
use base64::{engine::general_purpose::STANDARD, Engine};
use domain::McpCallToolResult;
use serde_json::{json, Value};

pub(crate) fn mapping(
    value: &Value,
) -> Result<Option<plugin_framework::ManagedMcpMediaMapping>, &'static str> {
    if value.get("mcp_content").is_none() && value.get("structured_content_path").is_none() {
        return Ok(None);
    }
    plugin_framework::ManagedMcpMediaMapping::parse(value).map(Some)
}

pub(crate) fn valid_mime(mime: &str, prefix: Option<&str>) -> bool {
    fn token(value: &str) -> bool {
        !value.is_empty()
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
    }
    mime.split_once('/').is_some_and(|(kind, subtype)| {
        token(kind)
            && token(subtype)
            && prefix.is_none_or(|expected| kind.eq_ignore_ascii_case(expected))
    })
}

pub(crate) fn valid_uri(uri: &str) -> bool {
    // URL parsers normalize whitespace and bad percent escapes; reject them before parsing.
    if uri
        .bytes()
        .any(|b| b.is_ascii_control() || b.is_ascii_whitespace() || b"<>\\\"{}|^`".contains(&b))
    {
        return false;
    }
    let bytes = uri.as_bytes();
    if bytes.iter().enumerate().any(|(i, b)| {
        *b == b'%'
            && !(bytes.get(i + 1).is_some_and(u8::is_ascii_hexdigit)
                && bytes.get(i + 2).is_some_and(u8::is_ascii_hexdigit))
    }) {
        return false;
    }
    url::Url::parse(uri).is_ok()
}

pub(crate) fn valid_base64(data: &str, allow_empty: bool) -> bool {
    if (!allow_empty && data.is_empty())
        || data
            .bytes()
            .enumerate()
            .any(|(index, byte)| byte == b'=' && index < data.len().saturating_sub(2))
    {
        return false;
    }
    let mut decoded = [0u8; 3072];
    data.as_bytes()
        .chunks(4096)
        .all(|chunk| STANDARD.decode_slice(chunk, &mut decoded).is_ok())
}

fn string<'a>(source: &'a Value, path: &str) -> Result<&'a str, &'static str> {
    source
        .pointer(path)
        .and_then(Value::as_str)
        .ok_or("content path must resolve to a string")
}

pub(crate) fn project(
    mapping: &plugin_framework::ManagedMcpMediaMapping,
    source: &Value,
) -> Result<McpCallToolResult, &'static str> {
    let metadata = source
        .pointer(&mapping.structured_content_path)
        .filter(|value| value.is_object())
        .ok_or("media metadata path must resolve to an object")?;
    let mut content = Vec::new();
    for item in &mapping.mcp_content {
        use plugin_framework::ManagedMcpImageMapping::*;
        let (mut block, annotations) = match item {
            Text {
                text_path,
                annotations_path,
            } => (
                json!({"type":"text","text":string(source, text_path)?}),
                annotations_path,
            ),
            Image {
                data_path,
                mime_type_path,
                annotations_path,
            }
            | Audio {
                data_path,
                mime_type_path,
                annotations_path,
            } => {
                let kind = if matches!(item, Image { .. }) {
                    "image"
                } else {
                    "audio"
                };
                let data = string(source, data_path)?;
                let mime = string(source, mime_type_path)?;
                if !valid_base64(data, false) {
                    return Err("media data is not valid nonempty base64");
                }
                if !valid_mime(mime, Some(kind)) {
                    return Err("media MIME type is invalid");
                }
                (
                    json!({"type":kind,"data":data,"mimeType":mime}),
                    annotations_path,
                )
            }
            ResourceLink {
                uri_path,
                name_path,
                mime_type_path,
                description_path,
                annotations_path,
            } => {
                let uri = string(source, uri_path)?;
                if !valid_uri(uri) {
                    return Err("resource URI must be absolute");
                }
                let mut block =
                    json!({"type":"resource_link","uri":uri,"name":string(source, name_path)?});
                optional_mime(&mut block, source, mime_type_path)?;
                if let Some(path) = description_path {
                    block["description"] = json!(string(source, path)?);
                }
                (block, annotations_path)
            }
            Resource {
                uri_path,
                mime_type_path,
                text_path,
                blob_path,
                annotations_path,
            } => {
                let uri = string(source, uri_path)?;
                if !valid_uri(uri) {
                    return Err("resource URI must be absolute");
                }
                let mut resource = json!({"uri":uri});
                optional_mime(&mut resource, source, mime_type_path)?;
                match (text_path, blob_path) {
                    (Some(path), None) => resource["text"] = json!(string(source, path)?),
                    (None, Some(path)) => {
                        let blob = string(source, path)?;
                        if !valid_base64(blob, true) {
                            return Err("resource blob is not valid base64");
                        }
                        resource["blob"] = json!(blob);
                    }
                    _ => return Err("resource requires exactly one text or blob path"),
                }
                (
                    json!({"type":"resource","resource":resource}),
                    annotations_path,
                )
            }
        };
        if let Some(path) = annotations {
            let annotations = source
                .pointer(path)
                .filter(|a| valid_annotations(a))
                .ok_or("invalid content annotations")?;
            block["annotations"] = annotations.clone();
        }
        content.push(block);
    }
    Ok(McpCallToolResult {
        content: Value::Array(content),
        structured_content: Some(metadata.clone()),
        is_error: None,
    })
}

fn optional_mime(
    block: &mut Value,
    source: &Value,
    path: &Option<String>,
) -> Result<(), &'static str> {
    if let Some(path) = path {
        let mime = string(source, path)?;
        if !valid_mime(mime, None) {
            return Err("resource MIME type is invalid");
        }
        block["mimeType"] = json!(mime);
    }
    Ok(())
}

pub(crate) fn valid_annotations(value: &Value) -> bool {
    value.as_object().is_some_and(|object| {
        object.iter().all(|(key, value)| match key.as_str() {
            "audience" => value.as_array().is_some_and(|items| {
                items
                    .iter()
                    .all(|item| matches!(item.as_str(), Some("user" | "assistant")))
            }),
            "priority" => value.as_f64().is_some_and(|p| (0.0..=1.0).contains(&p)),
            "lastModified" => value.as_str().is_some(),
            _ => false,
        })
    })
}

/// Only valid standard binary fields are exempt from the serialized envelope budget.
pub(crate) fn binary_bytes(item: &Value, resource_contents: bool) -> Option<usize> {
    if resource_contents {
        return resource_blob_bytes(item);
    }
    match item.get("type").and_then(Value::as_str) {
        Some(kind @ ("image" | "audio")) => {
            if !item
                .get("mimeType")
                .and_then(Value::as_str)
                .is_some_and(|mime| valid_mime(mime, Some(kind)))
            {
                return None;
            }
            item.get("data")
                .and_then(Value::as_str)
                .filter(|data| valid_base64(data, false))
                .map(str::len)
        }
        Some("resource") => resource_blob_bytes(item.get("resource")?),
        _ => None,
    }
}

fn resource_blob_bytes(resource: &Value) -> Option<usize> {
    if resource.get("text").is_some()
        || !resource
            .get("uri")
            .and_then(Value::as_str)
            .is_some_and(valid_uri)
        || resource
            .get("mimeType")
            .is_some_and(|mime| !mime.as_str().is_some_and(|mime| valid_mime(mime, None)))
    {
        return None;
    }
    resource
        .get("blob")
        .and_then(Value::as_str)
        .filter(|blob| valid_base64(blob, true))
        .map(str::len)
}

pub(crate) fn retain_native_content(mut delivered: Value, original: &Value) -> Value {
    if let (Some(target), Some(source)) = (
        delivered.get_mut("content").and_then(Value::as_array_mut),
        original.as_array(),
    ) {
        target.extend(source.iter().cloned());
    }
    delivered
}

/// Proxy text continues through metadata budgets rather than being duplicated.
pub(crate) fn retain_content(delivered: Value, original: &Value) -> Value {
    let nontext = original
        .as_array()
        .map(|source| {
            source
                .iter()
                .filter(|item| item.get("type").and_then(Value::as_str) != Some("text"))
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    retain_native_content(delivered, &Value::Array(nontext))
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/media.rs"]
mod tests;
