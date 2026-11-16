use super::*;

#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub struct UploadedExtensionArtifactTestIdentity {
    pub category: ExtensionCatalogCategory,
    pub organization: String,
    pub artifact_id: String,
    pub version: String,
}

#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub struct UploadedExtensionArtifactTestSignature {
    pub signature_status: domain::ExtensionSignatureStatus,
    pub signature_algorithm: Option<String>,
    pub signing_key_id: Option<String>,
}

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
        identity: UploadedExtensionArtifactTestIdentity,
        minimum_host_version: Option<String>,
        node_plugin: bool,
        signature: UploadedExtensionArtifactTestSignature,
        application_action: domain::ExtensionApplicationAction,
        managed_schema: Option<ManagedSchemaDeclaration>,
    ) -> Self {
        Self {
            category: identity.category,
            organization: identity.organization,
            artifact_id: identity.artifact_id,
            version: identity.version,
            minimum_host_version,
            node_plugin,
            signature_status: signature.signature_status,
            signature_algorithm: signature.signature_algorithm,
            signing_key_id: signature.signing_key_id,
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
