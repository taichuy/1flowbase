//! Provider-owned exports and consumer-requested rights. No deployment participant registry.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginDependency {
    pub publisher_namespace: String,
    pub plugin_code: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginResourceAccess {
    ReadOnly,
    ReadWrite,
}
impl PluginResourceAccess {
    pub fn allows(self, requested: Self) -> bool {
        self == Self::ReadWrite || requested == Self::ReadOnly
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginResourceCollection {
    pub collection_code: String,
    pub access: PluginResourceAccess,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginResourceRecordBinding {
    pub resource_code: String,
    pub collection_code: String,
    pub id_field: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginResourceExport {
    pub resource_code: String,
    #[serde(default)]
    pub collections: Vec<PluginResourceCollection>,
    #[serde(default)]
    pub record_resources: Vec<PluginResourceRecordBinding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credentials: Option<PluginResourceAccess>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginResourceImport {
    pub provider: PluginDependency,
    pub resource_code: String,
    #[serde(default)]
    pub collections: Vec<PluginResourceCollection>,
    #[serde(default)]
    pub record_resources: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credentials: Option<PluginResourceAccess>,
}
