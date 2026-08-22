use std::collections::{BTreeMap, BTreeSet};

use crate::errors::ControlPlaneError;
use anyhow::Result;
use regex::Regex;
use serde_json::Value;

use super::block_tree::is_dependency_lock;

const REACT_MODULE_SOURCE: &str = "react";
const REACT_JSX_RUNTIME_MODULE_SOURCE: &str = "react/jsx-runtime";
const TAILWIND_MODULE_SOURCE: &str = "tailwindcss";

#[derive(Default)]
struct RequestedModule {
    runtime_exports: BTreeSet<String>,
}

struct StaticImport {
    source: String,
    runtime_exports: BTreeSet<String>,
}

pub(super) fn dependency_lock_from_source(
    source_code: &str,
    modules: Vec<domain::FrontendBlockCodeModule>,
) -> Result<Value> {
    let mut requested_modules = BTreeMap::<String, RequestedModule>::new();
    for import in static_imports(source_code) {
        if import.source == TAILWIND_MODULE_SOURCE {
            continue;
        }
        let source = if import.source == REACT_JSX_RUNTIME_MODULE_SOURCE {
            REACT_MODULE_SOURCE.to_owned()
        } else {
            import.source
        };
        requested_modules
            .entry(source)
            .or_default()
            .runtime_exports
            .extend(import.runtime_exports);
    }
    // Native React compilation may synthesize `react/jsx-runtime` even when
    // the authored source contains only JSX. The runtime maps that ABI to the
    // registered React host module, so the persisted lock must always retain it.
    requested_modules
        .entry(REACT_MODULE_SOURCE.to_owned())
        .or_default();
    let mut resolved_modules = BTreeMap::new();

    for module in modules {
        if !requested_modules.contains_key(&module.source) {
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

    for (source, requested) in &requested_modules {
        let module = resolved_modules
            .get(source)
            .ok_or(ControlPlaneError::InvalidInput(
                "frontstage_component_module_import",
            ))?;
        for export_name in &requested.runtime_exports {
            if !module.exports.contains(export_name) {
                return Err(ControlPlaneError::InvalidComponentModuleExport {
                    module_source: source.clone(),
                    export_name: export_name.clone(),
                }
                .into());
            }
        }
    }

    canonical_dependency_lock(resolved_modules.into_values().collect())
}

fn same_runtime_module(
    left: &domain::FrontendBlockCodeModule,
    right: &domain::FrontendBlockCodeModule,
) -> bool {
    left.version == right.version && left.binding == right.binding && left.assets == right.assets
}

fn static_imports(source_code: &str) -> Vec<StaticImport> {
    let source_without_comments = source_without_comments(source_code);
    let expression =
        Regex::new(
            r#"(?ms)^[ \t]*import(?:\s+(?P<type>type))?(?:\s+(?P<bindings>[^;]*?)\s+from)?\s*[\"'](?P<source>[^\"'\r\n]+)[\"']\s*;?"#,
        )
            .expect("static import expression is valid");
    expression
        .captures_iter(&source_without_comments)
        .filter_map(|capture| {
            let source = capture.name("source")?.as_str().trim();
            if source.is_empty() {
                return None;
            }
            Some(StaticImport {
                source: source.to_owned(),
                runtime_exports: runtime_imported_exports(
                    capture.name("bindings").map(|value| value.as_str()),
                    capture.name("type").is_some(),
                ),
            })
        })
        .collect()
}

fn runtime_imported_exports(bindings: Option<&str>, type_only: bool) -> BTreeSet<String> {
    if type_only {
        return BTreeSet::new();
    }
    let Some(bindings) = bindings else {
        return BTreeSet::new();
    };
    let bindings = bindings.trim();
    if bindings.is_empty() || bindings.starts_with('*') {
        return BTreeSet::new();
    }

    let mut exports = BTreeSet::new();
    let named_start = bindings.find('{');
    if let Some(named_start) = named_start {
        if let Some(named_end) = bindings[named_start + 1..]
            .find('}')
            .map(|offset| named_start + 1 + offset)
        {
            for binding in bindings[named_start + 1..named_end].split(',') {
                let binding = binding.trim();
                if binding.is_empty() || binding.starts_with("type ") {
                    continue;
                }
                let imported = binding
                    .split_whitespace()
                    .next()
                    .expect("non-empty import binding has a first token");
                exports.insert(imported.to_owned());
            }
        }
    }

    let default_binding = named_start
        .map(|index| &bindings[..index])
        .unwrap_or(bindings)
        .trim()
        .trim_end_matches(',')
        .trim();
    if !default_binding.is_empty() && !default_binding.starts_with('*') {
        exports.insert("default".to_owned());
    }
    exports
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

fn canonical_dependency_lock(modules: Vec<domain::FrontendBlockCodeModule>) -> Result<Value> {
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
                            "/api/console/frontstage/component-module-assets/{sha256}"
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

#[cfg(test)]
mod tests {
    use super::*;

    fn module(
        source: &str,
        binding: domain::FrontendModuleBinding,
        exports: &[&str],
    ) -> domain::FrontendBlockCodeModule {
        domain::FrontendBlockCodeModule {
            source: source.to_owned(),
            version: "1.0.0".to_owned(),
            exports: exports.iter().map(ToString::to_string).collect(),
            binding,
            assets: match binding {
                domain::FrontendModuleBinding::Host => vec![],
                domain::FrontendModuleBinding::Fetched => {
                    vec![domain::FrontendModuleAsset {
                        path: "module.js".to_owned(),
                        role: domain::FrontendModuleAssetRole::BrowserModule,
                        media_type: "text/javascript; charset=utf-8".to_owned(),
                        sha256: "a".repeat(64),
                    }]
                }
            },
            type_declarations: String::new(),
            components: vec![],
        }
    }

    #[test]
    fn resolves_host_imports_and_the_implicit_jsx_runtime_through_react() {
        let lock = dependency_lock_from_source(
            "import { Button } from 'antd';\nexport default () => <Button />;",
            vec![
                module(
                    "react",
                    domain::FrontendModuleBinding::Host,
                    &["default", "jsx"],
                ),
                module("antd", domain::FrontendModuleBinding::Host, &["Button"]),
            ],
        )
        .expect("native JSX source must resolve its host modules");

        assert_eq!(
            lock,
            serde_json::json!([
                {
                    "module_source": "antd",
                    "module_version": "1.0.0",
                    "binding": "host",
                    "assets": [],
                    "exports": ["Button"]
                },
                {
                    "module_source": "react",
                    "module_version": "1.0.0",
                    "binding": "host",
                    "assets": [],
                    "exports": ["default", "jsx"]
                }
            ])
        );
    }

    #[test]
    fn rejects_a_runtime_named_import_absent_from_the_catalog_module() {
        let error = dependency_lock_from_source(
            "import { ReloadOutlined } from '@ant-design/icons';\nexport default () => null;",
            vec![module(
                "@ant-design/icons",
                domain::FrontendModuleBinding::Fetched,
                &["SearchOutlined"],
            )],
        )
        .expect_err("unknown runtime exports must be rejected while saving");

        assert!(matches!(
            error.downcast_ref::<ControlPlaneError>(),
            Some(ControlPlaneError::InvalidComponentModuleExport {
                module_source,
                export_name
            }) if module_source == "@ant-design/icons" && export_name == "ReloadOutlined"
        ));
    }

    #[test]
    fn keeps_type_only_imports_out_of_runtime_export_validation() {
        let lock = dependency_lock_from_source(
            "import type { BlockComponentProps } from '@1flowbase/block-sdk';\nexport default () => null;",
            vec![
                module(
                    "react",
                    domain::FrontendModuleBinding::Host,
                    &["default"],
                ),
                module(
                    "@1flowbase/block-sdk",
                    domain::FrontendModuleBinding::Fetched,
                    &["blockSdkVersion"],
                ),
            ],
        )
        .expect("type-only imports must not require a runtime export");

        assert_eq!(
            lock.as_array()
                .expect("canonical dependency lock is an array")
                .iter()
                .map(|entry| entry["module_source"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["@1flowbase/block-sdk", "react"]
        );
    }
}
