use axum::{
    Json, Router,
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use storage_ephemeral::MokaCacheStore;

use crate::config::ResolvedOfficialAgentFlowTemplateSourceConfig;
use crate::official_agent_flow_templates::{
    ApiOfficialAgentFlowTemplateRegistry, OfficialAgentFlowTemplateSourcePort,
};

const RAW_INDEX_URL: &str = "https://raw.githubusercontent.com/taichuy/1flowbase-official-plugins/main/agent-flow/catalog/v1/index.json";
const RAW_PAGE_URL: &str = "https://raw.githubusercontent.com/taichuy/1flowbase-official-plugins/main/agent-flow/catalog/v1/pages/1.json";
const RAW_TEMPLATE_URL: &str = "https://raw.githubusercontent.com/taichuy/1flowbase-official-plugins/main/agent-flow/workflows/multimodal-mount-test/template.json";

#[tokio::test]
async fn official_agent_flow_template_registry_uses_proxy_and_verifies_template_hash() {
    let template_bytes = template_bytes();
    let template_sha256 = format!("sha256:{:x}", Sha256::digest(&template_bytes));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_url = format!("http://{}/proxy", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, Router::new().fallback(proxy_response))
            .await
            .unwrap();
    });
    let registry = ApiOfficialAgentFlowTemplateRegistry::new(
        ResolvedOfficialAgentFlowTemplateSourceConfig {
            source_kind: "official_registry".to_string(),
            source_label: "官方源".to_string(),
            index_url: RAW_INDEX_URL.to_string(),
            github_proxy_url: Some(proxy_url.clone()),
        },
        Arc::new(MokaCacheStore::new(
            "flowbase:test:official-agent-flow-templates",
            128,
        )),
    );

    let catalog = registry.list_catalog_page(None).await.unwrap();
    let entry = catalog.entries.first().unwrap();

    assert_eq!(
        catalog.source.index_url,
        format!("{proxy_url}/{RAW_INDEX_URL}")
    );
    assert_eq!(
        catalog.page.next_cursor, None,
        "single generated page should not advertise a cursor"
    );
    assert_eq!(entry.workflow_id, "multimodal-mount-test");
    assert_eq!(
        entry.template_url,
        format!("{proxy_url}/{RAW_TEMPLATE_URL}")
    );
    assert_eq!(entry.template_sha256, template_sha256);
    assert_eq!(entry.application.name, "多模态挂载测试");

    let downloaded = registry
        .download_template("multimodal-mount-test")
        .await
        .unwrap();
    assert_eq!(downloaded.application.name, "多模态挂载测试");

    server.abort();
}

#[tokio::test]
async fn official_agent_flow_template_registry_caches_pages_in_ephemeral_cache_store() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_url = format!("http://{}/proxy", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, Router::new().fallback(proxy_response))
            .await
            .unwrap();
    });
    let cache = Arc::new(MokaCacheStore::new(
        "flowbase:test:official-agent-flow-templates",
        128,
    ));
    let registry = ApiOfficialAgentFlowTemplateRegistry::new(
        ResolvedOfficialAgentFlowTemplateSourceConfig {
            source_kind: "official_registry".to_string(),
            source_label: "官方源".to_string(),
            index_url: RAW_INDEX_URL.to_string(),
            github_proxy_url: Some(proxy_url),
        },
        cache.clone(),
    );

    let first_catalog = registry.list_catalog_page(None).await.unwrap();
    assert_eq!(
        first_catalog.entries.first().unwrap().workflow_id,
        "multimodal-mount-test"
    );
    server.abort();

    let cached_catalog = registry.list_catalog_page(None).await.unwrap();
    assert_eq!(
        cached_catalog.entries.first().unwrap().workflow_id,
        "multimodal-mount-test"
    );
    let cached_domains = control_plane::ports::CacheStore::list_cache_domains(cache.as_ref())
        .await
        .unwrap();
    let template_domain = cached_domains
        .iter()
        .find(|domain| domain.domain_code == "official-agent-flow-templates")
        .unwrap();
    assert_eq!(
        template_domain.entry_count, 2,
        "index and page should be cached as separate ephemeral cache entries"
    );
}

async fn proxy_response(uri: Uri) -> Response {
    let path = uri.path();
    if path.ends_with("index.json") {
        let page_json = serde_json::to_vec(&page_document()).unwrap();
        return Json(json!({
            "version": 1,
            "generated_at": "2026-06-16T00:00:00.000Z",
            "page_size": 100,
            "total_entries": 1,
            "first_page_url": RAW_PAGE_URL,
            "pages": [{
                "page": 1,
                "url": RAW_PAGE_URL,
                "entry_count": 1,
                "sha256": format!("sha256:{:x}", Sha256::digest(&page_json))
            }]
        }))
        .into_response();
    }
    if path.ends_with("pages/1.json") {
        return Json(page_document()).into_response();
    }
    if path.ends_with("template.json") {
        return template_bytes().into_response();
    }

    StatusCode::NOT_FOUND.into_response()
}

fn page_document() -> serde_json::Value {
    let template_bytes = template_bytes();
    json!({
        "version": 1,
        "page": 1,
        "page_size": 100,
        "next_page_url": null,
        "entries": [{
            "workflow_id": "multimodal-mount-test",
            "schema_version": "1flowbase.application-template/v1",
            "application": {
                "application_type": "agent_flow",
                "name": "多模态挂载测试",
                "description": "",
                "icon": "RobotOutlined",
                "icon_type": "iconfont",
                "icon_background": "#E6F7F2"
            },
            "template_url": RAW_TEMPLATE_URL,
            "template_sha256": format!("sha256:{:x}", Sha256::digest(&template_bytes)),
            "updated_at": "2026-06-16T00:00:00.000Z"
        }]
    })
}

fn template_bytes() -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schema_version": "1flowbase.application-template/v1",
        "application": {
            "application_type": "agent_flow",
            "name": "多模态挂载测试",
            "description": "",
            "icon": "RobotOutlined",
            "icon_type": "iconfont",
            "icon_background": "#E6F7F2"
        },
        "flow_document": {
            "schemaVersion": domain::FLOW_SCHEMA_VERSION,
            "meta": {
                "flowId": "019eb647-bee3-7ae2-a89d-5c6bca7921ad",
                "name": "多模态挂载测试",
                "description": "",
                "tags": []
            },
            "graph": {
                "nodes": [
                    {
                        "id": "node-start",
                        "type": "start",
                        "alias": "Start",
                        "config": { "input_fields": [] },
                        "outputs": [],
                        "bindings": {},
                        "position": { "x": 0, "y": 0 },
                        "containerId": null,
                        "description": "",
                        "configVersion": 1
                    }
                ],
                "edges": []
            }
        },
        "dependencies": []
    }))
    .unwrap()
}
