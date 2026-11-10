use access_control::*;
use anyhow::{Context, Result};
use control_plane_contracts::ports::PluginRepository;
use plugin_framework::{ManagedServiceManifest, PluginManifestV1, PluginSettingsPageManifest};
use std::path::Path;
use storage_durable_postgres::MainDurableStore;

#[derive(Debug, Clone)]
pub(crate) struct ManagedServiceRegistration {
    pub installation_id: uuid::Uuid,
    pub publisher_namespace: String,
    pub data_models: Vec<extension_contracts::PluginDataModelContribution>,
    pub plugin_code: String,
    pub plugin_version: String,
    pub display_name: String,
    pub description: String,
    pub declaration: ManagedServiceManifest,
    pub pages: Vec<PluginSettingsPageManifest>,
}
impl ManagedServiceRegistration {
    pub fn owner(&self) -> SettingsFeatureOwner {
        SettingsFeatureOwner {
            kind: SettingsFeatureOwnerKind::ManagedService,
            owner_id: self.plugin_code.clone(),
            version: self.plugin_version.clone(),
        }
    }
    pub fn feature(&self) -> Option<SettingsFeatureRegistration> {
        if self.pages.is_empty() {
            return None;
        }
        let f = self.declaration.feature.as_ref()?;
        Some(SettingsFeatureRegistration {
            feature_id: f.feature_id.clone(),
            owner: self.owner(),
            lifecycle: SettingsFeatureLifecycle::Active,
            console_surface: SettingsFeatureConsoleSurface {
                route_id: f.route_id.clone(),
                surface_key: format!("managed-service:{}", self.plugin_code),
                path: f.path.clone(),
                label_key: f.label.clone(),
                description_key: f.description.clone(),
                order: 1000,
            },
            api_routes: self
                .declaration
                .operations
                .iter()
                .map(|op| SettingsApiRoute {
                    method: op.method.clone(),
                    path: op.path.clone(),
                })
                .collect(),
        })
    }
    pub fn locale_catalog(&self) -> ConsoleLocaleCatalogContribution {
        if let Some(feature) = self.feature() {
            // Settings surfaces may have different captions; their permissions still belong to
            // the one plugin group.
            let mut contribution = self.plugin_locale_catalog();
            for text in [
                feature.console_surface.label_key,
                feature.console_surface.description_key,
            ] {
                if !contribution
                    .texts
                    .iter()
                    .any(|existing| existing.reference == text)
                {
                    contribution.texts.push(ConsoleLocaleText {
                        reference: text.clone(),
                        en_us: text.clone(),
                        zh_hans: text,
                    });
                }
            }
            return contribution;
        }
        self.plugin_locale_catalog()
    }
    fn plugin_locale_catalog(&self) -> ConsoleLocaleCatalogContribution {
        let texts = [&self.display_name, &self.description]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .map(|text| ConsoleLocaleText {
                reference: text.clone(),
                en_us: text.clone(),
                zh_hans: text.clone(),
            })
            .collect();
        ConsoleLocaleCatalogContribution {
            owner: self.owner(),
            lifecycle: SettingsFeatureLifecycle::Active,
            texts,
            policy_groups: Vec::new(),
            plugin_groups: vec![ConsoleOtherPolicyGroupDisplay {
                group_id: self.plugin_code.clone(),
                label_ref: self.display_name.clone(),
                description_ref: self.description.clone(),
            }],
        }
    }
    pub fn operations(&self) -> Vec<ConsoleOperationRegistration> {
        self.declaration
            .operations
            .iter()
            .enumerate()
            .map(|(index, op)| ConsoleOperationRegistration {
                operation_id: op.interface_id.clone(),
                authorization_profile_id: Some(op.interface_id.clone()),
                owner: self.owner(),
                lifecycle: SettingsFeatureLifecycle::Active,
                policy_group: ConsolePolicyGroup::Plugin(self.plugin_code.clone()),
                order: index as i32,
                routes: vec![ConsoleRouteBinding {
                    method: op.method.clone(),
                    path: op.path.clone(),
                }],
                authorization: ConsoleAuthorization::Simple,
            })
            .collect()
    }
    pub fn metadata(&self) -> Vec<ConsoleInterfaceRegistration> {
        self.declaration
            .operations
            .iter()
            .flat_map(|op| {
                let mut rows = vec![ConsoleInterfaceRegistration {
                interface_id: op.interface_id.clone(),
                route: ConsoleRouteBinding {
                    method: op.method.clone(),
                    path: op.path.clone(),
                },
                summary: op.summary.clone(),
                description: op.description.clone(),
            }];
                if op.stream.is_some() {
                    rows.push(ConsoleInterfaceRegistration {
                        interface_id: format!("{}.stream-upgrade",op.interface_id),
                        route: ConsoleRouteBinding {method:"GET".into(),path:op.path.clone()},
                        summary: format!("Open {} stream", op.summary),
                        description: "Upgrades a single-use session and Origin bound ticket; execution remains authorized by the declared POST operation.".into(),
                    });
                }
                rows
            })
            .collect()
    }
}
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
