use anyhow::{Context, Result};
pub use api_host_kernel::managed_services::registration::*;
use control_plane_contracts::ports::PluginRepository;
use plugin_framework::PluginManifestV1;
use std::path::Path;
use storage_durable_postgres::MainDurableStore;

pub(crate) async fn load(
    store: &MainDurableStore,
    node_id: &str,
) -> Result<Vec<ManagedServiceRegistration>> {
    let installations = store
        .list_installations()
        .await?
        .into_iter()
        .filter(|installation| {
            installation.desired_state == domain::PluginDesiredState::ActiveRequested
                && domain::managed_installation_scope(installation, domain::DEFAULT_SCOPE_ID)
                    == domain::SYSTEM_SCOPE_ID
        })
        .collect::<Vec<_>>();
    let registrations = load_installations(store, node_id, &installations).await?;
    for registration in &registrations {
        store
            .apply_managed_plugin_settings_templates(registration.installation_id)
            .await?;
    }
    Ok(registrations)
}

/// Pure candidate loading. No published template or durable selection changes during preparation.
pub(crate) async fn load_installations(
    store: &MainDurableStore,
    node_id: &str,
    installations: &[domain::PluginInstallationRecord],
) -> Result<Vec<ManagedServiceRegistration>> {
    let mut registrations = Vec::new();
    let mut owners = std::collections::BTreeSet::new();
    for installation in installations {
        let local = control_plane::plugin_management::ready_current_node_plugin_installation(
            store,
            node_id,
            Path::new(""),
            installation.id,
        )
        .await?;
        let root = Path::new(
            local
                .local_path()
                .context("managed service artifact missing")?,
        );
        let mut manifest: PluginManifestV1 = plugin_framework::parse_plugin_manifest(
            &tokio::fs::read_to_string(root.join("manifest.yaml")).await?,
        )?;
        let code = manifest.plugin_code()?.to_string();
        anyhow::ensure!(
            code == installation.provider_code && manifest.version == installation.plugin_version,
            "managed service installed identity mismatch"
        );
        // System scope also hosts other governed contributions, including authentication.
        if manifest.managed_service.is_none() {
            continue;
        }
        anyhow::ensure!(
            owners.insert(code.clone()),
            "duplicate active managed service owner"
        );
        plugin_framework::validate_frontend_module_assets(root, &manifest)?;
        plugin_framework::resolve_plugin_frontend_sources(root, &mut manifest)?;
        for page in &manifest.settings_pages {
            plugin_framework::read_plugin_settings_page_source(root, page)?;
        }
        registrations.push(ManagedServiceRegistration {
            installation_id: installation.id,
            publisher_namespace: manifest.publisher_namespace,
            data_models: manifest.data_models,
            plugin_code: code,
            plugin_version: manifest.version,
            display_name: manifest.display_name,
            description: manifest.description,
            declaration: manifest
                .managed_service
                .context("system service declaration missing")?,
            pages: manifest.settings_pages,
        });
    }
    Ok(registrations)
}
/// OpenAPI and MCP discovery consume the same frozen declarations as routes and handlers.
pub(crate) fn append_openapi(
    document: &mut serde_json::Value,
    services: &[ManagedServiceRegistration],
) {
    use serde_json::json;
    for service in services {
        for op in &service.declaration.operations {
            let operation = operation_openapi(op, &service.display_name);
            if op.stream.is_some() {
                document["paths"][&op.path]["get"] = json!({"operationId":format!("{}.stream-upgrade",op.interface_id),"summary":format!("Open {} stream",op.summary),"description":"Upgrades a one-time session and Origin bound stream ticket and invokes the original POST operation authorization.","parameters":[{"name":"ticket","in":"query","required":true,"schema":{"type":"string"}}],"responses":{"101":{"description":"WebSocket upgrade"}}});
            }
            document["paths"][&op.path][op.method.to_ascii_lowercase()] = operation;
        }
    }
}

fn operation_openapi(
    op: &plugin_framework::ManagedServiceOperation,
    display_name: &str,
) -> serde_json::Value {
    use serde_json::json;
    let mut parameters = Vec::new();
    for location in ["path", "query"] {
        if let Some(fields) = op
            .input_schema
            .pointer(&format!("/properties/{location}/properties"))
            .and_then(serde_json::Value::as_object)
        {
            for (name, schema) in fields {
                let required = location == "path"
                    || op
                        .input_schema
                        .pointer(&format!("/properties/{location}/required"))
                        .and_then(serde_json::Value::as_array)
                        .is_some_and(|values| values.iter().any(|v| v.as_str() == Some(name)));
                parameters
                    .push(json!({"name":name,"in":location,"required":required,"schema":schema}));
            }
        }
    }
    // Host catalog response contracts describe ApiSuccess.data, as static DTO docs do.
    // The shared dispatcher unwraps the HTTP envelope before validating this schema.
    let mut operation = json!({"operationId":op.interface_id,"summary":op.summary,"description":op.description,"tags":[display_name],"parameters":parameters,
    "responses":{"200":{"description":"Successful operation","content":{"application/json":{"schema":op.output_schema}}}}});
    if op.method != "GET" && op.method != "DELETE" {
        operation["requestBody"] = json!({"required":true,"content":{"application/json":{"schema":op.input_schema.pointer("/properties/body").cloned().unwrap_or(json!({"type":"object"}))}}});
    }
    if op.stream.is_some() {
        operation["responses"]["200"]["content"]["application/json"]["schema"] = serde_json::json!({"type":"object","properties":{"ticket":{"type":"string"}},"required":["ticket"],"additionalProperties":false});
    }
    operation
}

/// Uses the same selected HTTP operation as append_openapi. A managed adapter's
/// worker envelope can contain empty locations that the HTTP contract omits.
pub(super) fn operation_document_input_schema(
    op: &plugin_framework::ManagedServiceOperation,
) -> serde_json::Value {
    use serde_json::json;
    let mut document = json!({"paths":{}});
    document["paths"][&op.path][op.method.to_ascii_lowercase()] =
        operation_openapi(op, "Managed service");
    crate::openapi_interface::catalog_entry_from_operation(
        &crate::openapi_docs::DocsCatalogOperation {
            id: op.interface_id.clone(),
            method: op.method.clone(),
            path: op.path.clone(),
            summary: Some(op.summary.clone()),
            description: Some(op.description.clone()),
            tags: vec![],
            group: "managed-services".into(),
            deprecated: false,
        },
        &document,
    )
    .expect("managed service owner inserts the exact selected HTTP operation")
    .request_schema
}
