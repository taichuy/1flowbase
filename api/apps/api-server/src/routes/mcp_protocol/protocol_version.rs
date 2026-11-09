//! Supported wire subsets: tools and read-only resource discovery/read.
use super::*;

pub(super) const LATEST: &str = "2025-11-25";
pub(super) const LEGACY: &str = "2025-03-26";

pub(super) fn negotiate(
    method: &str,
    params: &Value,
    headers: &HeaderMap,
) -> Result<&'static str, &'static str> {
    if method == "initialize" {
        return match params.get("protocolVersion").and_then(Value::as_str) {
            Some(LEGACY) => Ok(LEGACY),
            Some(_) => Ok(LATEST),
            None => Err("Missing protocolVersion"),
        };
    }
    // Stateless Streamable HTTP: clients repeat their negotiated version on every request.
    // The protocol specifies 2025-03-26 when this header is absent.
    match headers.get("mcp-protocol-version") {
        None => Ok(LEGACY),
        Some(value) => match value.to_str() {
            Ok(LEGACY) => Ok(LEGACY),
            Ok(LATEST) => Ok(LATEST),
            _ => Err("Unsupported MCP-Protocol-Version"),
        },
    }
}

pub(super) fn project(mut result: Value, version: &str) -> Value {
    if version != LEGACY {
        return result;
    }
    let structured = result
        .as_object_mut()
        .and_then(|object| object.remove("structuredContent"));
    if let Some(content) = result.get_mut("content").and_then(Value::as_array_mut) {
        for block in content.iter_mut() {
            if block["type"] == "resource_link" {
                // Preserve the locator and descriptive fields in an old-schema text block.
                *block = json!({"type":"text", "text":block.to_string()});
            } else if let Some(annotations) =
                block.get_mut("annotations").and_then(Value::as_object_mut)
            {
                annotations.remove("lastModified");
            }
        }
        if let Some(structured) = structured {
            let represented = content.iter().any(|block| {
                block["type"] == "text"
                    && block["text"]
                        .as_str()
                        .and_then(|text| serde_json::from_str::<Value>(text).ok())
                        .as_ref()
                        == Some(&structured)
            });
            if !represented {
                content.push(json!({"type":"text","text":structured.to_string()}));
            }
        }
    } else if let Some(structured) = structured {
        result["content"] = json!([{"type":"text","text":structured.to_string()}]);
    }
    if let Some(tools) = result.get_mut("tools").and_then(Value::as_array_mut) {
        for tool in tools {
            if let Some(tool) = tool.as_object_mut() {
                tool.remove("outputSchema");
                tool.remove("execution");
            }
        }
    }
    result
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/protocol_version.rs"]
mod tests;
