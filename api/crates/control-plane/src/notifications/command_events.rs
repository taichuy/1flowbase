use super::*;
use control_plane_contracts::ports::*;
use domain::CommunicationConversationScope;
use extension_contracts::CommunicationCommandReply;

/// Decode only a persisted Host request after exclusive claim. Host registers the finite typed
/// executors; ordinary receive classification only enqueues this intent.
pub fn decode_builtin_command(
    record: &CommunicationCommandRecord,
) -> Result<CommunicationCommandIntent> {
    anyhow::ensure!(
        record.installation_id.is_none(),
        "command is managed by plugin outbox"
    );
    if record.request.command_id == "host/rejected" {
        let (code, reply): (String, String) = serde_json::from_str(&record.request.arguments)?;
        return Ok(CommunicationCommandIntent::Rejected { code, reply });
    }
    let catalog = plugin_framework::communication_commands::builtin_communication_commands();
    let entry = catalog
        .iter()
        .find(|entry| entry.descriptor.command_id == record.request.command_id)
        .ok_or(Error::InvalidInput("unknown_builtin_command"))?;
    Ok(
        plugin_framework::communication_commands::parse_communication_command(
            &format!("/{} {}", entry.descriptor.name, record.request.arguments),
            &catalog,
        ),
    )
}

impl<R> NotificationService<R>
where
    R: CommunicationCommandRepository,
{
    pub async fn command_for_input(
        &self,
        scope: &CommunicationConversationScope,
        input_id: Uuid,
    ) -> Result<Option<CommunicationCommandRecord>> {
        self.repository
            .communication_command_for_input(scope, input_id)
            .await
    }
    pub async fn enqueue_command(
        &self,
        scope: &CommunicationConversationScope,
        input_id: Uuid,
        intent: CommunicationCommandIntent,
    ) -> Result<CommunicationCommandRecord> {
        if let Some(existing) = self
            .repository
            .communication_command_for_input(scope, input_id)
            .await?
        {
            return Ok(existing);
        }
        let (command_id, arguments, target) = match intent {
            CommunicationCommandIntent::Ordinary => {
                return Err(Error::InvalidInput("ordinary_input_is_not_command").into());
            }
            CommunicationCommandIntent::Builtin(builtin) => {
                let (kind, arguments) = match builtin {
                    BuiltinCommunicationIntent::New => {
                        (BuiltinCommunicationCommand::New, String::new())
                    }
                    BuiltinCommunicationIntent::Help => {
                        (BuiltinCommunicationCommand::Help, String::new())
                    }
                    BuiltinCommunicationIntent::Model { model } => (
                        BuiltinCommunicationCommand::Model,
                        model.unwrap_or_default(),
                    ),
                };
                let descriptor =
                    plugin_framework::communication_commands::builtin_communication_commands()
                        .into_iter()
                        .find(|entry| entry.builtin == Some(kind))
                        .ok_or(Error::InvalidInput("builtin_command_registry"))?
                        .descriptor;
                (descriptor.command_id, arguments, None)
            }
            CommunicationCommandIntent::Rejected { code, reply } => (
                "host/rejected".into(),
                serde_json::to_string(&(code, reply))?,
                None,
            ),
            CommunicationCommandIntent::Plugin {
                command_id,
                arguments,
                installation_id,
            } => {
                let target = self
                    .commands
                    .as_ref()
                    .ok_or(Error::Conflict(
                        "communication_command_provider_unavailable",
                    ))?
                    .freeze_target(scope.workspace_id, installation_id, &command_id)
                    .await?;
                (command_id, arguments, Some(target))
            }
        };
        self.repository
            .begin_communication_command(BeginCommunicationCommand {
                scope: scope.clone(),
                input_id,
                command_id,
                arguments,
                target,
            })
            .await
    }
    pub async fn claim_builtin_command(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
        lease_seconds: u32,
    ) -> Result<Option<ClaimedBuiltinCommunicationCommand>> {
        self.repository
            .claim_builtin_communication_command(workspace_id, instance_id, lease_seconds)
            .await
    }
    pub async fn renew_builtin_command_claim(
        &self,
        claim: &ClaimedBuiltinCommunicationCommand,
        lease_seconds: u32,
    ) -> Result<bool> {
        self.repository
            .renew_builtin_communication_command(claim, lease_seconds)
            .await
    }
    pub async fn complete_builtin_command(
        &self,
        claim: &ClaimedBuiltinCommunicationCommand,
        response_session_id: Uuid,
        reply: CommunicationCommandReply,
        failure_code: Option<String>,
    ) -> Result<CommunicationCommandRecord> {
        reply.validate()?;
        self.repository
            .complete_builtin_communication_command(claim, response_session_id, reply, failure_code)
            .await
    }
    pub async fn select_command_model(
        &self,
        claim: &ClaimedBuiltinCommunicationCommand,
        model: &str,
        reply: CommunicationCommandReply,
    ) -> Result<CommunicationCommandRecord> {
        anyhow::ensure!(!model.trim().is_empty(), "command model must not be empty");
        reply.validate()?;
        self.repository
            .select_communication_command_model(claim, model, reply)
            .await
    }
    pub async fn pending_command_responses(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
        limit: u32,
    ) -> Result<Vec<CommunicationCommandRecord>> {
        self.repository
            .pending_communication_command_responses(workspace_id, instance_id, limit)
            .await
    }
    pub async fn reserve_command_response(
        &self,
        scope: &CommunicationConversationScope,
        event_id: Uuid,
    ) -> Result<Option<ClaimedCommunicationCommandResponse>> {
        self.repository
            .reserve_communication_command_response(scope, event_id)
            .await
    }
    pub async fn command_response_target(
        &self,
        scope: &CommunicationConversationScope,
        event_id: Uuid,
    ) -> Result<Option<extension_contracts::NotificationSendTarget>> {
        self.repository
            .communication_command_response_target(scope, event_id)
            .await
    }
    pub async fn record_command_response(
        &self,
        scope: &CommunicationConversationScope,
        event_id: Uuid,
        claim_id: Uuid,
        outcome: CommunicationCommandSendOutcome,
    ) -> Result<bool> {
        self.repository
            .record_communication_command_response(scope, event_id, claim_id, outcome)
            .await
    }
}
