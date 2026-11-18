//! Root #2824 equivalence fixture. The same frozen state owns every exported plane.
//! No authenticated endpoint is called: `/openapi.json` is public. The existing
//! test state owns an exclusive temporary PostgreSQL database and its cleanup.
use std::{collections::BTreeMap, path::Path};

use anyhow::{ensure, Context, Result};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use control_plane::ports::ApplicationPublicationRepository;
use serde_json::{json, Value};
use tower::ServiceExt;
use utoipa::OpenApi;
#[path = "crate_split_contract/data.rs"]
mod data;

/// Object keys alone are canonicalized. Arrays, nulls, and dynamic fields remain intact.
fn canonical(value: Value) -> Value {
    match value {
        Value::Object(entries) => Value::Object(
            entries
                .into_iter()
                .map(|(key, value)| (key, canonical(value)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        Value::Array(entries) => Value::Array(entries.into_iter().map(canonical).collect()),
        value => value,
    }
}

/// Exported documents intentionally exclude nondeterministic fixture *metadata*,
/// never fields in the mount, Catalog, inventory, or either OpenAPI document.
pub async fn capture() -> Result<BTreeMap<&'static str, Value>> {
    let (initial_state, _) = super::support::test_api_state_with_database_url().await;
    let publication_data = data::seed_publication(&initial_state).await?;
    let config = super::support::test_config();
    let initial_router = crate::app_with_state_and_config(initial_state.clone(), &config);
    // Reuse the real hot-generation fixture path, including registry compilation,
    // managed OpenAPI, complete host mount, and published Catalog cross-checks.
    let publisher = crate::managed_publication::ManagedGenerationPublisher::new(
        &initial_state,
        config,
        vec![],
        initial_router,
    )?;
    let service = data::managed_service();
    let managed_data = serde_json::to_value(&service.declaration)?;
    let prepared = publisher.prepare(vec![service], BTreeMap::new()).await?;
    // Captured by the production prepare path from the very same router it publishes.
    // Its historical console_router_with_assembly name covers the complete host:
    // public/native/SSE/WebSocket, logs/OTLP, OAuth, MCP/WebMCP, console/runtime,
    // public auth, branding, collectors, files, health, OpenAPI, and docs.
    let mounted = serde_json::to_value(&prepared.mounted)?;
    publisher.publish(prepared);
    let lease = publisher
        .current_generation()
        .context("missing published generation")?;
    let state = lease.state;
    let boot = state
        .extension_boot_snapshot
        .as_ref()
        .context("missing frozen boot graph")?;
    ensure!(
        state
            .store
            .list_enabled_extension_publications()
            .await?
            .len()
            == 1,
        "fixed data must contain one enabled workflow publication"
    );
    ensure!(
        state.console_surface_registry.managed_services().len() == 1,
        "fixed generation must contain one managed service"
    );
    let static_openapi = serde_json::to_value(crate::openapi::ApiDoc::openapi())?;
    let catalog = boot
        .external_endpoint_catalog()
        .context("missing published Catalog")?;
    let catalog_rows = serde_json::to_value(catalog.rows().collect::<Vec<_>>())?;
    let response = publisher
        .ingress()
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())?,
        )
        .await?;
    ensure!(
        response.status() == StatusCode::OK,
        "dynamic OpenAPI failed: {}",
        response.status()
    );
    let dynamic_openapi: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await?)?;
    ensure!(
        dynamic_openapi["paths"]
            .as_object()
            .context("dynamic paths")?
            .len()
            > static_openapi["paths"]
                .as_object()
                .context("static paths")?
                .len(),
        "dynamic runtime-template projection must be present"
    );
    ensure!(
        dynamic_openapi["paths"]["/api/console/managed-services/fixture/items"]["get"]
            ["operationId"]
            == "fixture.list",
        "managed OpenAPI append path must execute"
    );
    ensure!(
        dynamic_openapi["paths"]["/api/ex/crate-split/orders/{order_id}"]["post"].is_object(),
        "published workflow OpenAPI append path must execute"
    );
    let documents = BTreeMap::from([
        (
            "context.json",
            json!({
                "fixture": "root-2824-published-managed-v1",
                "graph_fingerprint": boot.graph().fingerprint().as_str(),
                "cookie_name": state.cookie_name,
                "workflow_publications": [publication_data],
                "managed_services": [managed_data],
                "runtime_template_catalog": "compiled provider runtime template catalog",
                "mount_owner": "api-server::console_router_with_assembly (complete host)",
                "include_docs": true
            }),
        ),
        ("http-mounts.json", mounted),
        (
            "effective-plan.json",
            serde_json::to_value(boot.effective_plan())?,
        ),
        (
            "compiled-console-operation-snapshot.json",
            serde_json::to_value(
                boot.console_operation_snapshot_for_test_support()
                    .context("missing compiled console operation snapshot")?,
            )?,
        ),
        ("endpoint-catalog.json", catalog_rows),
        (
            "console-operation-inventory.json",
            serde_json::to_value(state.console_operation_registry.inventory())?,
        ),
        (
            "core-console-operation-snapshot.json",
            serde_json::to_value(
                crate::app_state::compile_core_console_operation_inventory_snapshot()?,
            )?,
        ),
        ("openapi-static.json", static_openapi),
        ("openapi-dynamic.json", dynamic_openapi),
    ]);
    Ok(documents
        .into_iter()
        .map(|(name, value)| (name, canonical(value)))
        .collect())
}

pub fn write(directory: &Path, documents: &BTreeMap<&str, Value>) -> Result<()> {
    std::fs::create_dir_all(directory)?;
    for (name, value) in documents {
        std::fs::write(directory.join(name), serde_json::to_vec_pretty(value)?)?;
    }
    Ok(())
}

pub fn compare(directory: &Path, documents: &BTreeMap<&str, Value>) -> Result<()> {
    for (name, actual) in documents {
        let path = directory.join(name);
        let expected: Value = serde_json::from_slice(
            &std::fs::read(&path).with_context(|| format!("read golden {}", path.display()))?,
        )?;
        ensure!(
            canonical(expected) == *actual,
            "contract differs from golden: {}",
            path.display()
        );
    }
    Ok(())
}
