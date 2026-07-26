use std::fs;

use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    load_frontend_module_asset, parse_plugin_manifest, FrontendModuleBrowserAssetManifest,
};

fn manifest_with_modules(modules: &str) -> String {
    format!(
        r#"manifest_version: 1
plugin_id: native_fixture@0.1.0
version: 0.1.0
vendor: acme
display_name: Native Fixture
description: Native React fixture
source_kind: uploaded
trust_level: checksum_only
consumption_kind: capability_plugin
execution_mode: declarative_only
slot_codes: [frontend_block]
binding_targets: [workspace]
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.capability/v1
schema_version: 1flowbase.plugin.manifest/v1
permissions: {{ network: none, secrets: none, storage: none, mcp: none, subprocess: deny }}
runtime: {{ protocol: stdio_json, entry: index.js }}
block_contributions:
  - contribution_code: native
    title: Native
    runtime: native_react
    entry: index.js
    code_modules:
{modules}
    context_contract:
      primitives: [text]
      input_schema: {{ type: object }}
    permissions: {{ network: none, storage: none, secrets: none }}
"#
    )
}

fn valid_module(source: &str, version: &str, path: &str, sha256: &str, export: &str) -> String {
    format!(
        r#"      - source: "{source}"
        version: "{version}"
        exports: [Fixture]
        browser_asset:
          path: "{path}"
          sha256: "{sha256}"
        type_declarations: "declare module '{source}' {{}}"
        components:
          - component_code: fixture
            export_name: "{export}"
            description: "Native React fixture."
            limitations: ["Host-owned React singleton."]
            insert_snippet: "<Fixture />"
"#
    )
}

#[test]
fn d2_ac_004_manifest_rejects_duplicate_identity_export_path_and_digest() {
    let digest = "a".repeat(64);
    let module = valid_module(
        "@acme/native",
        "1.0.0",
        "assets/native.js",
        &digest,
        "Fixture",
    );
    let duplicate =
        parse_plugin_manifest(&manifest_with_modules(&format!("{module}{module}"))).unwrap_err();
    assert!(duplicate
        .to_string()
        .contains("source/version must be unique"));

    let path_escape = valid_module("@acme/native", "1.0.0", "../native.js", &digest, "Fixture");
    assert!(parse_plugin_manifest(&manifest_with_modules(&path_escape))
        .unwrap_err()
        .to_string()
        .contains("must stay within the plugin package"));

    let invalid_digest = valid_module(
        "@acme/native",
        "1.0.0",
        "assets/native.js",
        "sha256-nope",
        "Fixture",
    );
    assert!(
        parse_plugin_manifest(&manifest_with_modules(&invalid_digest))
            .unwrap_err()
            .to_string()
            .contains("lowercase SHA-256")
    );

    let invalid_export = valid_module(
        "@acme/native",
        "1.0.0",
        "assets/native.js",
        &digest,
        "bad-export",
    );
    assert!(
        parse_plugin_manifest(&manifest_with_modules(&invalid_export))
            .unwrap_err()
            .to_string()
            .contains("must be a TypeScript identifier")
    );

    let empty_export = valid_module("@acme/native", "1.0.0", "assets/native.js", &digest, "");
    assert!(parse_plugin_manifest(&manifest_with_modules(&empty_export))
        .unwrap_err()
        .to_string()
        .contains("export_name cannot be empty"));

    let empty_exports = module.replace("exports: [Fixture]", "exports: []");
    assert!(
        parse_plugin_manifest(&manifest_with_modules(&empty_exports))
            .unwrap_err()
            .to_string()
            .contains("exports must not be empty")
    );

    let duplicate_exports = module.replace("exports: [Fixture]", "exports: [Fixture, Fixture]");
    assert!(
        parse_plugin_manifest(&manifest_with_modules(&duplicate_exports))
            .unwrap_err()
            .to_string()
            .contains("exports[] must be unique")
    );

    let illegal_exports = module.replace("exports: [Fixture]", "exports: [bad-export]");
    assert!(
        parse_plugin_manifest(&manifest_with_modules(&illegal_exports))
            .unwrap_err()
            .to_string()
            .contains("must be a JavaScript export name")
    );

    let missing_component_export = module.replace("exports: [Fixture]", "exports: [OtherExport]");
    assert!(
        parse_plugin_manifest(&manifest_with_modules(&missing_component_export))
            .unwrap_err()
            .to_string()
            .contains("export_name must be declared in module exports")
    );

    let default_export = module
        .replace("exports: [Fixture]", "exports: [default]")
        .replace("export_name: \"Fixture\"", "export_name: \"default\"");
    assert!(parse_plugin_manifest(&manifest_with_modules(&default_export)).is_ok());
}

#[test]
fn d2_ac_004_registered_asset_requires_existing_digest_matching_bytes() {
    let root = std::env::temp_dir().join(format!("1flowbase-module-asset-{}", Uuid::now_v7()));
    fs::create_dir_all(root.join("assets")).unwrap();
    let bytes = b"export function Fixture() {}\n";
    fs::write(root.join("assets/native.js"), bytes).unwrap();
    let sha256 = format!("{:x}", Sha256::digest(bytes));

    let valid = FrontendModuleBrowserAssetManifest {
        path: "assets/native.js".into(),
        sha256: sha256.clone(),
    };
    assert_eq!(load_frontend_module_asset(&root, &valid).unwrap(), bytes);

    let mismatch = FrontendModuleBrowserAssetManifest {
        path: "assets/native.js".into(),
        sha256: "a".repeat(64),
    };
    assert!(load_frontend_module_asset(&root, &mismatch)
        .unwrap_err()
        .to_string()
        .contains("SHA-256 mismatch"));

    let missing = FrontendModuleBrowserAssetManifest {
        path: "assets/missing.js".into(),
        sha256,
    };
    assert!(load_frontend_module_asset(&root, &missing)
        .unwrap_err()
        .to_string()
        .contains("is unavailable"));
    fs::remove_dir_all(root).unwrap();
}
