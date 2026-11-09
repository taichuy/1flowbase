use super::super::*;
use super::*;
use crate::openapi_interface::contract::{self as ep, ApiInput, ApiOperationContext, ApiOutput};
use crate::routes::mcp_management::{McpClientCredentialResponse, SaveMcpClientCredentialBody};
use serde::Serialize;
use serde_json::{json, Value};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(paths(
    crate::routes::mcp_management::bundles::list_official_bundles,
    crate::routes::mcp_management::bundles::preview_official_bundle,
    crate::routes::mcp_management::bundles::import_official_bundle,
    crate::routes::mcp_management::bundles::export_bundle,
    crate::routes::mcp_management::bundles::export_bundle_defaults,
    crate::routes::mcp_management::bundles::export_instance_bundle,
    crate::routes::mcp_management::bundles::preview_uploaded_bundle,
    crate::routes::mcp_management::bundles::import_uploaded_bundle,
    crate::routes::mcp_management::bundles::switch_library_bundle,
    crate::routes::mcp_management::bundles::delete_library_bundle_release,
    crate::routes::mcp_management::bundles::repair_library_bundle_release,
    crate::routes::mcp_management::get_mcp_client_credential,
    crate::routes::mcp_management::save_mcp_client_credential,
    crate::routes::mcp_management::delete_mcp_client_credential
))]
struct McpBundleApiDocs;

fn assert_valid<T: ToSchema>(wire: &Value) {
    assert!(jsonschema::validator_for(&ep::schema::<T>())
        .unwrap()
        .is_valid(wire));
}

fn assert_owner_fields<T: ToSchema>(actual: &impl Serialize) -> Value {
    let wire = serde_json::to_value(actual).unwrap();
    let schema = ep::schema::<T>();
    assert_eq!(
        wire.as_object()
            .unwrap()
            .keys()
            .collect::<std::collections::BTreeSet<_>>(),
        schema["properties"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<std::collections::BTreeSet<_>>()
    );
    assert_valid::<T>(&wire);
    wire
}

#[test]
fn mcp_missing_openapi_operations_preserve_identities_statuses_and_binary_media() {
    let spec = serde_json::to_value(McpBundleApiDocs::openapi()).unwrap();
    let expected = [
        ("get","/api/console/mcp/bundles/official","mcp.bundles.official.list","200"),
        ("post","/api/console/mcp/bundles/preview-official","preview_official_mcp_bundle","200"),
        ("post","/api/console/mcp/bundles/import-official","import_official_mcp_bundle","200"),
        ("post","/api/console/mcp/bundles/export","export_mcp_bundle","200"),
        ("get","/api/console/mcp/bundles/export-defaults","get_mcp_bundle_export_defaults","200"),
        ("post","/api/console/mcp/instances/{instance_id}/bundles/export","mcp.instances.export","200"),
        ("post","/api/console/mcp/bundles/preview-upload","preview_uploaded_mcp_bundle","200"),
        ("post","/api/console/mcp/bundles/import-upload","import_uploaded_mcp_bundle","200"),
        ("post","/api/console/mcp/bundles/library/{organization}/{bundle_id}/current/{bundle_version}","mcp.bundle_library.current.switch","200"),
        ("delete","/api/console/mcp/bundles/library/{organization}/{bundle_id}/releases/{bundle_version}","mcp.bundle_library.releases.delete","200"),
        ("post","/api/console/mcp/bundles/library/{organization}/{bundle_id}/releases/{bundle_version}/repair","mcp.bundle_library.releases.repair","200"),
        ("get","/api/console/mcp/instances/{instance_id}/client-credential","mcp.client_credential.reveal","200"),
        ("put","/api/console/mcp/instances/{instance_id}/client-credential","mcp.client_credential.save","200"),
        ("delete","/api/console/mcp/instances/{instance_id}/client-credential","mcp.client_credential.delete","204"),
    ];
    let count = spec["paths"]
        .as_object()
        .unwrap()
        .values()
        .map(|path| {
            path.as_object()
                .unwrap()
                .keys()
                .filter(|method| {
                    matches!(
                        method.as_str(),
                        "get" | "post" | "put" | "patch" | "delete" | "head" | "options" | "trace"
                    )
                })
                .count()
        })
        .sum::<usize>();
    assert_eq!(count, expected.len());
    for (method, path, identity, status) in expected {
        let operation = &spec["paths"][path][method];
        assert_eq!(operation["operationId"], identity);
        assert!(operation["summary"].is_string());
        assert!(operation["description"].is_string());
        assert!(operation["responses"].get(status).is_some());
    }
    for path in [
        "/api/console/mcp/bundles/export",
        "/api/console/mcp/instances/{instance_id}/bundles/export",
    ] {
        let response = &spec["paths"][path]["post"]["responses"]["200"]["content"];
        assert!(response.get("application/json").is_none());
        assert_eq!(response["application/zip"]["schema"]["type"], "string");
        assert_eq!(response["application/zip"]["schema"]["format"], "binary");
    }
    for path in [
        "/api/console/mcp/bundles/preview-upload",
        "/api/console/mcp/bundles/import-upload",
    ] {
        let content = &spec["paths"][path]["post"]["requestBody"]["content"];
        assert!(content.get("application/json").is_none());
        let schema = &content["multipart/form-data"]["schema"];
        assert_eq!(schema["properties"]["file"]["type"], "string");
        assert_eq!(schema["properties"]["file"]["format"], "binary");
        assert!(schema["required"]
            .as_array()
            .unwrap()
            .contains(&json!("file")));
    }
    let credential_delete =
        &spec["paths"]["/api/console/mcp/instances/{instance_id}/client-credential"]["delete"];
    assert!(credential_delete["responses"]["204"]
        .get("content")
        .is_none());
    assert!(credential_delete.get("requestBody").is_none());
    let defaults = &spec["paths"]["/api/console/mcp/bundles/export-defaults"]["get"];
    assert!(defaults.get("requestBody").is_none());
    assert!(
        spec["paths"]["/api/console/mcp/bundles/import-official"]["post"]["responses"]["409"]
            ["content"]["application/json"]["schema"]
            .is_object()
    );
}

fn preview() -> domain::McpBundlePreview {
    domain::McpBundlePreview {
        manifest: domain::McpBundleManifest {
            schema_version: "1flowbase.mcp-bundle/v1".into(),
            organization: "example".into(),
            bundle_id: "actual-bundle".into(),
            bundle_version: "1.0.0".into(),
            locale: "en-US".into(),
            minimum_host_version: "1.0.0".into(),
            exported_from_system_version: "1.0.0".into(),
            exported_at: "2026-11-08T00:00:00Z".into(),
            files: vec![],
        },
        current_system_version: "1.0.0".into(),
        version_status: domain::McpBundleVersionStatus::SameSystemVersion,
        effect_summary: Default::default(),
        tools: vec![],
        instances: vec![],
        connections: vec![],
        shared_tool_impacts: vec![],
    }
}

#[test]
fn mcp_bundle_document_markers_match_real_catalog_and_integrity_wire_fields() {
    let catalog = crate::official_mcp_bundles::OfficialMcpBundleCatalogSnapshot {
        source: crate::official_mcp_bundles::OfficialMcpBundleCatalogSource {
            source_kind: "catalog".into(),
            source_label: "Actual catalog".into(),
            catalog_url: "https://example.test/catalog.json".into(),
        },
        entries: (0..40)
            .map(
                |index| crate::official_mcp_bundles::OfficialMcpBundleCatalogEntry {
                    organization: "example".into(),
                    bundle_id: format!("bundle-{index}"),
                    latest_version: "1.0.0".into(),
                    locale: "en-US".into(),
                    minimum_host_version: "1.0.0".into(),
                    exported_from_system_version: "1.0.0".into(),
                    release_tag: "v1".into(),
                    download_url: format!("https://example.test/bundle-{index}.zip"),
                    artifact_sha256: Some("actual-checksum".into()),
                },
            )
            .collect(),
    };
    let wire = assert_owner_fields::<OfficialMcpBundleCatalogSnapshotDocument>(&catalog);
    assert_eq!(wire["entries"].as_array().unwrap().len(), 40);
    assert_owner_fields::<OfficialMcpBundleCatalogEntryDocument>(&catalog.entries[39]);
    let warning = domain::ExtensionIntegrityWarning {
        code: "unsigned".into(),
        message: "Actual warning".into(),
        overridable: true,
    };
    let compatibility = domain::ExtensionCompatibilityWarning {
        reason: "minimum_host_version".into(),
        current_host_version: "1.0.0".into(),
        minimum_host_version: "2.0.0".into(),
    };
    assert_owner_fields::<McpBundleIntegrityWarningDocument>(&warning);
    assert_owner_fields::<McpBundleCompatibilityWarningDocument>(&compatibility);
    let challenge = domain::ExtensionRiskChallenge {
        warnings: vec![warning.clone()],
        compatibility: Some(compatibility),
    };
    assert_owner_fields::<McpBundleRiskChallengeDocument>(&challenge);
    let response = InstalledMcpExtensionPreviewResponse {
        extension_installation_id: "installation-1".into(),
        artifact_installation_status: "installed".into(),
        workspace_application_status: "ready_to_import".into(),
        integrity_warnings: vec![warning.clone()],
        required_integrity_override: Some(challenge.clone()),
        preview: preview(),
    };
    let wire =
        serde_json::to_value(McpBundlePreviewSourceResponse::InstalledExtension(response)).unwrap();
    assert_valid::<McpBundlePreviewSourceResponse>(&wire);
    for response in [
        McpBundlePreviewSourceResponse::OfficialCatalog(preview()),
        McpBundlePreviewSourceResponse::BuiltinTemplate(BuiltinMcpTemplatePreviewResponse {
            builtin_template_id: "template-1".into(),
            workspace_application_status: "ready_to_import".into(),
            preview: preview(),
        }),
    ] {
        assert_valid::<McpBundlePreviewSourceResponse>(&serde_json::to_value(response).unwrap());
    }
    let source_preview = preview();
    let report = domain::McpBundleImportReport {
        manifest: source_preview.manifest,
        current_system_version: source_preview.current_system_version,
        version_status: source_preview.version_status,
        status: "imported".into(),
        effect_summary: source_preview.effect_summary,
        tools: source_preview.tools,
        instances: source_preview.instances,
        connections: source_preview.connections,
    };
    for response in [
        McpBundleImportSourceResponse::OfficialCatalog(report.clone()),
        McpBundleImportSourceResponse::InstalledExtension(InstalledMcpExtensionImportResponse {
            extension_installation_id: "installation-1".into(),
            artifact_installation_status: "installed".into(),
            workspace_application_status: "imported".into(),
            integrity_warnings: vec![warning.clone()],
            import_report: report.clone(),
        }),
        McpBundleImportSourceResponse::BuiltinTemplate(BuiltinMcpTemplateImportResponse {
            builtin_template_id: "template-1".into(),
            workspace_application_status: "imported".into(),
            import_report: report,
        }),
    ] {
        assert_valid::<McpBundleImportSourceResponse>(&serde_json::to_value(response).unwrap());
    }

    let conflict = InstalledMcpExtensionIntegrityChallengeResponse {
        status: 409,
        code: "mcp_bundle_integrity_confirmation_required".into(),
        message: "Actual integrity confirmation".into(),
        extension_installation_id: "installation-1".into(),
        artifact_installation_status: "installed".into(),
        workspace_application_status: "not_imported".into(),
        integrity_warnings: vec![warning],
        required_integrity_override: challenge,
        preview: preview(),
    };
    assert_owner_fields::<InstalledMcpExtensionIntegrityChallengeResponse>(&conflict);
    assert_owner_fields::<McpBundleLibraryDeleteResponse>(&json!({"deleted":true}));
}

#[test]
fn mcp_source_selector_schemas_accept_each_real_typed_input_shape() {
    for wire in [
        json!({"organization":"example","bundle_id":"actual-bundle"}),
        json!({"extension_installation_id":"installation-1","instance_id":"instance-1","integrity_override":{"reason":"Actual acknowledgement","acknowledged_warnings":["unsigned"]}}),
        json!({"builtin_template_id":"template-1","instance_id":"instance-1"}),
    ] {
        let _: McpBundleSourceBody = serde_json::from_value(wire.clone()).unwrap();
        assert_valid::<McpBundleSourceBody>(&wire);
    }
    let export:ExportMcpInstanceBundleBody=serde_json::from_value(json!({"organization":"example","bundle_id":"actual-bundle","bundle_version":"1.0.0","locale":"en-US","export_profile":"portable"})).unwrap();
    assert_valid::<ExportMcpInstanceBundleBody>(&serde_json::to_value(export).unwrap());
}

#[test]
fn mcp_credential_wire_schema_and_existing_event_safe_views_stay_separate() {
    use crate::routes::mcp_management::interface_core::{McpCoreInput, McpCoreOutput};
    const CREDENTIAL: &str = "MCP_DOCUMENT_WIRE_CREDENTIAL_CANARY";
    let body_wire = json!({"api_key":CREDENTIAL});
    let body: SaveMcpClientCredentialBody = serde_json::from_value(body_wire.clone()).unwrap();
    assert_valid::<SaveMcpClientCredentialBody>(&body_wire);
    let context = ApiOperationContext {
        interface_id: "mcp.client_credential.save".into(),
        method: "PUT".into(),
        path: "/api/console/mcp/instances/{instance_id}/client-credential".into(),
    };
    let event = McpCoreInput::SaveCredential("instance-1".into(), body)
        .project_document_input(&context)
        .unwrap();
    assert_eq!(event["body"], json!({}));
    assert!(!event.to_string().contains(CREDENTIAL));
    let response = McpClientCredentialResponse {
        saved: true,
        api_key: Some(CREDENTIAL.into()),
    };
    let wire = serde_json::to_value(&response).unwrap();
    assert_eq!(wire["api_key"], CREDENTIAL);
    assert_valid::<McpClientCredentialResponse>(&wire);
    let event = McpCoreOutput::Credential(response)
        .project_document_output(&context)
        .unwrap();
    assert_eq!(event, json!({"saved":true}));
    assert!(!event.to_string().contains(CREDENTIAL));
}
