use anyhow::Result;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    audit::audit_log,
    errors::ControlPlaneError,
    ports::{CreateFrontstageBlockInput, FrontstagePageRepository},
};

use super::{
    ensure_design_permission, normalize_code_ref, validate_frontstage_block_renderer_versions,
    FrontstagePageService,
};

pub struct CreateFrontstageBlockCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub page_id: Uuid,
    pub tab_id: Uuid,
    pub document_payload: Value,
    pub code_ref: String,
    pub code: String,
}

impl<R> FrontstagePageService<R>
where
    R: FrontstagePageRepository,
{
    pub async fn create_block(
        &self,
        command: CreateFrontstageBlockCommand,
    ) -> Result<domain::frontstage::FrontstagePageDetail> {
        let actor = self
            .repository
            .load_actor_context_for_workspace(command.actor_user_id, command.workspace_id)
            .await?;
        ensure_design_permission(&actor)?;
        self.ensure_existing_page(command.workspace_id, command.page_id)
            .await?;
        validate_frontstage_block_renderer_versions(&command.document_payload)?;
        let code_ref = normalize_code_ref(command.code_ref)?;
        validate_created_block_binding(&command.document_payload, &code_ref)?;

        let audit_log = audit_log(
            Some(actor.current_workspace_id),
            Some(actor.user_id),
            "frontstage_page",
            Some(command.page_id),
            "frontstage.block_created",
            serde_json::json!({
                "code_ref": code_ref,
                "tab_id": command.tab_id,
            }),
        );

        self.repository
            .create_frontstage_block(&CreateFrontstageBlockInput {
                workspace_id: command.workspace_id,
                actor_user_id: command.actor_user_id,
                page_id: command.page_id,
                tab_id: command.tab_id,
                document_payload: command.document_payload,
                code_ref,
                code: command.code,
                audit_log,
            })
            .await
    }
}

fn validate_created_block_binding(document_payload: &Value, code_ref: &str) -> Result<()> {
    let count = document_payload
        .as_object()
        .and_then(|document| document.get("blocks"))
        .and_then(Value::as_array)
        .map(|blocks| {
            blocks
                .iter()
                .filter(|block| {
                    block
                        .as_object()
                        .and_then(|block| block.get("codeRef"))
                        .and_then(Value::as_str)
                        == Some(code_ref)
                })
                .count()
        })
        .unwrap_or(0);

    if count != 1 {
        return Err(ControlPlaneError::InvalidInput("frontstage_block_code_ref_binding").into());
    }

    Ok(())
}
