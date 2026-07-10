use super::*;

#[async_trait]
pub trait FlowRepository: Send + Sync {
    async fn get_or_create_editor_state(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
    ) -> anyhow::Result<domain::FlowEditorState>;
    async fn save_draft(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        document: serde_json::Value,
        change_kind: domain::FlowChangeKind,
        summary: &str,
    ) -> anyhow::Result<domain::FlowEditorState>;
    async fn restore_version(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        version_id: Uuid,
    ) -> anyhow::Result<domain::FlowEditorState>;
    #[allow(clippy::too_many_arguments)]
    async fn update_version_metadata(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        version_id: Uuid,
        summary: Option<String>,
        summary_is_custom: Option<bool>,
        is_user_protected: Option<bool>,
    ) -> anyhow::Result<domain::FlowEditorState>;
}
