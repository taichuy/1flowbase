use std::fs;

use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    load_frontend_module_asset, parse_plugin_manifest, FrontendModuleAssetManifest,
    FrontendModuleAssetRoleManifest,
};

fn manifest_with_modules(modules: &str) -> String {
    format!(
        r#"manifest_version: 1
plugin_id: native_fixture@0.1.0
version: 0.1.0
publisher_namespace: acme
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
        exports: [{export}]
        binding: fetched
        assets:
          - path: "{path}"
            role: browser_module
            media_type: "text/javascript; charset=utf-8"
            sha256: "{sha256}"
        type_declarations: "declare module '{source}' {{}}"
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
    assert!(
        duplicate
            .to_string()
            .contains("code_modules[].source must be unique"),
        "{duplicate}"
    );

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
            .contains("must be a JavaScript export name")
    );

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

    let default_export = module.replace("exports: [Fixture]", "exports: [default]");
    assert!(parse_plugin_manifest(&manifest_with_modules(&default_export)).is_ok());
}

#[test]
fn ac_pub_006_manifest_distinguishes_host_and_fetched_module_assets() {
    let digest = "a".repeat(64);
    let fetched = valid_module(
        "@acme/native",
        "1.0.0",
        "assets/native.js",
        &digest,
        "Fixture",
    );
    assert!(parse_plugin_manifest(&manifest_with_modules(&fetched)).is_ok());

    let fetched_with_support = fetched.replace(
        "        type_declarations:",
        &format!(
            "          - path: \"assets/runtime.wasm\"\n            role: support\n            media_type: \"application/wasm\"\n            sha256: \"{}\"\n        type_declarations:",
            "b".repeat(64)
        ),
    );
    assert!(parse_plugin_manifest(&manifest_with_modules(&fetched_with_support)).is_ok());

    let invalid_media_type = fetched.replace("text/javascript; charset=utf-8", "not-a-media-type");
    assert!(
        parse_plugin_manifest(&manifest_with_modules(&invalid_media_type))
            .unwrap_err()
            .to_string()
            .contains("media_type must be a valid MIME type")
    );

    let host = fetched
        .replace("@acme/native", "react")
        .replace("binding: fetched", "binding: host")
        .replace(
            &format!(
                "        assets:\n          - path: \"assets/native.js\"\n            role: browser_module\n            media_type: \"text/javascript; charset=utf-8\"\n            sha256: \"{digest}\"\n"
            ),
            "",
        );
    assert!(parse_plugin_manifest(&manifest_with_modules(&host)).is_ok());

    let unknown_host = host.replace("source: \"react\"", "source: \"lodash\"");
    assert!(parse_plugin_manifest(&manifest_with_modules(&unknown_host))
        .unwrap_err()
        .to_string()
        .contains("not part of the Host ABI"));

    let host_with_asset = fetched.replace("binding: fetched", "binding: host");
    assert!(
        parse_plugin_manifest(&manifest_with_modules(&host_with_asset))
            .unwrap_err()
            .to_string()
            .contains("host frontend modules must not register fetched assets")
    );

    let fetched_without_browser = fetched
        .replace("role: browser_module", "role: shadow_style")
        .replace("text/javascript; charset=utf-8", "text/css; charset=utf-8");
    assert!(
        parse_plugin_manifest(&manifest_with_modules(&fetched_without_browser))
            .unwrap_err()
            .to_string()
            .contains("require exactly one browser_module asset")
    );
}

#[test]
fn d2_ac_004_registered_asset_requires_existing_digest_matching_bytes() {
    let root = std::env::temp_dir().join(format!("1flowbase-module-asset-{}", Uuid::now_v7()));
    fs::create_dir_all(root.join("assets")).unwrap();
    let bytes = b"export function Fixture() {}\n";
    fs::write(root.join("assets/native.js"), bytes).unwrap();
    let sha256 = format!("{:x}", base16ct::HexDisplay(&Sha256::digest(bytes)));

    let valid = FrontendModuleAssetManifest {
        path: "assets/native.js".into(),
        role: FrontendModuleAssetRoleManifest::BrowserModule,
        media_type: "text/javascript; charset=utf-8".into(),
        sha256: sha256.clone(),
    };
    assert_eq!(load_frontend_module_asset(&root, &valid).unwrap(), bytes);

    let mismatch = FrontendModuleAssetManifest {
        path: "assets/native.js".into(),
        role: FrontendModuleAssetRoleManifest::BrowserModule,
        media_type: "text/javascript; charset=utf-8".into(),
        sha256: "a".repeat(64),
    };
    assert!(load_frontend_module_asset(&root, &mismatch)
        .unwrap_err()
        .to_string()
        .contains("SHA-256 mismatch"));

    let missing = FrontendModuleAssetManifest {
        path: "assets/missing.js".into(),
        role: FrontendModuleAssetRoleManifest::BrowserModule,
        media_type: "text/javascript; charset=utf-8".into(),
        sha256,
    };
    assert!(load_frontend_module_asset(&root, &missing)
        .unwrap_err()
        .to_string()
        .contains("is unavailable"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn settings_pages_reuse_canonical_module_validation_and_asset_discovery() {
    let raw = include_str!("../../../../plugins/fixtures/northwind.settings-page/manifest.yaml");
    let old = parse_plugin_manifest(raw).unwrap();
    assert!(old.settings_pages[0].code_modules.is_empty());
    let root = std::env::temp_dir().join(format!("settings-module-{}", Uuid::now_v7()));
    fs::create_dir_all(root.join("assets")).unwrap();
    let bytes = b"export function Fixture() {}\n";
    fs::write(root.join("assets/native.js"), bytes).unwrap();
    let digest = format!("{:x}", base16ct::HexDisplay(&Sha256::digest(bytes)));
    let module = valid_module(
        "@fixture/native",
        "1.0.0",
        "assets/native.js",
        &digest,
        "Fixture",
    );
    let raw = format!("{raw}    code_modules:\n{module}");
    let manifest = parse_plugin_manifest(&raw).unwrap();
    assert_eq!(manifest.settings_pages[0].code_modules.len(), 1);
    crate::validate_frontend_module_assets(&root, &manifest).unwrap();
    assert!(parse_plugin_manifest(&raw.replace("assets/native.js", "../native.js")).is_err());
    assert!(
        parse_plugin_manifest(&raw.replace("exports: [Fixture]", "exports: [bad-export]")).is_err()
    );
    assert!(parse_plugin_manifest(&raw.replace("binding: fetched", "binding: host")).is_err());
    fs::write(root.join("assets/native.js"), b"tampered").unwrap();
    assert!(crate::validate_frontend_module_assets(&root, &manifest).is_err());
    fs::remove_file(root.join("assets/native.js")).unwrap();
    assert!(crate::validate_frontend_module_assets(&root, &manifest).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn shared_sibling_modules_preserve_independent_authority_and_reject_conflicts() {
    let module = valid_module(
        "@acme/shared",
        "1.0.0",
        "assets/shared.js",
        &"a".repeat(64),
        "Fixture",
    );
    let raw = manifest_with_modules(&module);
    let block_start = raw.find("  - contribution_code: native").unwrap();
    let sibling =
        raw[block_start..].replace("contribution_code: native", "contribution_code: sibling");
    let shared = format!("{raw}{sibling}");
    assert_eq!(
        parse_plugin_manifest(&shared)
            .unwrap()
            .block_contributions
            .len(),
        2
    );
    let conflicting = sibling.replace("version: \"1.0.0\"", "version: \"2.0.0\"");
    assert_ne!(
        conflicting, sibling,
        "fixture must change the shared version"
    );
    assert!(parse_plugin_manifest(&format!("{raw}{conflicting}"))
        .unwrap_err()
        .to_string()
        .contains("shared frontend module declarations must be identical"));
}

#[test]
fn shared_host_bindings_use_the_existing_native_dependency_abi_only() {
    for source in ["react", "antd", "antd-style", "dayjs", "@ant-design/icons"] {
        let module = format!("      - source: '{source}'\n        version: '1.0.0'\n        exports: [default]\n        binding: host\n        type_declarations: \"declare module '{source}' {{}}\"\n");
        assert!(
            parse_plugin_manifest(&manifest_with_modules(&module)).is_ok(),
            "{source}"
        );
    }
    let forbidden = "      - source: '@unregistered/dependency'\n        version: '1.0.0'\n        exports: [default]\n        binding: host\n        type_declarations: \"declare module '@unregistered/dependency' {}\"\n";
    assert!(parse_plugin_manifest(&manifest_with_modules(forbidden))
        .unwrap_err()
        .to_string()
        .contains("not part of the Host ABI"));
}
