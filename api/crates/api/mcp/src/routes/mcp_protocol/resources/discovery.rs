//! Stateless discovery is a projection of the current actor-filtered catalog.
use super::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use std::collections::BTreeMap;

const TEMPLATE_PREFIX: &str = "flowbase-resource://template/";

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TemplateIdentity {
    instance_id: String,
    tool_id: String,
    des_id: String,
}

fn template_uri(instance: &domain::McpInstanceRecord, tool: &domain::McpToolRecord) -> String {
    let identity = TemplateIdentity {
        instance_id: instance.instance_id.clone(),
        tool_id: tool.tool_id.clone(),
        des_id: tool.des_id.clone(),
    };
    let encoded = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&identity).expect("identity JSON"));
    format!("{TEMPLATE_PREFIX}{encoded}?arguments={{arguments}}&resource_uri={{resource_uri}}")
}

pub(super) fn decode_read_reference(
    uri: &str,
    actor: &domain::ActorContext,
) -> Result<ResourceReference, &'static str> {
    let Some(template) = uri.strip_prefix(TEMPLATE_PREFIX) else {
        return ResourceReference::decode(uri);
    };
    if !media::valid_uri(uri) {
        return Err("Invalid resource template URI");
    }
    McpToolArguments::from_protocol(json!({"uri":uri}))
        .map_err(|_| "Resource reference exceeds transport budget")?;
    let (identity, query) = template
        .split_once('?')
        .ok_or("Invalid resource template URI")?;
    let identity: TemplateIdentity = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(identity)
            .map_err(|_| "Invalid resource template URI")?,
    )
    .map_err(|_| "Invalid resource template URI")?;
    let pairs = url::form_urlencoded::parse(query.as_bytes()).collect::<Vec<_>>();
    if pairs.len() != 2
        || pairs.iter().filter(|(key, _)| key == "arguments").count() != 1
        || pairs
            .iter()
            .filter(|(key, _)| key == "resource_uri")
            .count()
            != 1
    {
        return Err("Invalid resource template parameters");
    }
    let parameter = |name: &str| {
        pairs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_ref())
            .unwrap()
    };
    let reference = ResourceReference(extension_contracts::ResourceReference {
        v: 1,
        user_id: actor.user_id,
        workspace_id: actor.current_workspace_id,
        instance_id: identity.instance_id,
        tool_id: identity.tool_id,
        des_id: identity.des_id,
        arguments: serde_json::from_str(parameter("arguments"))
            .map_err(|_| "Invalid resource arguments")?,
        resource_uri: parameter("resource_uri").to_owned(),
    });
    if reference.resource_uri.starts_with("flowbase-resource:") {
        return Err("Resource template requires an original resource URI");
    }
    // Reuse the canonical locator codec and its public-argument budget; no new authority wire.
    ResourceReference::decode(&reference.encode()?)
}

pub(in super::super) fn list_result(
    actor: &domain::ActorContext,
    catalog: &domain::McpCatalogSnapshot,
    scope: &virtual_ui::VirtualMcpScope,
    arguments: &Value,
    templates: bool,
) -> VirtualToolOutcome {
    match entries(actor, catalog, scope, templates).and_then(|entries| {
        page(
            entries,
            arguments,
            templates,
            super::super::JSON_RPC_RESPONSE_MAX_BYTES / 2,
        )
    }) {
        Ok(value) => VirtualToolOutcome::Success(value),
        Err(message) => VirtualToolOutcome::invalid(message),
    }
}

fn entries(
    actor: &domain::ActorContext,
    catalog: &domain::McpCatalogSnapshot,
    scope: &virtual_ui::VirtualMcpScope,
    templates: bool,
) -> Result<BTreeMap<String, Value>, &'static str> {
    let mut entries = BTreeMap::new();
    for candidate in &catalog.tools {
        let Some((instance, tool)) =
            virtual_ui::visible_tool(catalog, scope, &candidate.tool_id, false)
        else {
            continue;
        };
        if instance.workspace_id != actor.current_workspace_id
            || tool.workspace_id != actor.current_workspace_id
            || !matches!(
                tool.execution_target,
                domain::McpToolExecutionTarget::InterfaceWrapper { .. }
            )
        {
            continue;
        }
        let Ok(Some(mapping)) = media::mapping(&tool.output_mapping) else {
            continue;
        };
        if !mapping.resource_read {
            continue;
        }
        if templates {
            let uri = template_uri(instance, tool);
            entries.insert(uri.clone(), json!({"name":tool.name,"uriTemplate":uri,"description":format!("{}; arguments is percent-encoded JSON; resource_uri selects one original resource URI.", tool.short_description)}));
            continue;
        }
        for declaration in mapping.resources {
            if !media::valid_uri(&declaration.uri)
                || declaration.uri.starts_with("flowbase-resource:")
                || declaration
                    .mime_type
                    .as_deref()
                    .is_some_and(|mime| !media::valid_mime(mime, None))
            {
                return Err("Invalid resource discovery declaration");
            }
            let reference = ResourceReference(extension_contracts::ResourceReference {
                v: 1,
                user_id: actor.user_id,
                workspace_id: actor.current_workspace_id,
                instance_id: instance.instance_id.clone(),
                tool_id: tool.tool_id.clone(),
                des_id: tool.des_id.clone(),
                arguments: Value::Object(declaration.arguments),
                resource_uri: declaration.uri,
            });
            let uri = reference.encode()?;
            let mut entry = json!({"name":declaration.name,"uri":uri});
            if let Some(mime) = declaration.mime_type {
                entry["mimeType"] = json!(mime);
            }
            entries.insert(uri, entry);
        }
    }
    Ok(entries)
}

/// Last-key pagination stays deterministic under reorder and never retains binary data.
fn page(
    entries: BTreeMap<String, Value>,
    arguments: &Value,
    templates: bool,
    budget: usize,
) -> Result<Value, &'static str> {
    let field = if templates {
        "resourceTemplates"
    } else {
        "resources"
    };
    let after = match arguments.get("cursor") {
        None => None,
        Some(Value::String(cursor)) => {
            let bytes = URL_SAFE_NO_PAD
                .decode(cursor)
                .map_err(|_| "Invalid resource cursor")?;
            let (kind, last): (String, String) =
                serde_json::from_slice(&bytes).map_err(|_| "Invalid resource cursor")?;
            if kind != field {
                return Err("Invalid resource cursor");
            }
            Some(last)
        }
        _ => return Err("Invalid resource cursor"),
    };
    let mut result = json!({field:[]});
    let mut remaining = entries
        .into_iter()
        .filter(|(key, _)| after.as_ref().is_none_or(|after| key > after))
        .peekable();
    let mut last_cursor = None;
    while let Some((key, entry)) = remaining.next() {
        let cursor = URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&(field, key)).map_err(|_| "Invalid resource cursor")?);
        result[field].as_array_mut().unwrap().push(entry);
        if remaining.peek().is_some() {
            result["nextCursor"] = json!(cursor);
        } else {
            result.as_object_mut().unwrap().remove("nextCursor");
        }
        if serde_json::to_vec(&result)
            .map_err(|_| "Invalid resource discovery")?
            .len()
            > budget
        {
            result[field].as_array_mut().unwrap().pop();
            let Some(last) = last_cursor else {
                return Err("Resource discovery entry exceeds response budget");
            };
            result["nextCursor"] = json!(last);
            return Ok(result);
        }
        last_cursor = Some(cursor);
    }
    Ok(result)
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "../_tests/resource_discovery.rs"]
mod tests;
