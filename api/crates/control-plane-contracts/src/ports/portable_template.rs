use crate::portable_template::PortableTemplatePackage;
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
