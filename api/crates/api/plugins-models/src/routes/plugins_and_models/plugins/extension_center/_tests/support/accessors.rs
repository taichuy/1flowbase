use super::*;

#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl InstalledCatalogJoin {
    pub fn test_new(
        installation_id: Uuid,
        current_version: String,
        source: String,
        trust: String,
        status: InstalledCatalogJoinStatus,
    ) -> Self {
        Self {
            installation_id,
            current_version,
            source,
            trust,
            status,
        }
    }
}
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl UploadedExtensionArtifact {
    pub fn test_new(
        category: ExtensionCatalogCategory,
        organization: String,
        artifact_id: String,
        version: String,
        minimum_host_version: Option<String>,
        node_plugin: bool,
        signature_status: domain::ExtensionSignatureStatus,
        signature_algorithm: Option<String>,
        signing_key_id: Option<String>,
        application_action: domain::ExtensionApplicationAction,
        managed_schema: Option<ManagedSchemaDeclaration>,
    ) -> Self {
        Self {
            category,
            organization,
            artifact_id,
            version,
            minimum_host_version,
            node_plugin,
            signature_status,
            signature_algorithm,
            signing_key_id,
            application_action,
            managed_schema,
        }
    }
}
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl InstalledCatalogJoin {
    pub fn test_current_version(&self) -> &String {
        &self.current_version
    }
}
