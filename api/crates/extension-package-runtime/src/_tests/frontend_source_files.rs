use crate::{parse_plugin_manifest, resolve_plugin_frontend_sources, PluginManifestV1};
use serde_json::{json, Value};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

const SOURCE: &str = "export default function Block() { return <p>Calendar</p>; }\n";
const TYPES: &str = "declare const React: unknown; export default React;\n";

struct Package(PathBuf);

impl Package {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "flowbase-frontend-sources-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir_all(root.join("ui/types")).unwrap();
        fs::write(root.join("ui/calendar.tsx"), SOURCE).unwrap();
        fs::write(root.join("ui/types/react.d.ts"), TYPES).unwrap();
        Self(root)
    }
}

impl Drop for Package {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn declaration() -> Value {
    let mut manifest: Value = serde_yaml::from_str(include_str!("managed_manifest.yaml")).unwrap();
    manifest["block_contributions"] = json!([{
        "contribution_code": "Calendar", "title": "Calendar", "runtime": "native_react",
        "entry": "default", "ui_registration": "component", "block_registration": false,
        "code_template_file": "ui/calendar.tsx", "code_template_version": "1.0.0",
        "code_template_language": "tsx",
        "code_modules": [{ "source": "react", "version": "19.2.5", "binding": "host",
            "exports": ["default"], "type_declarations_file": "ui/types/react.d.ts" }],
        "context_contract": { "primitives": [], "input_schema": {"type": "object"} },
        "permissions": { "network": "none", "storage": "none", "secrets": "none" }
    }]);
    manifest["managed"]["module"]["contributions"].as_array_mut().unwrap().push(json!({
        "contribution_id": "managed_fixture.Calendar", "contributor_module_id": "managed_fixture",
        "point_id": "1flowbase.workspace.frontend-block-contribution", "contract_version": "1",
        "required_permissions": ["frontend-block.ui-mount.trusted-host"], "mode": "append"
    }));
    manifest["managed"]["execution_bindings"].as_array_mut().unwrap().push(json!({
        "contribution_id": "managed_fixture.Calendar", "execution_mode": "declarative_only",
        "runtime": { "protocol": "stdio_json", "entry": "bin/fixture" }, "handler": "render.Calendar",
        "payload": { "kind": "frontend_block", "contribution_code": "Calendar" }
    }));
    manifest
}

fn parse(value: &Value) -> crate::FrameworkResult<PluginManifestV1> {
    parse_plugin_manifest(&serde_yaml::to_string(value).unwrap())
}

#[test]
fn file_sources_do_not_implicitly_register_creatable_blocks() {
    let mut value = declaration();
    value["block_contributions"][0]
        .as_object_mut()
        .unwrap()
        .remove("block_registration");
    for destination in ["component", "code_template"] {
        value["block_contributions"][0]["ui_registration"] = json!(destination);
        assert!(!parse(&value).unwrap().block_contributions[0].block_registration);
    }
    value["block_contributions"][0]["block_registration"] = json!(true);
    assert!(parse(&value).unwrap().block_contributions[0].block_registration);
}

#[test]
fn file_references_resolve_to_owned_snapshots_without_changing_registration() {
    let package = Package::new();
    let value = declaration();
    let raw = serde_yaml::to_string(&value).unwrap();
    fs::write(package.0.join("manifest.yaml"), &raw).unwrap();
    let mut manifest = parse(&value).unwrap();
    assert!(manifest.block_contributions[0].code_template.is_none());
    resolve_plugin_frontend_sources(&package.0, &mut manifest).unwrap();
    let block = &manifest.block_contributions[0];
    assert_eq!(block.code_template.as_deref(), Some(SOURCE));
    assert_eq!(block.code_modules[0].type_declarations, TYPES);
    assert!(block.code_template_file.is_none());
    assert!(block.code_modules[0].type_declarations_file.is_none());
    assert_eq!(block.ui_registration, "component");
    assert!(!block.block_registration);
    assert_eq!(
        fs::read_to_string(package.0.join("manifest.yaml")).unwrap(),
        raw
    );
    fs::remove_dir_all(package.0.join("ui")).unwrap();
    assert_eq!(
        manifest.block_contributions[0].code_template.as_deref(),
        Some(SOURCE)
    );
}

#[test]
fn settings_page_modules_use_the_same_package_type_files() {
    let package = Package::new();
    let mut manifest = parse(&declaration()).unwrap();
    manifest
        .settings_pages
        .push(crate::PluginSettingsPageManifest {
            feature_id: "managed_fixture.settings".into(),
            contribution_code: "settings".into(),
            source_file: "ui/calendar.tsx".into(),
            language: "tsx".into(),
            code_modules: manifest.block_contributions[0].code_modules.clone(),
        });
    resolve_plugin_frontend_sources(&package.0, &mut manifest).unwrap();
    let page = &manifest.settings_pages[0];
    assert_eq!(page.code_modules[0].type_declarations, TYPES);
    assert!(page.code_modules[0].type_declarations_file.is_none());
    assert_eq!(
        crate::read_plugin_settings_page_source(&package.0, page).unwrap(),
        SOURCE
    );
}

#[test]
fn inline_sources_remain_valid_but_cannot_be_combined_with_file_references() {
    let mut value = declaration();
    value["block_contributions"][0]["code_template"] = json!(SOURCE);
    assert!(parse(&value)
        .unwrap_err()
        .to_string()
        .contains("mutually exclusive"));
    value["block_contributions"][0]
        .as_object_mut()
        .unwrap()
        .remove("code_template_file");
    value["block_contributions"][0]["code_modules"][0]["type_declarations"] = json!(TYPES);
    assert!(parse(&value)
        .unwrap_err()
        .to_string()
        .contains("mutually exclusive"));
    value["block_contributions"][0]["code_modules"][0]
        .as_object_mut()
        .unwrap()
        .remove("type_declarations_file");
    let mut manifest = parse(&value).unwrap();
    // Inline-only packages do not need a source-file root.
    resolve_plugin_frontend_sources(PathBuf::from("missing-package").as_path(), &mut manifest)
        .unwrap();
    assert_eq!(
        manifest.block_contributions[0].code_template.as_deref(),
        Some(SOURCE)
    );
}

#[test]
fn malformed_paths_language_and_missing_source_metadata_are_rejected() {
    for path in [
        "/tmp/outside.tsx",
        "../outside.tsx",
        "ui/../outside.tsx",
        "C:/outside.tsx",
        "ui\\outside.tsx",
        "",
        "ui//calendar.tsx",
        "ui/./calendar.tsx",
        "ui/calendar.jsx",
    ] {
        let mut value = declaration();
        value["block_contributions"][0]["code_template_file"] = json!(path);
        assert!(parse(&value).is_err(), "accepted {path}");
    }
    for path in [
        "../types.d.ts",
        "/tmp/types.d.ts",
        "ui/types.ts",
        "ui/types.txt",
    ] {
        let mut value = declaration();
        value["block_contributions"][0]["code_modules"][0]["type_declarations_file"] = json!(path);
        assert!(parse(&value).is_err(), "accepted {path}");
    }
    for key in ["code_template_version", "code_template_language"] {
        let mut value = declaration();
        value["block_contributions"][0]
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert!(parse(&value).is_err(), "accepted missing {key}");
    }
}

#[test]
fn missing_non_file_empty_invalid_utf8_and_oversized_sources_fail_loading() {
    let package = Package::new();
    for bytes in [Vec::new(), b" \n".to_vec(), vec![0xff], vec![b'x'; 262145]] {
        fs::write(package.0.join("ui/calendar.tsx"), bytes).unwrap();
        let mut manifest = parse(&declaration()).unwrap();
        assert!(resolve_plugin_frontend_sources(&package.0, &mut manifest).is_err());
    }
    fs::remove_file(package.0.join("ui/calendar.tsx")).unwrap();
    let mut manifest = parse(&declaration()).unwrap();
    assert!(resolve_plugin_frontend_sources(&package.0, &mut manifest).is_err());
    fs::create_dir(package.0.join("ui/calendar.tsx")).unwrap();
    assert!(resolve_plugin_frontend_sources(&package.0, &mut manifest).is_err());
    fs::remove_dir(package.0.join("ui/calendar.tsx")).unwrap();
    fs::write(package.0.join("ui/calendar.tsx"), SOURCE).unwrap();
    for bytes in [Vec::new(), vec![0xff], vec![b'x'; 2 * 1024 * 1024 + 1]] {
        fs::write(package.0.join("ui/types/react.d.ts"), bytes).unwrap();
        let mut manifest = parse(&declaration()).unwrap();
        assert!(resolve_plugin_frontend_sources(&package.0, &mut manifest).is_err());
    }
}

#[cfg(unix)]
#[test]
fn symlinks_cannot_escape_the_package_for_templates_or_types() {
    let package = Package::new();
    let outside = Package::new();
    for path in ["ui/calendar.tsx", "ui/types/react.d.ts"] {
        fs::remove_file(package.0.join(path)).unwrap();
        std::os::unix::fs::symlink(outside.0.join(path), package.0.join(path)).unwrap();
        let mut manifest = parse(&declaration()).unwrap();
        assert!(resolve_plugin_frontend_sources(&package.0, &mut manifest).is_err());
        fs::remove_file(package.0.join(path)).unwrap();
        fs::write(
            package.0.join(path),
            if path.ends_with(".d.ts") {
                TYPES
            } else {
                SOURCE
            },
        )
        .unwrap();
    }
}
