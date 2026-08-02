use async_trait::async_trait;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct UpsertExtensionInstallationInput {
    pub installation_id: Uuid,
    pub identity: domain::ExtensionInstallationIdentity,
    pub source: String,
    pub trust: String,
    pub local_path: String,
    pub checksum: String,
    pub signature_status: domain::ExtensionSignatureStatus,
    pub signature_algorithm: Option<String>,
    pub signing_key_id: Option<String>,
    pub warnings: Vec<domain::ExtensionIntegrityWarning>,
    pub receipt: serde_json::Value,
    pub application_action: domain::ExtensionApplicationAction,
    pub status: domain::ExtensionInstallationStatus,
    pub is_current: bool,
    pub installed_by: Uuid,
}

#[async_trait]
pub trait ExtensionInstallationRepository: Send + Sync {
    async fn upsert_extension_installation(
        &self,
        input: &UpsertExtensionInstallationInput,
    ) -> anyhow::Result<domain::ExtensionInstallationRecord>;

    async fn find_extension_installation(
        &self,
        identity: &domain::ExtensionInstallationIdentity,
    ) -> anyhow::Result<Option<domain::ExtensionInstallationRecord>>;

    async fn find_extension_installation_by_id(
        &self,
        node_id: &str,
        installation_id: Uuid,
    ) -> anyhow::Result<Option<domain::ExtensionInstallationRecord>>;

    async fn list_extension_installations_for_node(
        &self,
        node_id: &str,
    ) -> anyhow::Result<Vec<domain::ExtensionInstallationRecord>>;

    async fn set_extension_installation_status(
        &self,
        installation_id: Uuid,
        status: domain::ExtensionInstallationStatus,
    ) -> anyhow::Result<()>;

    async fn select_current_extension_installation(
        &self,
        node_id: &str,
        installation_id: Uuid,
    ) -> anyhow::Result<Option<domain::ExtensionInstallationRecord>>;

    async fn remove_extension_installation(
        &self,
        node_id: &str,
        installation_id: Uuid,
    ) -> anyhow::Result<Option<domain::ExtensionInstallationRecord>>;
}
