use axum::{routing::get, Router};
use base64::{engine::general_purpose::STANDARD, Engine};
use ed25519_dalek::{pkcs8::EncodePublicKey, Signer, SigningKey};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::{
    config::ResolvedOfficialMcpBundleSourceConfig,
    official_mcp_bundles::{ApiOfficialMcpBundleRegistry, OfficialMcpBundleSourcePort},
};

#[tokio::test]
async fn mcp_library_verifies_signed_releases_and_resolves_existing_local_artifact_offline() {
    // AC-004: sync writes only the fixed local library; resolve remains local-first.
    let artifact = b"fixture-mcp-bundle-zip".to_vec();
    let signing_key = SigningKey::from_bytes(&[23; 32]);
    let checksum = format!("sha256:{:x}", Sha256::digest(&artifact));
    let signature = STANDARD.encode(signing_key.sign(&artifact).to_bytes());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let catalog = catalog(&base, &checksum, &signature);
    let served_artifact = artifact.clone();
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
                    "/bundle.zip",
                    get(move || {
                        let artifact = served_artifact.clone();
                        async move { artifact }
                    }),
                ),
        )
        .await
        .unwrap();
    });
    let root = temp_root();
    let library = library(&base, root.clone(), &signing_key);

    let projected = library.library_catalog().await.unwrap();
    assert_eq!(
        projected.bundles[0].remote_versions[0].bundle_version,
        "1.10.0"
    );

    assert_eq!(
        library
            .resolve_artifact("taichuy", "zh_hans", None)
            .await
            .unwrap(),
        artifact
    );
    assert!(root
        .join("@taichuy/zh_hans/releases/1.10.0/bundle.zip")
        .is_file());
    assert_eq!(
        std::fs::read_to_string(root.join("@taichuy/zh_hans/current")).unwrap(),
        "1.10.0"
    );
    library
        .sync("taichuy", "zh_hans", Some("1.2.0"))
        .await
        .unwrap();
    let with_history = library.library_catalog().await.unwrap();
    assert_eq!(
        with_history.bundles[0].local_versions[0].bundle_version,
        "1.10.0"
    );
    library
        .switch_current("taichuy", "zh_hans", "1.10.0")
        .await
        .unwrap();
    library
        .repair("taichuy", "zh_hans", "1.10.0")
        .await
        .unwrap();
    library
        .delete_local_version("taichuy", "zh_hans", "1.2.0")
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join("@taichuy/zh_hans/current")).unwrap(),
        "1.10.0"
    );

    server.abort();
    let _ = server.await;
    assert_eq!(
        library
            .resolve_artifact("taichuy", "zh_hans", None)
            .await
            .unwrap(),
        artifact,
        "an existing current release must not require remote catalog access"
    );
    std::fs::write(
        root.join("@taichuy/zh_hans/releases/1.10.0/bundle.zip"),
        b"tampered",
    )
    .unwrap();
    assert!(library
        .resolve_artifact("taichuy", "zh_hans", None)
        .await
        .is_err());
    let _ = std::fs::remove_dir_all(root);
}

fn library(
    base: &str,
    root: std::path::PathBuf,
    signing_key: &SigningKey,
) -> ApiOfficialMcpBundleRegistry {
    ApiOfficialMcpBundleRegistry::new(
        ResolvedOfficialMcpBundleSourceConfig {
            source_kind: "official_registry".into(),
            source_label: "Official".into(),
            catalog_url: format!("{base}/catalog.json"),
            github_proxy_url: None,
        },
        root,
        vec![plugin_framework::TrustedPublicKey {
            key_id: "fixture-key".into(),
            algorithm: "ed25519".into(),
            public_key_pem: signing_key
                .verifying_key()
                .to_public_key_pem(Default::default())
                .unwrap(),
        }],
    )
}

fn catalog(base: &str, checksum: &str, signature: &str) -> serde_json::Value {
    json!({
        "schema_version": "1flowbase.mcp-catalog/v2",
        "bundles": [{
            "organization": "taichuy",
            "bundle_id": "zh_hans",
            "source_path": "./",
            "versions": [{
                "bundle_version": "1.2.0",
                "locale": "zh_Hans",
                "minimum_host_version": "0.3.1",
                "exported_from_system_version": "0.3.1",
                "release_tag": "mcp-taichuy-zh_hans-v1.1.1",
                "download_url": format!("{base}/bundle.zip"),
                "checksum": checksum,
                "algorithm": "ed25519",
                "key_id": "fixture-key",
                "signature": signature
            }, {
                "bundle_version": "1.10.0",
                "locale": "zh_Hans",
                "minimum_host_version": "0.3.1",
                "exported_from_system_version": "0.3.1",
                "release_tag": "mcp-taichuy-zh_hans-v1.10.0",
                "download_url": format!("{base}/bundle.zip"),
                "checksum": checksum,
                "algorithm": "ed25519",
                "key_id": "fixture-key",
                "signature": signature
            }]
        }]
    })
}

fn temp_root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("mcp-library-{}", uuid::Uuid::now_v7()))
}
