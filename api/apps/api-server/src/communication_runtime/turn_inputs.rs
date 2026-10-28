//! Host-only input preparation. Private supplier locators never enter runtime messages.
use super::*;
use control_plane::{
    orchestration_runtime::CommunicationInputPreparer,
    ports::{CommunicationTurnClaim, CommunicationTurnInput},
};
use orchestration_runtime::execution_engine::communication::CommunicationExecutionInput;
use serde_json::json;

pub(super) struct TurnInputPreparer {
    state: Arc<ApiState>,
    binding: CommunicationBindingRecord,
}

impl TurnInputPreparer {
    pub(super) fn new(state: Arc<ApiState>, binding: CommunicationBindingRecord) -> Self {
        Self { state, binding }
    }

    async fn validate(&self, claim: &CommunicationTurnClaim) -> Result<()> {
        let current = active_binding(
            &self.state,
            (self.binding.workspace_id, self.binding.instance_id),
        )
        .await?;
        validate_binding_snapshot(&self.binding, &current, claim)?;
        validate_binding(&self.state.store, &current).await?;
        ensure!(
            self.state
                .notification_service
                .renew_turn_claim(claim, LEASE_SECONDS)
                .await?,
            "communication_turn_fenced"
        );
        Ok(())
    }
}

#[async_trait::async_trait]
impl CommunicationInputPreparer for TurnInputPreparer {
    async fn prepare_inputs(
        &self,
        claim: &CommunicationTurnClaim,
        inputs: Vec<CommunicationTurnInput>,
    ) -> Result<Vec<CommunicationExecutionInput>> {
        let _permit = self.state.system_maintenance.try_enter_write(
            control_plane::system_recovery::SystemWriteOwner::CommunicationExecution,
        )?;
        // Runs can remain open for human/tool callbacks. Reload account, workspace,
        // application, MCP and file permissions instead of keeping the initial actor.
        self.validate(claim).await?;
        let mut prepared = Vec::with_capacity(inputs.len());
        for (index, input) in inputs.into_iter().enumerate() {
            let files = if input.attachments.is_empty() {
                Vec::new()
            } else {
                // A previous member may have needed a slow provider download. Fence the
                // original binding and claim again before importing this member's files.
                if index > 0 {
                    self.validate(claim).await?;
                }
                attachments::attach_turn(&self.state, &self.binding, &input.attachments, claim)
                    .await?
            };
            prepared.push(prepare_input(input, files)?);
        }
        Ok(prepared)
    }
}

fn validate_binding_snapshot(
    original: &CommunicationBindingRecord,
    current: &CommunicationBindingRecord,
    claim: &CommunicationTurnClaim,
) -> Result<()> {
    ensure!(
        claim.scope.workspace_id == original.workspace_id
            && claim.scope.instance_id == original.instance_id
            && current.workspace_id == original.workspace_id
            && current.instance_id == original.instance_id,
        "communication_turn_scope_invalid"
    );
    ensure!(current.revision == original.revision, "binding_changed");
    Ok(())
}

fn prepare_input(
    input: CommunicationTurnInput,
    files: Vec<Value>,
) -> Result<CommunicationExecutionInput> {
    let mut content = input.content.unwrap_or_default();
    if !files.is_empty() {
        content.push_str("\n\nAttached files: ");
        content.push_str(&serde_json::to_string(&files)?);
    }
    let mut message = json!({"role":"user", "content":content});
    if !files.is_empty() {
        // Keep the safe references structured for NativeRunRequest.attachments. Parsing
        // user-authored text for file IDs would let text impersonate an imported file.
        // Provider projection reads content; later rounds retain the same public IDs.
        message["attachments"] = Value::Array(files);
    }
    Ok(CommunicationExecutionInput {
        input_id: input.input_id,
        input_sequence: input.input_sequence,
        messages: vec![message],
    })
}

#[cfg(test)]
#[path = "_tests/turn_inputs.rs"]
mod tests;
