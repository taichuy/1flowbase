use axum::{routing::get, Router};
use base64::{engine::general_purpose::STANDARD, Engine};
use ed25519_dalek::{pkcs8::EncodePublicKey, Signer, SigningKey};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

use crate::{
    config::ResolvedOfficialAgentFlowTemplateSourceConfig,
    official_agent_flow_templates::{AgentFlowTemplateLibraryPort, ApiAgentFlowTemplateLibrary},
};

#[tokio::test]
async fn agent_flow_library_syncs_signed_releases_and_manages_current_history() {
    let artifact = template_bytes();
    let signing_key = SigningKey::from_bytes(&[17; 32]);
    let signature = STANDARD.encode(signing_key.sign(&artifact).to_bytes());
    let checksum = format!("sha256:{:x}", Sha256::digest(&artifact));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let catalog = catalog_document(&base, &checksum, &signature);
    let artifact_for_server = artifact.clone();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new()
                .route(
                    "/catalog.json",
                    get(move || {
                        let catalog = catalog.clone();
                        async move { axum::Json(catalog) }
                    }),
                )
                .route(
                    "/releases/1/template.json",
                    get(move || {
                        let artifact = artifact_for_server.clone();
                        async move { artifact }
                    }),
                ),
        )
        .await
        .unwrap();
    });
    let root = temp_library_root();
    let library = ApiAgentFlowTemplateLibrary::new(
        ResolvedOfficialAgentFlowTemplateSourceConfig {
            source_kind: "official_registry".into(),
            source_label: "Official".into(),
            index_url: format!("{base}/catalog.json"),
            github_proxy_url: None,
        },
        root.clone(),
        vec![plugin_framework::TrustedPublicKey {
            key_id: "fixture-key".into(),
            algorithm: "ed25519".into(),
            public_key_pem: signing_key
                .verifying_key()
                .to_public_key_pem(Default::default())
                .unwrap(),
        }],
    );

    assert_eq!(
        library
            .resolve_artifact("support-flow", None)
            .await
            .unwrap(),
        artifact,
        "the first preview must sync remote latest into the local library"
    );
    assert_eq!(
        std::fs::read(root.join("support-flow/releases/1/template.json")).unwrap(),
        artifact
    );
    assert!(root.join("support-flow/releases/1/receipt.json").is_file());
    assert_eq!(
        std::fs::read_to_string(root.join("support-flow/current")).unwrap(),
        "1"
    );
    let catalog = library.catalog().await.unwrap();
    assert!(catalog.remote_available);
    assert_eq!(catalog.templates[0].local_versions.len(), 1);
    library.switch_current("support-flow", 1).await.unwrap();
    library.repair("support-flow", 1).await.unwrap();

    server.abort();
    assert_eq!(
        library
            .resolve_artifact("support-flow", None)
            .await
            .unwrap(),
        artifact,
        "an existing local template must preview without a remote request"
    );
    std::fs::write(
        root.join("support-flow/releases/1/template.json"),
        b"tampered",
    )
    .unwrap();
    assert!(library
        .resolve_artifact("support-flow", None)
        .await
        .is_err());
    std::fs::write(
        root.join("support-flow/releases/1/template.json"),
        &artifact,
    )
    .unwrap();
    let offline = library.catalog().await.unwrap();
    assert!(!offline.remote_available);
    assert_eq!(offline.templates[0].local_versions.len(), 1);
    let mut second_receipt: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("support-flow/releases/1/receipt.json")).unwrap(),
    )
    .unwrap();
    second_receipt["release_version"] = json!(2);
    std::fs::create_dir_all(root.join("support-flow/releases/2")).unwrap();
    std::fs::write(
        root.join("support-flow/releases/2/template.json"),
        &artifact,
    )
    .unwrap();
    std::fs::write(
        root.join("support-flow/releases/2/receipt.json"),
        serde_json::to_vec(&second_receipt).unwrap(),
    )
    .unwrap();
    library.switch_current("support-flow", 2).await.unwrap();
    library
        .delete_local_version("support-flow", 2)
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join("support-flow/current")).unwrap(),
        "1",
        "deleting current must select the highest remaining local release"
    );
    library
        .delete_local_version("support-flow", 1)
        .await
        .unwrap();
    assert!(!root.join("support-flow").exists());
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn agent_flow_library_rejects_catalog_release_checksum_changes() {
    let root = temp_library_root();
    let release = root.join("support-flow/releases/1");
    std::fs::create_dir_all(&release).unwrap();
    std::fs::write(release.join("template.json"), b"{}").unwrap();
    std::fs::write(
        release.join("receipt.json"),
        serde_json::to_vec(&json!({
            "template_id": "support-flow",
            "release_version": 1,
            "exported_from_system_version": "0.3.1",
            "exported_at": "2026-08-02T00:00:00Z",
            "application": {"name": "Support", "description": ""},
            "checksum": format!("sha256:{}", "a".repeat(64)),
            "algorithm": "ed25519",
            "key_id": "fixture-key",
            "signature": ""
        }))
        .unwrap(),
    )
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let catalog = catalog_document(&base, &format!("sha256:{}", "b".repeat(64)), "");
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route(
                "/catalog.json",
                get(move || async move { axum::Json(catalog) }),
            ),
        )
        .await
        .unwrap();
    });
    let library = ApiAgentFlowTemplateLibrary::new(
        ResolvedOfficialAgentFlowTemplateSourceConfig {
            source_kind: "official_registry".into(),
            source_label: "Official".into(),
            index_url: format!("{base}/catalog.json"),
            github_proxy_url: None,
        },
        root.clone(),
        Vec::new(),
    );
    assert!(library.sync("support-flow", Some(1)).await.is_err());
    server.abort();
    let _ = std::fs::remove_dir_all(root);
}

fn catalog_document(base: &str, checksum: &str, signature: &str) -> serde_json::Value {
    json!({
        "schema_version": "1flowbase.agent-flow-catalog/v1",
        "templates": [{
            "template_id": "support-flow",
            "source_path": "./",
            "versions": [{
                "template_id": "support-flow",
                "release_version": 1,
                "exported_from_system_version": "0.3.1",
                "exported_at": "2026-08-02T00:00:00Z",
                "application": {"name": "Support", "description": ""},
                "download_url": format!("{base}/releases/1/template.json"),
                "checksum": checksum,
                "algorithm": "ed25519",
                "key_id": "fixture-key",
                "signature": signature
            }]
        }]
    })
}

fn template_bytes() -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schema_version": "1flowbase.application-archive/v1",
        "applications": [{
            "template_id": "support-flow",
            "release_version": 1,
            "exported_from_system_version": "0.3.1",
            "exported_at": "2026-08-02T00:00:00Z",
            "application": {
                "application_type": "agent_flow",
                "workflow_trigger_type": null,
                "name": "Support",
                "description": "",
                "icon": null,
                "icon_type": null,
                "icon_background": null
            },
            "flow_document": {"graph": {"nodes": [], "edges": []}},
            "dependencies": []
        }]
    }))
    .unwrap()
}

fn temp_library_root() -> PathBuf {
    std::env::temp_dir().join(format!(
        "1flowbase-agent-flow-library-{}",
        uuid::Uuid::now_v7()
    ))
}
