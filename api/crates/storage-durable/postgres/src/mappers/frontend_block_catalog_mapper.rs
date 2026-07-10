use anyhow::{anyhow, Result};
use serde_json::Value;
use uuid::Uuid;

pub struct StoredFrontendBlockCatalogRow {
    pub installation_id: Uuid,
    pub provider_code: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub contribution_code: String,
    pub title: String,
    pub runtime: String,
    pub entry: String,
    pub code_template: Option<String>,
    pub code_template_version: Option<String>,
    pub code_template_language: Option<String>,
    pub code_modules: Value,
    pub context_contract: Value,
    pub permission_network: String,
    pub permission_storage: String,
    pub permission_secrets: String,
    pub ui_capabilities: Value,
}

pub struct PgFrontendBlockCatalogMapper;

impl PgFrontendBlockCatalogMapper {
    pub fn to_catalog_entry(
        row: StoredFrontendBlockCatalogRow,
    ) -> Result<domain::FrontendBlockCatalogEntry> {
        Ok(domain::FrontendBlockCatalogEntry {
            installation_id: row.installation_id,
            provider_code: row.provider_code,
            plugin_id: row.plugin_id,
            plugin_version: row.plugin_version,
            contribution_code: row.contribution_code,
            title: row.title,
            runtime: row.runtime,
            entry: row.entry,
            code_template: row.code_template,
            code_template_version: row.code_template_version,
            code_template_language: row.code_template_language,
            code_modules: parse_code_modules(row.code_modules)?,
            context_contract: parse_context_contract(row.context_contract)?,
            permissions: domain::FrontendBlockPermissions {
                network: row.permission_network,
                storage: row.permission_storage,
                secrets: row.permission_secrets,
            },
            ui_capabilities: parse_string_array(row.ui_capabilities)?,
        })
    }
}

fn parse_code_modules(value: Value) -> Result<Vec<domain::FrontendBlockCodeModule>> {
    serde_json::from_value(value).map_err(Into::into)
}

fn parse_context_contract(value: Value) -> Result<domain::FrontendBlockContextContract> {
    let primitives = value
        .get("primitives")
        .cloned()
        .map(parse_string_array)
        .transpose()?
        .unwrap_or_default();
    let input_schema = value.get("input_schema").cloned().unwrap_or(Value::Null);
    Ok(domain::FrontendBlockContextContract {
        primitives,
        input_schema,
    })
}

fn parse_string_array(value: Value) -> Result<Vec<String>> {
    let items = value
        .as_array()
        .ok_or_else(|| anyhow!("expected json array of strings"))?;
    items
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or_else(|| anyhow!("expected string array item"))
        })
        .collect()
}
