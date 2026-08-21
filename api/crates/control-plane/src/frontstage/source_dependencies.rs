use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use regex::Regex;
use serde_json::Value;
use uuid::Uuid;

use crate::errors::ControlPlaneError;

use super::block_tree::is_dependency_lock;

const HOST_MODULE_SOURCES: &[&str] = &["react", "react/jsx-runtime", "antd", "tailwindcss"];

pub(super) fn dependency_lock_from_source(
    workspace_id: Uuid,
    source_code: &str,
    modules: Vec<domain::FrontendBlockCodeModule>,
) -> Result<Value> {
    let import_sources = static_import_sources(source_code);
    let requested_sources = import_sources
        .iter()
        .filter(|source| !HOST_MODULE_SOURCES.contains(&source.as_str()))
        .collect::<BTreeSet<_>>();
    let mut resolved_modules = BTreeMap::new();

    for module in modules {
        if !requested_sources.contains(&module.source) {
            continue;
        }
        match resolved_modules.get_mut(&module.source) {
            Some(existing) if same_runtime_module(existing, &module) => {
                existing.exports.extend(module.exports);
                existing.exports.sort();
                existing.exports.dedup();
            }
            Some(_) => {
                return Err(ControlPlaneError::InvalidInput(
                    "frontstage_component_module_ambiguous",
                )
                .into());
            }
            None => {
                resolved_modules.insert(module.source.clone(), module);
            }
        }
    }

    if requested_sources
        .iter()
        .any(|source| !resolved_modules.contains_key(*source))
    {
        return Err(ControlPlaneError::InvalidInput("frontstage_component_module_import").into());
    }

    canonical_dependency_lock(workspace_id, resolved_modules.into_values().collect())
}

fn same_runtime_module(
    left: &domain::FrontendBlockCodeModule,
    right: &domain::FrontendBlockCodeModule,
) -> bool {
    left.version == right.version && left.binding == right.binding && left.assets == right.assets
}

fn static_import_sources(source_code: &str) -> BTreeSet<String> {
    let source_without_comments = source_without_comments(source_code);
    let expression =
        Regex::new(r#"(?m)^[ \t]*import(?:\s+type)?(?:[\s\S]*?\bfrom)?\s*[\"']([^\"'\r\n]+)[\"']"#)
            .expect("static import expression is valid");
    expression
        .captures_iter(&source_without_comments)
        .filter_map(|capture| capture.get(1).map(|value| value.as_str().trim().to_owned()))
        .filter(|value| !value.is_empty())
        .collect()
}

fn source_without_comments(source: &str) -> String {
    let mut result = String::with_capacity(source.len());
    let mut index = 0;
    let bytes = source.as_bytes();
    let mut quote = None;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(current_quote) = quote {
            result.push(byte as char);
            if byte == b'\\' && index + 1 < bytes.len() {
                index += 1;
                result.push(bytes[index] as char);
            } else if byte == current_quote {
                quote = None;
            }
            index += 1;
            continue;
        }
        if matches!(byte, b'\'' | b'\"' | b'`') {
            quote = Some(byte);
            result.push(byte as char);
            index += 1;
            continue;
        }
        if byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            continue;
        }
        if byte == b'/' && bytes.get(index + 1) == Some(&b'*') {
            index += 2;
            while index < bytes.len() {
                if bytes[index] == b'*' && bytes.get(index + 1) == Some(&b'/') {
                    index += 2;
                    break;
                }
                if bytes[index] == b'\n' {
                    result.push('\n');
                }
                index += 1;
            }
            continue;
        }
        result.push(byte as char);
        index += 1;
    }
    result
}

fn canonical_dependency_lock(
    workspace_id: Uuid,
    modules: Vec<domain::FrontendBlockCodeModule>,
) -> Result<Value> {
    let entries = modules
        .into_iter()
        .map(|module| {
            let binding = match module.binding {
                domain::FrontendModuleBinding::Host => "host",
                domain::FrontendModuleBinding::Fetched => "fetched",
            };
            let assets = module
                .assets
                .into_iter()
                .map(|asset| {
                    let sha256 = asset.sha256;
                    let role = match asset.role {
                        domain::FrontendModuleAssetRole::BrowserModule => "browser_module",
                        domain::FrontendModuleAssetRole::ShadowStyle => "shadow_style",
                        domain::FrontendModuleAssetRole::Support => "support",
                    };
                    serde_json::json!({
                        "role": role,
                        "media_type": asset.media_type,
                        "sha256": sha256.clone(),
                        "url": format!(
                            "/api/console/frontstage/{workspace_id}/component-module-assets/{sha256}"
                        ),
                        "integrity": "verified_sha256"
                    })
                })
                .collect::<Vec<_>>();
            serde_json::json!({
                "module_source": module.source,
                "module_version": module.version,
                "binding": binding,
                "assets": assets,
                "exports": module.exports
            })
        })
        .collect::<Vec<_>>();
    let value = Value::Array(entries);
    if !is_dependency_lock(&value) {
        return Err(ControlPlaneError::InvalidInput("dependency_lock").into());
    }
    Ok(value)
}
