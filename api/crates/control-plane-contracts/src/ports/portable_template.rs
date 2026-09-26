use crate::portable_template::PortableTemplatePackage;
use std::collections::BTreeMap;
use uuid::Uuid;
/// Read projection scoped by the authenticated target/source workspace. It never writes editor state.
#[async_trait::async_trait]
pub trait PortableTemplateReadRepository: Send + Sync {
    async fn portable_template_snapshot(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
    ) -> anyhow::Result<PortableTemplatePackage>;
}

#[async_trait::async_trait]
pub trait PortableTemplateIdentityRepository: Send + Sync {
    async fn load_portable_template_identity_map(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<BTreeMap<String, String>>;

    async fn record_portable_template_identity(
        &self,
        workspace_id: Uuid,
        kind: &str,
        source_id: &str,
        target_id: &str,
    ) -> anyhow::Result<()>;
}
