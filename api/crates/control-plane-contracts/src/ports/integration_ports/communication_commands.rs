//! Host-only durable command records. Request scope/destination never come from worker output.
use super::{FrozenLifecyclePublication, LifecycleSubscriberTarget, ManagedFrozenExecutionTarget};
use anyhow::Result;
use async_trait::async_trait;
use domain::{CommunicationConversationScope, CommunicationSendStatus};
use extension_contracts::{
    CommunicationCommandReply, CommunicationCommandRequest, ManagedContributionSubject,
    ManagedEventDelivery,
};
use uuid::Uuid;

pub struct FrozenCommunicationCommandTarget {
    pub subject: ManagedContributionSubject,
    pub publication: FrozenLifecyclePublication,
}

pub struct BeginCommunicationCommand {
    pub scope: CommunicationConversationScope,
    pub input_id: Uuid,
    pub command_id: String,
    pub arguments: String,
    /// None is reserved for host builtins and explicit parser errors.
    pub target: Option<FrozenCommunicationCommandTarget>,
}

#[derive(Debug, Clone)]
pub struct CommunicationCommandRecord {
    pub event_id: Uuid,
    pub scope: CommunicationConversationScope,
    pub request: CommunicationCommandRequest,
    pub installation_id: Option<Uuid>,
    pub contribution_id: Option<String>,
    pub target: Option<ManagedFrozenExecutionTarget>,
    pub subscriber_id: Option<String>,
    pub reply: Option<CommunicationCommandReply>,
    pub failure_code: Option<String>,
    pub send_status: CommunicationSendStatus,
    pub response_session_id: Option<Uuid>,
}

/// This receipt is validated against the persisted event, frozen target and current authority.
pub struct CompleteManagedCommunicationCommand {
    pub subject: ManagedContributionSubject,
    pub event_id: Uuid,
    pub graph_fingerprint: String,
    pub target: LifecycleSubscriberTarget,
    pub reply: CommunicationCommandReply,
    pub failure_code: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ClaimedCommunicationCommandResponse {
    pub record: CommunicationCommandRecord,
    pub claim_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct ClaimedBuiltinCommunicationCommand {
    pub record: CommunicationCommandRecord,
    pub claim_id: Uuid,
}

#[derive(Debug, Clone, Copy)]
pub enum CommunicationCommandSendOutcome {
    Succeeded,
    Failed,
    Unknown,
}

#[async_trait]
pub trait CommunicationCommandRepository: Send + Sync {
    /// Serializes on the durable input. Repeated identical input reuses the original frozen target.
    async fn begin_communication_command(
        &self,
        input: BeginCommunicationCommand,
    ) -> Result<CommunicationCommandRecord>;
    async fn communication_command_for_input(
        &self,
        scope: &CommunicationConversationScope,
        input_id: Uuid,
    ) -> Result<Option<CommunicationCommandRecord>>;
    async fn communication_command(
        &self,
        workspace_id: Uuid,
        event_id: Uuid,
    ) -> Result<Option<CommunicationCommandRecord>>;
    async fn claim_builtin_communication_command(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
        lease_seconds: u32,
    ) -> Result<Option<ClaimedBuiltinCommunicationCommand>>;
    async fn renew_builtin_communication_command(
        &self,
        claim: &ClaimedBuiltinCommunicationCommand,
        lease_seconds: u32,
    ) -> Result<bool>;
    /// Host effects must have committed before this call. Plugin records require an authority lease.
    async fn complete_builtin_communication_command(
        &self,
        claim: &ClaimedBuiltinCommunicationCommand,
        response_session_id: Uuid,
        reply: CommunicationCommandReply,
        failure_code: Option<String>,
    ) -> Result<CommunicationCommandRecord>;
    /// Host already validated the requested model against the bound application's capabilities.
    /// Fence the command claim and active request session, select the chat model and commit the
    /// reply/exclusion together, so an expired worker cannot overwrite a later selection.
    async fn select_communication_command_model(
        &self,
        claim: &ClaimedBuiltinCommunicationCommand,
        model: &str,
        reply: CommunicationCommandReply,
    ) -> Result<CommunicationCommandRecord>;
    async fn pending_communication_command_responses(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
        limit: u32,
    ) -> Result<Vec<CommunicationCommandRecord>>;
    /// Resolve immutable provider destination only from the fully scoped persisted request,
    /// original inbound receipt and conversation. Worker results cannot supply any selector.
    async fn communication_command_response_target(
        &self,
        scope: &CommunicationConversationScope,
        event_id: Uuid,
    ) -> Result<Option<extension_contracts::NotificationSendTarget>>;
    /// Reserves once. An abandoned reservation becomes unknown; it is never automatically resent.
    async fn reserve_communication_command_response(
        &self,
        scope: &CommunicationConversationScope,
        event_id: Uuid,
    ) -> Result<Option<ClaimedCommunicationCommandResponse>>;
    async fn record_communication_command_response(
        &self,
        scope: &CommunicationConversationScope,
        event_id: Uuid,
        claim_id: Uuid,
        outcome: CommunicationCommandSendOutcome,
    ) -> Result<bool>;
}

pub fn communication_command_delivery(
    event_id: Uuid,
    scope: &CommunicationConversationScope,
    request: &CommunicationCommandRequest,
) -> Result<ManagedEventDelivery> {
    request.validate()?;
    anyhow::ensure!(
        request.channel_id == scope.instance_id.to_string()
            && request.conversation_id == scope.conversation_id.to_string(),
        "command request scope mismatch"
    );
    let delivery = ManagedEventDelivery {
        event_id: event_id.to_string(),
        contract_id: extension_contracts::COMMUNICATION_COMMAND_POINT.into(),
        contract_version: "1".into(),
        point_id: extension_contracts::COMMUNICATION_COMMAND_POINT.into(),
        point_contract_version: "1".into(),
        schema: extension_contracts::ManagedEventSchema {
            contract_id: extension_contracts::COMMUNICATION_COMMAND_POINT.into(),
            contract_version: "1".into(),
            payload_schema: CommunicationCommandRequest::event_schema(),
        },
        workspace_id: scope.workspace_id.to_string(),
        causation_id: request.input_id.clone(),
        correlation_id: request.input_id.clone(),
        payload: serde_json::to_value(request)?,
    };
    delivery.validate()?;
    Ok(delivery)
}

#[cfg(test)]
#[path = "_tests/communication_commands.rs"]
mod tests;
