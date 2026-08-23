use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontendBlockPermissions {
    pub network: String,
    pub storage: String,
    pub secrets: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrontendBlockContextContract {
    pub primitives: Vec<String>,
    pub input_schema: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontendBlockCodeModule {
    pub source: String,
    pub version: String,
    pub exports: Vec<String>,
    pub binding: FrontendModuleBinding,
    #[serde(default)]
    pub assets: Vec<FrontendModuleAsset>,
    pub type_declarations: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrontendModuleBinding {
    #[serde(rename = "host")]
    Host,
    #[serde(rename = "fetched")]
    Fetched,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontendModuleAsset {
    pub path: String,
    pub role: FrontendModuleAssetRole,
    pub media_type: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrontendModuleAssetRole {
    #[serde(rename = "browser_module")]
    BrowserModule,
    #[serde(rename = "shadow_style")]
    ShadowStyle,
    #[serde(rename = "support")]
    Support,
}

impl FrontendBlockCodeModule {
    pub fn resolved_type_declarations(&self) -> String {
        self.type_declarations.trim_end().to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrontendBlockCatalogEntry {
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
    pub code_modules: Vec<FrontendBlockCodeModule>,
    pub context_contract: FrontendBlockContextContract,
    pub permissions: FrontendBlockPermissions,
    pub ui_capabilities: Vec<String>,
}
