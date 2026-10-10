use super::*;
use anyhow::Result;
use serde::Serialize;
use serde_json::json;
use std::collections::BTreeMap;

fn projected(
    out: &mut Vec<ProjectedTemplateResource>,
    kind: &str,
    source_id: String,
    target_id: String,
    mut value: Value,
    identities: &BTreeMap<String, String>,
) -> Result<()> {
    rewrite_template_value(&mut value, identities);
    let fingerprint = template_resource_fingerprint(&value)?;
    out.push(ProjectedTemplateResource {
        key: TemplateResourceKey {
            kind: kind.into(),
            source_id,
        },
        target_id,
        value,
        fingerprint,
    });
    Ok(())
}
fn without(value: &impl Serialize, fields: &[&str]) -> Result<Value> {
    let mut value = serde_json::to_value(value)?;
    if let Value::Object(object) = &mut value {
        for field in fields {
            object.remove(*field);
        }
    }
    Ok(value)
}
/// Composite IDs are JSON arrays, never delimiter-concatenated user-controlled identifiers.
fn composite(parts: &[&str]) -> String {
    serde_json::to_string(parts).expect("string array")
}

/// Portable DTOs already omit audit, timestamps, physical storage and generated revisions.
/// Remove only generated resource identity slots; typed references and user content remain.
/// Project targets with an empty identity map; project source with source -> target mappings.
pub fn project_template_resources(
    package: &PortableTemplatePackage,
    identities: &BTreeMap<String, String>,
) -> Result<Vec<ProjectedTemplateResource>> {
    let mapped = |id: &str| identities.get(id).cloned().unwrap_or_else(|| id.to_owned());
    let mut out = Vec::new();
    for page in &package.pages {
        let id = page.id.to_string();
        projected(
            &mut out,
            "page",
            id.clone(),
            mapped(&id),
            without(page, &["id", "tabs", "visibility_rules"])?,
            identities,
        )?;
        // Visibility is an independently managed policy, not part of page metadata.
        for rule in &page.visibility_rules {
            let tab = rule.tab_id.map(|id| id.to_string()).unwrap_or_default();
            let source = composite(&[&id, &tab, &rule.role_code]);
            let target = composite(&[&mapped(&id), &mapped(&tab), &rule.role_code]);
            projected(
                &mut out,
                "page_visibility",
                source,
                target,
                json!({"visibility":rule.visibility}),
                identities,
            )?;
        }
        for tab in &page.tabs {
            let tab_id = tab.id.to_string();
            let metadata = json!({"page_id":page.id,"metadata":without(tab, &["id", "document_root_uid", "document_payload", "blocks"])?});
            projected(
                &mut out,
                "tab",
                tab_id.clone(),
                mapped(&tab_id),
                metadata,
                identities,
            )?;
            projected(
                &mut out,
                "tab_document",
                tab_id.clone(),
                mapped(&tab_id),
                json!({"page_id":page.id,"document_payload":tab.document_payload}),
                identities,
            )?;
            for block in &tab.blocks {
                projected(
                    &mut out,
                    "block",
                    block.block_id.clone(),
                    mapped(&block.block_id),
                    json!({
                        "page_id":page.id,"tab_id":tab.id,
                        "definition":without(block, &["block_id", "code_ref"])?
                    }),
                    identities,
                )?;
            }
        }
    }
    for app in &package.applications {
        let id = app.id.to_string();
        projected(
            &mut out,
            "application",
            id.clone(),
            mapped(&id),
            without(app, &["id", "dependency_issues"])?,
            identities,
        )?;
    }
    for model in &package.data_models {
        if model.builtin {
            continue;
        }
        let id = model.id.to_string();
        projected(
            &mut out,
            "data_model",
            id.clone(),
            mapped(&id),
            without(model, &["id", "fields", "builtin"])?,
            identities,
        )?;
        for field in &model.fields {
            if field.is_system {
                continue;
            }
            let field_id = field.id.to_string();
            projected(
                &mut out,
                "model_field",
                field_id.clone(),
                mapped(&field_id),
                json!({"model_id":model.id,"definition":without(field, &["id"])?}),
                identities,
            )?;
        }
    }
    if let Some(bundle) = &package.mcp_bundle {
        for instance in &bundle.instances {
            let id = &instance.instance_id;
            projected(
                &mut out,
                "mcp_instance",
                id.clone(),
                mapped(id),
                without(instance, &["instance_id", "groups", "bindings"])?,
                identities,
            )?;
            for group in &instance.groups {
                projected(
                    &mut out,
                    "mcp_group",
                    composite(&[id, &group.path]),
                    composite(&[&mapped(id), &group.path]),
                    without(group, &["path"])?,
                    identities,
                )?;
            }
            for binding in &instance.bindings {
                projected(
                    &mut out,
                    "mcp_binding",
                    composite(&[id, &binding.group_path, &binding.tool_id]),
                    composite(&[&mapped(id), &binding.group_path, &mapped(&binding.tool_id)]),
                    without(binding, &["group_path", "tool_id"])?,
                    identities,
                )?;
            }
        }
        for tool in &bundle.tools {
            projected(
                &mut out,
                "mcp_tool",
                tool.tool_id.clone(),
                mapped(&tool.tool_id),
                without(tool, &["tool_id"])?,
                identities,
            )?;
        }
        for connection in &bundle.connections {
            let id = connection.connection_id.to_string();
            projected(
                &mut out,
                "mcp_connection",
                id.clone(),
                mapped(&id),
                without(connection, &["connection_id"])?,
                identities,
            )?;
        }
    }
    for entry in &package.i18n_entries {
        // Translation text is global dictionary content; never rewrite UUID-looking text.
        let value = json!({"translation":entry.translation});
        let id = composite(&[&entry.key, &entry.locale]);
        projected(
            &mut out,
            "i18n_entry",
            id.clone(),
            id,
            value,
            &BTreeMap::new(),
        )?;
    }
    Ok(out)
}
